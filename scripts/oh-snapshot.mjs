import { lstatSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, renameSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { canonicalJson, createOhSyncBundleV1, parseOhSyncBundleV1, sha256Hex } from "@hraness/oh";
import { Oh } from "@hraness/oh/sdk";
import { CONTRACT_SHA256, PROFILE, SPACE, inspectContract, openLedger } from "./oh-ledger.mjs";
import { admitPublicHistory, admitUnpublishedDocuments } from "./oh-public-records.mjs";

// The committed history is a sequence of pages, ledger/pages/NNNN.json, each a
// complete oh.sync.v1 bundle for one contiguous interval, serialized exactly as
// the former single file. Pages lift the byte bound on the whole history; the
// reviewed 1,000-operation bound stays. Every bound is a size review, never a
// silent truncation.
export const PAGE_SOFT_BYTES = 4194304;
export const PAGE_HARD_BYTES = 8388608;
export const MAX_MANIFEST_BYTES = 65536;
const MAX_OPERATIONS = 1000;
// Retained bound of the legacy single-file layout (peqnp.public-ledger.v1),
// read only under a v1 manifest or during migration.
const MAX_LEGACY_BYTES = 16 * 1024 * 1024;
const SERIALIZATION = "json-indent-2-newline";
const LAYOUT = Object.freeze({ pageSoftBytes: PAGE_SOFT_BYTES, pageHardBytes: PAGE_HARD_BYTES, serialization: SERIALIZATION });
const SCHEMA_V1 = "peqnp.public-ledger.v1";
const SCHEMA_V2 = "peqnp.public-ledger.v2";
const LEGACY_PATH = "ledger/operations.json";
const MANIFEST_PATH = "ledger/manifest.json";
const INTERPRETATION = "Replay verifies record history and integrity, not mathematical truth or experimental provenance.";
const GENESIS = { sequence: 0, operationSha256: null };
const HEX64 = /^[a-f0-9]{64}$/;
const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function info(path) {
  try { return lstatSync(path); } catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}

function serialize(value) { return Buffer.from(JSON.stringify(value, null, 2) + "\n"); }
function pageName(index) { return String(index).padStart(4, "0") + ".json"; }
function pagePath(index) { return "ledger/pages/" + pageName(index); }
function same(left, right) { return canonicalJson(left) === canonicalJson(right); }
function sameOperations(left, right) { return left.length === right.length && left.every((operation, index) => same(operation, right[index])); }

// A snapshot file must be one regular, unlinked file within its bound.
function regularFile(path, bound, what) {
  const file = info(path);
  if (!file) return null;
  if (!file.isFile() || file.isSymbolicLink() || file.nlink !== 1) throw new Error(`${what} must be one regular, unlinked file.`);
  if (file.size > bound) throw new Error(`Missing or oversized ${what}: ${file.size} bytes exceeds the ${bound}-byte bound.`);
  return file;
}

function readBounded(path, bound, what) {
  if (!regularFile(path, bound, what)) throw new Error(`Missing ${what}.`);
  const bytes = readFileSync(path);
  if (bytes.length > bound) throw new Error(`${what} grew beyond its bound.`);
  return bytes;
}

export function validateLayout(layout) {
  if (!layout || typeof layout !== "object" || Object.keys(layout).length !== 3 ||
      !Number.isSafeInteger(layout.pageSoftBytes) || layout.pageSoftBytes < 1 ||
      !Number.isSafeInteger(layout.pageHardBytes) || layout.pageSoftBytes > layout.pageHardBytes || layout.pageHardBytes > PAGE_HARD_BYTES ||
      layout.serialization !== SERIALIZATION) {
    throw new Error("Invalid ledger page layout; expected integer pageSoftBytes <= pageHardBytes <= " + PAGE_HARD_BYTES + " and " + SERIALIZATION + ".");
  }
  return { pageSoftBytes: layout.pageSoftBytes, pageHardBytes: layout.pageHardBytes, serialization: SERIALIZATION };
}

function assertOperationRules(operation) {
  if (operation.actorId !== "peqnp.local-research" || !/^op_peqnp_[a-f0-9]{64}$/.test(operation.operationId) || operation.changes.some(change =>
    change.kind !== "put" || change.record.value.profile !== PROFILE)) {
    throw new Error("Snapshot contains an unsupported actor, mutation, or research profile; review its public scope.");
  }
}

function assertChain(prior, operations, what) {
  for (const operation of operations) {
    if (operation.sequence !== prior.sequence + 1 || operation.parentOperationSha256 !== prior.operationSha256) {
      throw new Error(`${what} does not continue the complete ordered history from genesis at sequence ${prior.sequence}.`);
    }
    prior = operation;
  }
  return prior;
}

function parseProjectBundle(value) {
  const bundle = parseOhSyncBundleV1(value);
  if (!bundle || bundle.spaceId !== SPACE || bundle.contractSha256 !== CONTRACT_SHA256 || !bundle.operations.length) {
    throw new Error("Invalid public peqnp operation bundle or contract.");
  }
  for (const operation of bundle.operations) assertOperationRules(operation);
  return bundle;
}

// Legacy rule: one bundle holding the complete history from genesis.
function validateBundle(value) {
  inspectContract();
  const bundle = parseProjectBundle(value);
  assertChain(GENESIS, bundle.operations, "Snapshot");
  return bundle;
}

// Page rule: one bundle for one internally chained interval, serialized as
// json-indent-2-newline so an append diffs as added lines plus the digest.
function parsePage(bytes, what) {
  const value = JSON.parse(bytes.toString("utf8"));
  if (!serialize(value).equals(bytes)) throw new Error(`${what} is not serialized as ${SERIALIZATION}.`);
  const bundle = parseProjectBundle(value);
  const first = bundle.operations[0];
  assertChain({ sequence: first.sequence - 1, operationSha256: first.parentOperationSha256 }, bundle.operations, what);
  return bundle;
}

function replayPages(slices) {
  const temporary = mkdtempSync(join(tmpdir(), "peqnp-ledger-replay-"));
  let oh;
  try {
    oh = Oh.open({ databasePath: join(temporary, "replay.sqlite"), spaceId: SPACE });
    let head = GENESIS;
    for (const operations of slices) {
      if (!operations.length) continue;
      oh.store.importOperations({ expectedHead: head, operations });
      head = oh.verify().head;
    }
    return oh.verify();
  } finally {
    oh?.store.close();
    rmSync(temporary, { recursive: true, force: true });
  }
}

function manifestV1(bytes, bundle, verification) {
  return {
    schema: SCHEMA_V1,
    space: SPACE,
    ohVersion: "0.4.3",
    contractSha256: CONTRACT_SHA256,
    bundle: { path: LEGACY_PATH, sha256: sha256Hex(bytes), bundleSha256: bundle.bundleSha256 },
    verification,
    interpretation: INTERPRETATION,
  };
}

function pageEntry(page, index) {
  const operations = page.operations;
  return {
    path: pagePath(index),
    sha256: sha256Hex(page.bytes),
    bundleSha256: page.bundle.bundleSha256,
    firstSequence: operations[0].sequence,
    lastSequence: operations[operations.length - 1].sequence,
    lastOperationSha256: operations[operations.length - 1].operationSha256,
    bytes: page.bytes.length,
  };
}

function manifestV2({ pages, verification, layout, migration }) {
  return {
    schema: SCHEMA_V2,
    space: SPACE,
    ohVersion: "0.4.3",
    contractSha256: CONTRACT_SHA256,
    layout,
    pages: pages.map((page, index) => pageEntry(page, index + 1)),
    ...(migration ? { migration } : {}),
    verification,
    interpretation: INTERPRETATION,
  };
}

function validateMigrationShape(migration) {
  const legacy = migration?.legacy;
  if (!migration || typeof migration !== "object" || Object.keys(migration).length !== 1 || !legacy || typeof legacy !== "object" ||
      Object.keys(legacy).length !== 3 || legacy.path !== LEGACY_PATH || !HEX64.test(legacy.sha256) || !HEX64.test(legacy.bundleSha256)) {
    throw new Error("Invalid migration record in the ledger manifest; inspect.");
  }
  return { legacy: { path: LEGACY_PATH, sha256: legacy.sha256, bundleSha256: legacy.bundleSha256 } };
}

// Every command classifies ledger/ into exactly one layout state first.
// S0: v1 manifest + legacy file. S1: S0 plus pages. S2: v2 manifest with
// migration + legacy + pages. S2b: S2 without the legacy file. S3: v2 manifest
// without migration + pages. "empty": nothing yet (a first export).
function classify(root) {
  const directory = resolve(root, "ledger");
  const folder = info(directory);
  if (folder && (!folder.isDirectory() || folder.isSymbolicLink())) throw new Error("ledger must be a real directory.");
  const legacyPath = join(directory, "operations.json");
  const manifestPath = join(directory, "manifest.json");
  const pagesDir = join(directory, "pages");
  const stagePath = join(directory, ".stage");
  const legacy = regularFile(legacyPath, MAX_LEGACY_BYTES, "ledger snapshot file " + LEGACY_PATH) !== null;
  const manifestFile = regularFile(manifestPath, MAX_MANIFEST_BYTES, "ledger manifest") !== null;
  const pagesFolder = info(pagesDir);
  if (pagesFolder && (!pagesFolder.isDirectory() || pagesFolder.isSymbolicLink())) throw new Error("ledger/pages must be a real directory.");
  const pages = pagesFolder !== null;
  const manifest = manifestFile ? JSON.parse(readBounded(manifestPath, MAX_MANIFEST_BYTES, "ledger manifest").toString("utf8")) : null;
  const unrecognized = reason => new Error(`unrecognized ledger layout; inspect (${reason}).`);
  let state;
  if (!manifestFile) {
    if (legacy || pages) throw new Error("Missing ledger manifest beside ledger history: " + unrecognized("manifest absent").message);
    state = "empty";
  } else if (manifest === null || typeof manifest !== "object" || Array.isArray(manifest)) {
    throw unrecognized("manifest is not a JSON object");
  } else if (manifest.schema === SCHEMA_V1) {
    if (!legacy) throw unrecognized("v1 manifest without " + LEGACY_PATH);
    if (["pages", "layout", "migration"].some(key => key in manifest)) throw unrecognized("v1 manifest with a v2 field");
    state = pages ? "S1" : "S0";
  } else if (manifest.schema === SCHEMA_V2) {
    if (!pages) throw unrecognized("v2 manifest without ledger/pages");
    if ("bundle" in manifest) throw unrecognized("v2 manifest with a v1 field");
    if (manifest.migration !== undefined) state = legacy ? "S2" : "S2b";
    else if (legacy) throw unrecognized("ambiguous layout: legacy bundle beside a v2 manifest without migration");
    else state = "S3";
  } else {
    throw unrecognized("unknown manifest schema");
  }
  return { state, root, directory, legacyPath, manifestPath, pagesDir, stagePath, manifest };
}

function resetStage(layout) { rmSync(layout.stagePath, { recursive: true, force: true }); }

function stageDirectory(layout, prefix) {
  mkdirSync(layout.stagePath, { recursive: true });
  return mkdtempSync(join(layout.stagePath, prefix + "-"));
}

// Atomic replacement: stage under ledger/.stage, write with wx, rename into place.
function writeAtomically(layout, files, { stopAfterWrites, prefix = "write" } = {}) {
  const written = [];
  if (!files.length) return written;
  const stage = stageDirectory(layout, prefix);
  for (const [path, content, relative] of files) {
    if (written.length === stopAfterWrites) throw new Error(`stopped after ${written.length} writes by the test hook.`);
    mkdirSync(dirname(path), { recursive: true });
    const staged = join(stage, basename(path));
    writeFileSync(staged, content, { flag: "wx", mode: 0o644 });
    renameSync(staged, path);
    written.push(relative);
  }
  resetStage(layout);
  return written;
}

// The page directory holds exactly the contiguous files 0001.json..NNNN.json.
function listPageFiles(pagesDir) {
  const entries = readdirSync(pagesDir, { withFileTypes: true }).sort((left, right) => left.name.localeCompare(right.name));
  entries.forEach((entry, index) => {
    if (!entry.isFile() || entry.name !== pageName(index + 1)) {
      throw new Error(`ledger/pages must contain exactly the contiguous page files; unexpected, gapped, or linked entry ${entry.name}.`);
    }
  });
  return entries.length;
}

function readPageFile(pagesDir, index) {
  const path = pagePath(index);
  const bytes = readBounded(join(pagesDir, pageName(index)), PAGE_HARD_BYTES, "page " + path);
  const bundle = parsePage(bytes, path);
  return { index, path, bytes, bundle, operations: bundle.operations, sha256: sha256Hex(bytes) };
}

// Reads pages 1..count, chaining across boundaries from genesis.
function readPageSequence(pagesDir, count) {
  const pages = [];
  let prior = GENESIS;
  for (let index = 1; index <= count; index += 1) {
    const page = readPageFile(pagesDir, index);
    prior = assertChain(prior, page.operations, page.path);
    pages.push(page);
  }
  return pages;
}

function validatePagedManifest(manifest) {
  const layout = validateLayout(manifest.layout);
  const listed = manifest.pages;
  if (!Array.isArray(listed) || !listed.length || listed.some(entry => !entry || typeof entry !== "object")) throw new Error("Ledger manifest lists no pages.");
  const operations = manifest.verification?.operations;
  if (!Number.isSafeInteger(operations) || operations < 1) throw new Error("Ledger manifest verification lacks an operation count.");
  return { layout, listed, operations };
}

// Reads the paged layout under a v2 manifest. Sealed pages must match their
// entries exactly. The last listed page may have grown and further files may
// follow it only as the tolerated remains of an interrupted export: the listed
// interval must still be an exact prefix (its lastOperationSha256 commits to
// content and parent chain) and every further file must continue the chain.
function readPagedLayout(layout) {
  const { manifest } = layout;
  const shape = validatePagedManifest(manifest);
  const listedCount = shape.listed.length;
  const onDisk = listPageFiles(layout.pagesDir);
  const pages = readPageSequence(layout.pagesDir, Math.max(onDisk, listedCount));
  const committed = [];
  const pending = [];
  let interrupted = false;
  for (const page of pages) {
    const entry = shape.listed[page.index - 1];
    if (!entry) {
      pending.push(...page.operations);
      page.committedCount = 0;
      interrupted = true;
      continue;
    }
    if (entry.path !== page.path) throw new Error(`Ledger manifest lists ${entry.path} where ${page.path} is required.`);
    if (same(entry, pageEntry(page, page.index))) {
      committed.push(...page.operations);
      page.committedCount = page.operations.length;
      continue;
    }
    if (page.index < listedCount) throw new Error(`sealed page ${page.path} changed; inspect.`);
    const first = page.operations[0];
    const cut = page.operations.findIndex(operation => operation.sequence === entry.lastSequence);
    if (entry.firstSequence !== first.sequence || cut < 0 || page.operations[cut].operationSha256 !== entry.lastOperationSha256 ||
        entry.sha256 === page.sha256) {
      throw new Error(`truncated or altered last page ${page.path}: the listed lastOperationSha256 is absent at lastSequence ${entry.lastSequence}; manifest and page disagree; inspect.`);
    }
    committed.push(...page.operations.slice(0, cut + 1));
    pending.push(...page.operations.slice(cut + 1));
    page.committedCount = cut + 1;
    interrupted = true;
  }
  if (committed.length !== shape.operations) throw new Error("Ledger manifest page intervals do not cover its verified operation count.");
  if (committed.length + pending.length > MAX_OPERATIONS) throw new Error("Ledger history exceeds 1,000 operations; a windowed export is a reviewed change.");
  return { ...shape, pages, committed, pending, interrupted };
}

function verifyLegacyAgainstMigration(layout, committed) {
  const migration = validateMigrationShape(layout.manifest.migration);
  const bytes = readBounded(layout.legacyPath, MAX_LEGACY_BYTES, "legacy ledger snapshot " + LEGACY_PATH);
  if (sha256Hex(bytes) !== migration.legacy.sha256) throw new Error("ledger/operations.json does not match migration.legacy.sha256; inspect.");
  const bundle = validateBundle(JSON.parse(bytes.toString("utf8")));
  if (bundle.bundleSha256 !== migration.legacy.bundleSha256 || !sameOperations(bundle.operations, committed)) {
    throw new Error("ledger/operations.json does not match the paged history recorded by migration.legacy; inspect.");
  }
  return migration;
}

// v1 rule, unchanged: one bundle, one replay. In S1 the pages beside it must
// concatenate to the same operations.
function checkLegacySnapshot(layout, { requireCurrentDocuments = true } = {}) {
  const bytes = readBounded(layout.legacyPath, MAX_LEGACY_BYTES, "ledger snapshot file " + LEGACY_PATH);
  const bundle = validateBundle(JSON.parse(bytes.toString("utf8")));
  admitPublicHistory(layout.root, bundle.operations, { requireCurrentDocuments });
  const verification = replayPages([bundle.operations]);
  if (!same(layout.manifest, manifestV1(bytes, bundle, verification))) {
    throw new Error("Snapshot manifest does not match bundle bytes and exact replayed head.");
  }
  let pages = [];
  if (layout.state === "S1") {
    pages = readPageSequence(layout.pagesDir, listPageFiles(layout.pagesDir));
    if (!sameOperations(pages.flatMap(page => page.operations), bundle.operations)) {
      throw new Error("ledger/pages does not match ledger/operations.json; inspect.");
    }
  }
  return {
    state: layout.state, bundle, manifest: layout.manifest, verification, sha256: sha256Hex(bytes),
    pageRecords: pages, pages: pages.map(page => page.path), slices: pages.length ? pages.map(page => page.operations) : [bundle.operations],
  };
}

function checkPagedSnapshot(layout, { requireCurrentDocuments = true } = {}) {
  inspectContract();
  const read = readPagedLayout(layout);
  if (read.interrupted) throw new Error("interrupted export; rerun oh:export");
  admitPublicHistory(layout.root, read.committed, { requireCurrentDocuments });
  const verification = replayPages(read.pages.map(page => page.operations));
  const migration = layout.state === "S2" ? verifyLegacyAgainstMigration(layout, read.committed)
    : layout.state === "S2b" ? validateMigrationShape(layout.manifest.migration) : undefined;
  if (!same(layout.manifest, manifestV2({ pages: read.pages, verification, layout: read.layout, migration }))) {
    throw new Error("Ledger manifest does not match page bytes and exact replayed head.");
  }
  return {
    state: layout.state, bundle: { operations: read.committed }, manifest: layout.manifest, verification,
    pageRecords: read.pages, pages: read.pages.map(page => page.path), slices: read.pages.map(page => page.operations),
  };
}

export function checkSnapshot(root, options = {}) {
  const layout = classify(root);
  if (layout.state === "empty") throw new Error("Missing ledger manifest and history under ledger/.");
  return layout.state === "S0" || layout.state === "S1" ? checkLegacySnapshot(layout, options) : checkPagedSnapshot(layout, options);
}

function assertPrefix(prefix, operations) {
  if (prefix.length > operations.length || prefix.some((operation, index) => !same(operation, operations[index]))) {
    throw new Error("Divergent ledger history; preserve both histories and reconcile explicitly.");
  }
}

function buildPage(operations) {
  const bundle = createOhSyncBundleV1(SPACE, operations);
  return { operations, bundle, bytes: serialize(bundle) };
}

// The paging rule: append while the rebuilt page stays within pageSoftBytes;
// otherwise seal and open a new page. A one-operation page may exceed the soft
// bound but never the hard bound. Deterministic in the history and constants.
export function paginate(current, operations, layout = LAYOUT) {
  const pages = [];
  let open = current.length ? buildPage(current.slice()) : null;
  for (const operation of operations) {
    const candidate = buildPage((open?.operations ?? []).concat([operation]));
    if (open && candidate.bytes.length > layout.pageSoftBytes) {
      pages.push(open);
      open = buildPage([operation]);
    } else {
      open = candidate;
    }
  }
  if (open) pages.push(open);
  pages.forEach((page, index) => {
    // The inherited committed page was accepted when it was written; the
    // hard bound applies to every page that holds an appended operation.
    const appended = index > 0 || page.operations.length > current.length;
    if (appended && page.operations.length === 1 && page.bytes.length > layout.pageHardBytes) {
      throw new Error(`operation ${page.operations[0].sequence} alone exceeds the ${layout.pageHardBytes}-byte page bound; split its ingestion per the edition size policy`);
    }
    if (page.bytes.length > PAGE_HARD_BYTES) throw new Error(`page of ${page.bytes.length} bytes exceeds the ${PAGE_HARD_BYTES}-byte read bound; inspect the layout.`);
  });
  return pages;
}

function refuseUnlessExportable(layout) {
  if (layout.state === "S0") throw new Error("ledger is in the legacy single-file layout; run oh:migrate before exporting.");
  if (layout.state === "S1" || layout.state === "S2" || layout.state === "S2b") throw new Error("ledger migration is incomplete; finish oh:migrate before exporting.");
}

// Recovery preflight: an interrupted earlier export is resumable. Sealed pages
// must match exactly; the committed history (the listed intervals) must replay
// to the manifest's verification; pending on-disk operations are checked
// against the local history by the caller.
function exportPreflight(layout) {
  inspectContract();
  const read = readPagedLayout(layout);
  const verification = replayPages(read.pages.map(page => page.operations.slice(0, page.committedCount)));
  const consistent = read.interrupted ? same(verification, layout.manifest.verification)
    : same(layout.manifest, manifestV2({ pages: read.pages, verification, layout: read.layout }));
  if (!consistent) throw new Error("Ledger manifest does not match page bytes and exact replayed head.");
  return read;
}

export function exportSnapshot(root, { layout: layoutOverride, stopAfterWrites } = {}) {
  const pageLayout = layoutOverride ? validateLayout(layoutOverride) : LAYOUT;
  const target = classify(root);
  resetStage(target);
  refuseUnlessExportable(target);
  const previous = target.state === "S3" ? exportPreflight(target) : null;
  const committed = previous?.committed ?? [];
  const pending = previous?.pending ?? [];
  const listedCount = previous?.listed.length ?? 0;
  const oh = openLedger(root);
  let generated, verification;
  try {
    verification = oh.verify();
    if (verification.operations > MAX_OPERATIONS) throw new Error("Snapshot exceeds 1,000 operations; a windowed export is a reviewed change.");
    const local = oh.store.exportOperations(0, MAX_OPERATIONS);
    if (local.length !== verification.operations) throw new Error("Operation export is incomplete.");
    assertPrefix(committed, local);
    if (committed.length + pending.length > local.length || pending.some((operation, index) => !same(operation, local[committed.length + index]))) {
      throw new Error("interrupted export left operations absent from the local history; inspect before retrying");
    }
    const history = validateBundle(createOhSyncBundleV1(SPACE, local)).operations;
    admitPublicHistory(root, history);
    admitUnpublishedDocuments(root, history, committed);
    const sealed = previous ? previous.pages.slice(0, listedCount - 1) : [];
    const current = previous ? previous.pages[listedCount - 1].operations.slice(0, previous.pages[listedCount - 1].committedCount) : [];
    generated = sealed.concat(paginate(current, history.slice(committed.length), pageLayout));
    let prior = GENESIS;
    generated.forEach((page, index) => {
      const parsed = index < sealed.length ? page.bundle : parsePage(page.bytes, pagePath(index + 1));
      if (parsed.bundleSha256 !== page.bundle.bundleSha256) throw new Error("Generated page digest mismatch.");
      prior = assertChain(prior, parsed.operations, pagePath(index + 1));
    });
    if (!same(replayPages(generated.map(page => page.operations)), verification)) throw new Error("Export failed exact replay equivalence.");
    if (!same(oh.verify(), verification)) throw new Error("Local ledger changed during export; inspect before retrying.");
  } finally { oh.store.close(); }
  if (previous && generated.length < previous.pages.length) throw new Error("pending page files exceed the regenerated layout; inspect.");
  const plan = [];
  generated.forEach((page, index) => {
    const number = index + 1;
    const path = join(target.pagesDir, pageName(number));
    const existing = info(path) ? readFileSync(path) : null;
    if (existing?.equals(page.bytes)) return;
    if (existing) {
      if (number < listedCount) throw new Error(`sealed page ${pagePath(number)} would change; inspect.`);
      if (number > listedCount) {
        const last = previous.pages[index].operations.at(-1);
        const regenerated = page.operations.find(operation => operation.sequence === last.sequence);
        if (regenerated?.operationSha256 !== last.operationSha256) throw new Error(`pending page ${pagePath(number)} does not match the regenerated layout; inspect.`);
      }
    }
    plan.push([path, page.bytes, pagePath(number)]);
  });
  const manifestBytes = serialize(manifestV2({ pages: generated, verification, layout: pageLayout }));
  if (!(info(target.manifestPath) && readFileSync(target.manifestPath).equals(manifestBytes))) plan.push([target.manifestPath, manifestBytes, MANIFEST_PATH]);
  const written = writeAtomically(target, plan, { stopAfterWrites, prefix: "export" });
  return { written, pages: generated.map((_, index) => pagePath(index + 1)), verification };
}

export function restoreSnapshot(root, { stopAfterImports } = {}) {
  // Validate the whole history in a disposable database before opening or
  // creating the selected runtime; then import the missing suffix one atomic
  // interval per page, so a crash between calls leaves an exact prefix.
  const snapshot = checkSnapshot(root, { requireCurrentDocuments: false });
  const history = snapshot.bundle.operations;
  const oh = openLedger(root, { initialize: true });
  try {
    const before = oh.verify();
    if (before.operations > MAX_OPERATIONS) throw new Error("Local ledger exceeds the restore comparison bound; preserve it and inspect.");
    const current = oh.store.exportOperations(0, MAX_OPERATIONS);
    if (current.length !== before.operations) throw new Error("Local history changed during restore preflight.");
    if (current.length >= history.length) {
      assertPrefix(history, current);
      return { imported: 0, status: current.length === history.length ? "already-present" : "local-ahead", verification: oh.verify() };
    }
    assertPrefix(current, history);
    let imported = 0, status = "already-present", calls = 0, head = before.head;
    for (const slice of snapshot.slices) {
      const missing = slice.filter(operation => operation.sequence > current.length);
      if (!missing.length) continue;
      if (calls === stopAfterImports) throw new Error(`restore stopped after ${calls} imports by the test hook.`);
      const result = oh.store.importOperations({ expectedHead: head, operations: missing });
      imported += result.imported;
      status = result.status;
      head = oh.verify().head;
      calls += 1;
    }
    const verification = oh.verify();
    if (!same(verification, snapshot.verification)) throw new Error("Restored state differs from the reviewed snapshot; preserve it and inspect.");
    return { imported, status, verification };
  } finally { oh.store.close(); }
}

// One-time, resumable migration from the single file to pages; .oh is never
// touched. Each step re-verifies on entry; every intermediate state is
// accepted by check, so a rerun from any state finishes the migration.
export function migrateSnapshot(root, { layout: layoutOverride, stopAfter } = {}) {
  const pageLayout = layoutOverride ? validateLayout(layoutOverride) : LAYOUT;
  let target = classify(root);
  resetStage(target);
  const initial = target.state;
  if (initial === "empty") throw new Error("Missing ledger manifest and history under ledger/; nothing to migrate.");
  const stop = step => { if (stopAfter === step) throw new Error(`migration stopped after ${step} by the test hook.`); };
  let legacyVerification = null;
  if (target.state === "S0") {
    const legacy = checkLegacySnapshot(target);
    legacyVerification = legacy.verification;
    const pages = paginate([], legacy.bundle.operations, pageLayout);
    const stage = stageDirectory(target, "migrate");
    const staged = join(stage, "pages");
    mkdirSync(staged);
    pages.forEach((page, index) => writeFileSync(join(staged, pageName(index + 1)), page.bytes, { flag: "wx", mode: 0o644 }));
    if (info(target.pagesDir)) throw new Error("ledger/pages already exists; inspect.");
    renameSync(staged, target.pagesDir);
    stop("pages");
    resetStage(target);
    target = classify(root);
  }
  if (target.state === "S1") {
    const legacy = checkLegacySnapshot(target);
    legacyVerification ??= legacy.verification;
    const manifest = manifestV2({
      pages: legacy.pageRecords, verification: legacy.verification, layout: pageLayout,
      migration: { legacy: { path: LEGACY_PATH, sha256: legacy.sha256, bundleSha256: legacy.bundle.bundleSha256 } },
    });
    writeAtomically(target, [[target.manifestPath, serialize(manifest), MANIFEST_PATH]], { prefix: "migrate" });
    stop("manifest");
    target = classify(root);
  }
  if (target.state === "S2") {
    checkPagedSnapshot(target);
    unlinkSync(target.legacyPath);
    stop("unlink");
    target = classify(root);
  }
  if (target.state === "S2b") {
    const checked = checkPagedSnapshot(target);
    const manifest = manifestV2({ pages: checked.pageRecords, verification: checked.verification, layout: validateLayout(target.manifest.layout) });
    writeAtomically(target, [[target.manifestPath, serialize(manifest), MANIFEST_PATH]], { prefix: "migrate" });
    target = classify(root);
  }
  if (target.state !== "S3") throw new Error("unrecognized ledger layout; inspect (migration did not reach the paged layout).");
  const final = checkPagedSnapshot(target);
  if (legacyVerification && !same(final.verification, legacyVerification)) throw new Error("Migrated verification differs from the legacy manifest; inspect.");
  return { status: initial === "S3" ? "already-migrated" : "migrated", pages: final.pages, verification: final.verification };
}

export function main(args = process.argv.slice(2)) {
  if (args.length !== 1) throw new Error("Usage: bun scripts/oh-snapshot.mjs <export|restore|check|migrate>");
  if (args[0] === "export") return exportSnapshot(repositoryRoot);
  if (args[0] === "restore") return restoreSnapshot(repositoryRoot);
  if (args[0] === "migrate") return migrateSnapshot(repositoryRoot);
  if (args[0] === "check") {
    const { state, pages, verification } = checkSnapshot(repositoryRoot);
    return { state, pages, verification };
  }
  throw new Error("Unknown snapshot command.");
}

if (import.meta.main) {
  try { process.stdout.write(canonicalJson(main()) + "\n"); }
  catch (error) { process.stderr.write(String(error.message ?? error) + "\n"); process.exitCode = 1; }
}
