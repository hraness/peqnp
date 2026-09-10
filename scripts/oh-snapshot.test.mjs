import { afterEach, expect, test } from "bun:test";
import { cpSync, existsSync, linkSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, renameSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { canonicalJson, createKnowledgeGraphRecordV1, createOhSyncBundleV1, sha256Hex } from "@hraness/oh";
import { CONTRACT_SHA256, PROFILE, SPACE, commitAdditive, initializeLedger, openLedger } from "./oh-ledger.mjs";
import { DOCUMENT_PATHS, recordDocuments } from "./oh-documents.mjs";
import { recordExtractionCost, recordFragmentInterface, recordImplicationCalibration, recordIndexedTransfer } from "./oh-experiment-records.mjs";
import { PAGE_HARD_BYTES, checkSnapshot, exportSnapshot, migrateSnapshot, paginate, restoreSnapshot } from "./oh-snapshot.mjs";

const roots = [];
function root() {
  const path = mkdtempSync(join(tmpdir(), "peqnp-snapshot-test-"));
  roots.push(path);
  for (const file of DOCUMENT_PATHS) {
    mkdirSync(dirname(join(path, file)), { recursive: true });
    writeFileSync(join(path, file), "Synthetic test document: " + file + "\n");
  }
  return path;
}
afterEach(() => { for (const path of roots.splice(0)) rmSync(path, { recursive: true, force: true }); });
// Bootstrap records two operations: the seed history and the document registry.
function bootstrap(path) { initializeLedger(path); return recordDocuments(path); }
function advanceDocs(path, value, index = 0) {
  const file = join(path, DOCUMENT_PATHS[index]);
  writeFileSync(file, readFileSync(file, "utf8") + value + "\n");
  return recordDocuments(path);
}
function copySnapshot(source, clone) {
  rmSync(join(clone, "ledger"), { recursive: true, force: true });
  for (const directory of ["ledger", "docs", "experiments"]) cpSync(join(source, directory), join(clone, directory), { recursive: true });
}
// Tiny synthetic pages: one operation per page (a one-operation page may exceed
// the soft bound), so every boundary case is reachable with a few operations.
const SERIALIZATION = "json-indent-2-newline";
const onePerPage = { pageSoftBytes: 1, pageHardBytes: PAGE_HARD_BYTES, serialization: SERIALIZATION };
const onePage = { pageSoftBytes: PAGE_HARD_BYTES, pageHardBytes: PAGE_HARD_BYTES, serialization: SERIALIZATION };
// Each exported document edition must match the current Markdown, so growing
// the published history by several operations exports after every edit.
function grow(path, count, layout = onePerPage) {
  let exported;
  for (let index = 0; index < count; index += 1) { advanceDocs(path, "edition " + index); exported = exportSnapshot(path, { layout }); }
  return exported;
}
function pages(path) { return readdirSync(join(path, "ledger/pages")).sort(); }
function page(path, index) { return join(path, "ledger/pages", String(index).padStart(4, "0") + ".json"); }
function ledgerBytes(path) {
  const entries = new Map();
  for (const file of ["manifest.json", "operations.json"]) if (existsSync(join(path, "ledger", file))) entries.set(file, readFileSync(join(path, "ledger", file)));
  if (existsSync(join(path, "ledger/pages"))) {
    for (const entry of readdirSync(join(path, "ledger/pages"), { withFileTypes: true }).sort((left, right) => left.name.localeCompare(right.name))) {
      entries.set("pages/" + entry.name, entry.isFile() ? readFileSync(join(path, "ledger/pages", entry.name)) : Buffer.from("<not a regular file>"));
    }
  }
  return entries;
}
function expectUnchanged(path, before) {
  const after = ledgerBytes(path);
  expect([...after.keys()]).toEqual([...before.keys()]);
  for (const [file, bytes] of before) expect(after.get(file).equals(bytes)).toBe(true);
}
function localOperations(path) {
  const oh = openLedger(path);
  try { return { operations: oh.store.exportOperations(0, 1000), verification: oh.verify() }; } finally { oh.store.close(); }
}
function failure(run) { try { run(); return null; } catch (error) { return error.message; } }
// The legacy single-file writer lives only here, to build v1 layouts for migration tests.
function writeLegacy(path) {
  const { operations, verification } = localOperations(path);
  const bundle = createOhSyncBundleV1(SPACE, operations);
  const bytes = Buffer.from(JSON.stringify(bundle, null, 2) + "\n");
  mkdirSync(join(path, "ledger"), { recursive: true });
  writeFileSync(join(path, "ledger/operations.json"), bytes);
  writeFileSync(join(path, "ledger/manifest.json"), JSON.stringify({
    schema: "peqnp.public-ledger.v1", space: SPACE, ohVersion: "0.4.3", contractSha256: CONTRACT_SHA256,
    bundle: { path: "ledger/operations.json", sha256: sha256Hex(bytes), bundleSha256: bundle.bundleSha256 },
    verification, interpretation: "Replay verifies record history and integrity, not mathematical truth or experimental provenance.",
  }, null, 2) + "\n");
  return { operations, verification };
}
function readManifest(path) { return JSON.parse(readFileSync(join(path, "ledger/manifest.json"), "utf8")); }
function writeManifest(path, manifest) { writeFileSync(join(path, "ledger/manifest.json"), JSON.stringify(manifest, null, 2) + "\n"); }
function rewritePage(file, alter) {
  const value = JSON.parse(readFileSync(file, "utf8"));
  alter(value);
  writeFileSync(file, JSON.stringify(value, null, 2) + "\n");
}

test("fresh clone replay across pages preserves complete identities, re-exports identical bytes, and never seeds an independent genesis", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source, { layout: onePerPage });
  const exported = grow(source, 1);
  expect(pages(source)).toEqual(["0001.json", "0002.json", "0003.json"]);
  const bytes = ledgerBytes(source);
  expect(exportSnapshot(source, { layout: onePerPage }).written).toEqual([]);
  expectUnchanged(source, bytes);
  const clone = root();
  copySnapshot(source, clone);
  const checked = checkSnapshot(clone);
  expect(checked.verification).toEqual(exported.verification);
  expect(checked.bundle.operations.map(operation => operation.sequence)).toEqual([1, 2, 3]);
  expect(existsSync(join(clone, ".oh"))).toBe(false);
  const restored = restoreSnapshot(clone);
  expect(restored.imported).toBe(3);
  expect(restored.verification).toEqual(exported.verification);
  expect(restoreSnapshot(clone).imported).toBe(0);
  expect(exportSnapshot(clone, { layout: onePerPage }).written).toEqual([]);
  expectUnchanged(clone, bytes);
});

test("an append extends the open page as an exact prefix and a new page leaves every sealed page byte-identical", () => {
  const path = root();
  bootstrap(path);
  exportSnapshot(path, { layout: onePage });
  expect(pages(path)).toEqual(["0001.json"]);
  const pair = readFileSync(page(path, 1)).length;
  advanceDocs(path, "third operation opens page two");
  exportSnapshot(path, { layout: { pageSoftBytes: pair, pageHardBytes: PAGE_HARD_BYTES, serialization: SERIALIZATION } });
  expect(pages(path)).toEqual(["0001.json", "0002.json"]);
  const sealed = readFileSync(page(path, 1));
  const open = JSON.parse(readFileSync(page(path, 2), "utf8"));
  expect(open.operations.map(operation => operation.sequence)).toEqual([3]);
  advanceDocs(path, "appended into the open page");
  const appended = exportSnapshot(path, { layout: onePage });
  expect(appended.written).toEqual(["ledger/pages/0002.json", "ledger/manifest.json"]);
  expect(readFileSync(page(path, 1)).equals(sealed)).toBe(true);
  const grown = JSON.parse(readFileSync(page(path, 2), "utf8"));
  expect(grown.operations.slice(0, 1)).toEqual(open.operations);
  expect(grown.operations.map(operation => operation.sequence)).toEqual([3, 4]);
  expect(readManifest(path).pages[1]).toMatchObject({ firstSequence: 3, lastSequence: 4 });
  expect(readManifest(path).layout).toEqual(onePage);
  const secondSealed = readFileSync(page(path, 2));
  advanceDocs(path, "opens a new page");
  const opened = exportSnapshot(path, { layout: onePerPage });
  expect(opened.written).toEqual(["ledger/pages/0003.json", "ledger/manifest.json"]);
  expect(readFileSync(page(path, 1)).equals(sealed)).toBe(true);
  expect(readFileSync(page(path, 2)).equals(secondSealed)).toBe(true);
  expect(checkSnapshot(path).verification).toEqual(opened.verification);
});

test("a candidate of exactly pageSoftBytes is accepted and one byte more opens a page", () => {
  const path = root();
  bootstrap(path);
  const operations = localOperations(path).operations;
  const exact = paginate([], operations).at(-1).bytes.length;
  const tie = paginate([], operations, { pageSoftBytes: exact, pageHardBytes: PAGE_HARD_BYTES, serialization: SERIALIZATION });
  expect(tie.map(entry => entry.operations.length)).toEqual([2]);
  expect(tie[0].bytes.length).toBe(exact);
  const split = paginate([], operations, { pageSoftBytes: exact - 1, pageHardBytes: PAGE_HARD_BYTES, serialization: SERIALIZATION });
  expect(split.map(entry => entry.operations.length)).toEqual([1, 1]);
  const layout = { pageSoftBytes: exact, pageHardBytes: PAGE_HARD_BYTES, serialization: SERIALIZATION };
  exportSnapshot(path, { layout });
  expect(readManifest(path).pages.map(entry => [entry.firstSequence, entry.lastSequence, entry.bytes])).toEqual([[1, 2, exact]]);
  advanceDocs(path, "one operation past the tie");
  exportSnapshot(path, { layout });
  expect(readManifest(path).pages.map(entry => [entry.firstSequence, entry.lastSequence])).toEqual([[1, 2], [3, 3]]);
});

test("edited Markdown requires explicit recording and export preserves older document history", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source, { layout: onePerPage });
  const original = checkSnapshot(source).bundle.operations;
  const file = join(source, DOCUMENT_PATHS[0]);
  writeFileSync(file, readFileSync(file, "utf8") + "reviewed update\n");
  expect(() => checkSnapshot(source)).toThrow("Markdown differs");
  expect(() => exportSnapshot(source, { layout: onePerPage })).toThrow("Markdown differs");
  recordDocuments(source);
  exportSnapshot(source, { layout: onePerPage });
  expect(checkSnapshot(source).bundle.operations.slice(0, original.length)).toEqual(original);
  expect(recordDocuments(source).inserted).toBe(0);
});

test("A to B to A creates a new registry activation while reusing the original edition", () => {
  const path = root();
  bootstrap(path);
  exportSnapshot(path, { layout: onePerPage });
  const file = join(path, DOCUMENT_PATHS[0]);
  const original = readFileSync(file);
  const first = checkSnapshot(path);
  advanceDocs(path, "B");
  exportSnapshot(path, { layout: onePerPage });
  writeFileSync(file, original);
  const reverted = recordDocuments(path);
  expect(reverted.inserted).toBe(1);
  exportSnapshot(path, { layout: onePerPage });
  const final = checkSnapshot(path);
  expect(final.verification.operations).toBe(first.verification.operations + 2);
  expect(final.pages.length).toBe(first.pages.length + 2);
  expect(recordDocuments(path).inserted).toBe(0);
});

test("restore fast-forwards an exact prefix and explicitly preserves unpublished local-ahead state", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source, { layout: onePerPage });
  const clone = root();
  copySnapshot(source, clone);
  restoreSnapshot(clone);
  grow(source, 1);
  copySnapshot(source, clone);
  expect(restoreSnapshot(clone).imported).toBe(1);
  advanceDocs(clone, "unpublished local edition");
  const newer = restoreSnapshot(clone);
  expect(newer.imported).toBe(0);
  expect(newer.status).toBe("local-ahead");
  expect(newer.verification.operations).toBe(4);
});

test("clone restore imports only the missing suffix one atomic interval per page and a crash between calls leaves an exact prefix", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source, { layout: onePerPage });
  const clone = root();
  copySnapshot(source, clone);
  expect(restoreSnapshot(clone).imported).toBe(2);
  const exported = grow(source, 3);
  expect(pages(source).length).toBe(5);
  copySnapshot(source, clone);
  const files = ledgerBytes(clone);
  const history = checkSnapshot(clone).bundle.operations;
  expect(() => restoreSnapshot(clone, { stopAfterImports: 1 })).toThrow("test hook");
  const prefix = localOperations(clone).operations;
  expect(prefix.map(operation => canonicalJson(operation))).toEqual(history.slice(0, 3).map(operation => canonicalJson(operation)));
  const resumed = restoreSnapshot(clone);
  expect(resumed.imported).toBe(2);
  expect(resumed.verification).toEqual(exported.verification);
  expectUnchanged(clone, files);
  expect(restoreSnapshot(clone).status).toBe("already-present");
});

test("local-ahead and divergent histories at a page boundary leave files and database unchanged on refusal", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source, { layout: onePerPage });
  const clone = root();
  copySnapshot(source, clone);
  restoreSnapshot(clone);
  advanceDocs(clone, "ahead of the published boundary");
  const ahead = restoreSnapshot(clone);
  expect(ahead.status).toBe("local-ahead");
  expect(ahead.imported).toBe(0);
  grow(source, 1);
  copySnapshot(source, clone);
  const files = ledgerBytes(clone);
  const before = localOperations(clone).verification;
  expect(() => restoreSnapshot(clone)).toThrow("Divergent");
  expect(() => exportSnapshot(clone, { layout: onePerPage })).toThrow("Divergent");
  expectUnchanged(clone, files);
  expect(localOperations(clone).verification).toEqual(before);
});

test("tampered, wrong-space, wrong-contract, reserialized and broken-chain pages fail before destination creation", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source, { layout: onePerPage });
  grow(source, 1);
  const clone = root();
  copySnapshot(source, clone);
  const first = page(clone, 1);
  const original = readFileSync(first);
  for (const [alter, message] of [
    [value => { value.operations[0].actorId = "tampered"; }, "Invalid public peqnp operation bundle"],
    [value => { value.spaceId = "other-project"; }, "Invalid public peqnp operation bundle"],
    [value => { value.contractSha256 = "0".repeat(64); }, "Invalid public peqnp operation bundle"],
    [value => { Object.assign(value, createOhSyncBundleV1(SPACE, JSON.parse(readFileSync(page(clone, 2), "utf8")).operations)); }, "complete ordered history"],
  ]) {
    rewritePage(first, alter);
    expect(() => restoreSnapshot(clone)).toThrow(message);
    expect(existsSync(join(clone, ".oh"))).toBe(false);
    writeFileSync(first, original);
  }
  writeFileSync(first, JSON.stringify(JSON.parse(original)));
  expect(() => restoreSnapshot(clone)).toThrow("not serialized as json-indent-2-newline");
  writeFileSync(first, original);
  renameSync(page(clone, 2), join(clone, "ledger/pages/swap.json"));
  renameSync(page(clone, 3), page(clone, 2));
  renameSync(join(clone, "ledger/pages/swap.json"), page(clone, 3));
  expect(() => restoreSnapshot(clone)).toThrow("complete ordered history");
  expect(existsSync(join(clone, ".oh"))).toBe(false);
  expect(checkSnapshot(source).verification.operations).toBe(3);
});

test("partial manifests and linked snapshot paths fail closed without database or snapshot replacement", () => {
  const source = root();
  expect(() => restoreSnapshot(source)).toThrow("Missing");
  expect(existsSync(join(source, ".oh"))).toBe(false);
  bootstrap(source);
  exportSnapshot(source, { layout: onePerPage });
  const clone = root();
  copySnapshot(source, clone);
  unlinkSync(join(clone, "ledger/manifest.json"));
  const bytes = ledgerBytes(clone);
  expect(() => restoreSnapshot(clone)).toThrow("Missing");
  expect(() => exportSnapshot(clone)).toThrow("Missing");
  expectUnchanged(clone, bytes);
  expect(existsSync(join(clone, ".oh"))).toBe(false);
  const linked = root();
  symlinkSync(join(source, "ledger"), join(linked, "ledger"));
  expect(() => checkSnapshot(linked)).toThrow("real directory");
});

test("every paged failure mode fails closed with .oh absent and every file byte-identical", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source, { layout: onePerPage });
  grow(source, 2);
  expect(pages(source).length).toBe(4);
  const clone = root();
  const cases = [
    ["oversized manifest", () => { const manifest = readManifest(clone); manifest.padding = "x".repeat(70000); writeManifest(clone, manifest); }, "oversized"],
    ["missing last page", () => unlinkSync(page(clone, 4)), "Missing page"],
    ["missing middle page", () => unlinkSync(page(clone, 2)), "contiguous"],
    ["gapped page", () => renameSync(page(clone, 4), page(clone, 5)), "contiguous"],
    ["unlisted page that does not chain", () => cpSync(page(clone, 2), page(clone, 5)), "complete ordered history"],
    ["symlinked page", () => { unlinkSync(page(clone, 2)); symlinkSync(page(source, 2), page(clone, 2)); }, "contiguous"],
    ["multiply linked page", () => linkSync(page(clone, 2), join(clone, "extra-link.json")), "regular, unlinked"],
    ["page 1 not at genesis", () => {
      unlinkSync(page(clone, 1));
      for (const index of [2, 3, 4]) renameSync(page(clone, index), page(clone, index - 1));
      const manifest = readManifest(clone);
      manifest.pages.shift();
      manifest.pages.forEach((entry, index) => { entry.path = "ledger/pages/000" + (index + 1) + ".json"; });
      writeManifest(clone, manifest);
    }, "complete ordered history"],
    ["reordered pages", () => { renameSync(page(clone, 2), join(clone, "swap.json")); renameSync(page(clone, 3), page(clone, 2)); renameSync(join(clone, "swap.json"), page(clone, 3)); }, "complete ordered history"],
    ["bundleSha256 mismatch", () => { const manifest = readManifest(clone); manifest.pages[1].bundleSha256 = "0".repeat(64); writeManifest(clone, manifest); }, "sealed page"],
    ["file sha256 mismatch", () => { const manifest = readManifest(clone); manifest.pages[0].sha256 = "0".repeat(64); writeManifest(clone, manifest); }, "sealed page"],
    ["sealed page bytes changed", () => rewritePage(page(clone, 1), value => { value.operations[0].changes[0].record.value.body = "changed"; }), "Invalid public peqnp operation bundle"],
    ["truncated last page", () => { const manifest = readManifest(clone); manifest.pages[3].lastOperationSha256 = "0".repeat(64); writeManifest(clone, manifest); }, "truncated or altered last page"],
    ["last page listing altered without growth", () => { const manifest = readManifest(clone); manifest.pages[3].bytes += 1; writeManifest(clone, manifest); }, "truncated or altered last page"],
    ["verification tampered", () => { const manifest = readManifest(clone); manifest.verification.records += 1; writeManifest(clone, manifest); }, "manifest does not match"],
    ["layout tampered", () => { const manifest = readManifest(clone); manifest.layout.serialization = "compact"; writeManifest(clone, manifest); }, "Invalid ledger page layout"],
    ["legacy bundle beside a v2 manifest without migration", () => writeFileSync(join(clone, "ledger/operations.json"), readFileSync(page(clone, 1))), "unrecognized ledger layout"],
    ["missing manifest with pages", () => unlinkSync(join(clone, "ledger/manifest.json")), "unrecognized ledger layout"],
    ["manifest with a v1 field", () => { const manifest = readManifest(clone); manifest.bundle = {}; writeManifest(clone, manifest); }, "unrecognized ledger layout"],
    ["manifest with an unknown schema", () => { const manifest = readManifest(clone); manifest.schema = "peqnp.public-ledger.v3"; writeManifest(clone, manifest); }, "unrecognized ledger layout"],
    ["stray entry in pages", () => mkdirSync(join(clone, "ledger/pages/notes")), "contiguous"],
    ["operation count beyond the listed intervals", () => { const manifest = readManifest(clone); manifest.verification.operations = 5; writeManifest(clone, manifest); }, "do not cover"],
  ];
  for (const [name, corrupt, message] of cases) {
    rmSync(join(clone, "extra-link.json"), { force: true });
    copySnapshot(source, clone);
    corrupt();
    const files = ledgerBytes(clone);
    expect([name, failure(() => checkSnapshot(clone))]).toEqual([name, expect.stringContaining(message)]);
    expect([name, failure(() => restoreSnapshot(clone))]).toEqual([name, expect.stringContaining(message)]);
    expect([name, failure(() => exportSnapshot(clone, { layout: onePerPage }))]).toEqual([name, expect.any(String)]);
    expect([name, existsSync(join(clone, ".oh"))]).toEqual([name, false]);
    expectUnchanged(clone, files);
  }
});

test("an operation whose one-operation page exceeds the hard bound is refused before any write", () => {
  const path = root();
  bootstrap(path);
  exportSnapshot(path, { layout: onePerPage });
  const files = ledgerBytes(path);
  advanceDocs(path, "an operation too large for a tiny hard bound");
  const before = localOperations(path).verification;
  expect(() => exportSnapshot(path, { layout: { pageSoftBytes: 1, pageHardBytes: 1, serialization: SERIALIZATION } }))
    .toThrow("operation 3 alone exceeds the 1-byte page bound; split its ingestion per the edition size policy");
  expectUnchanged(path, files);
  expect(existsSync(join(path, "ledger/.stage"))).toBe(false);
  expect(localOperations(path).verification).toEqual(before);
});

test("an interrupted export is detected by check and completed idempotently by the rerun", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source, { layout: onePerPage });
  const committed = ledgerBytes(source);
  advanceDocs(source, "third operation", 0);
  advanceDocs(source, "fourth operation", 1);
  const complete = exportSnapshot(source, { layout: onePerPage });
  expect(complete.written).toEqual(["ledger/pages/0003.json", "ledger/pages/0004.json", "ledger/manifest.json"]);
  const expected = ledgerBytes(source);
  const rewind = path => {
    rmSync(join(path, "ledger"), { recursive: true, force: true });
    mkdirSync(join(path, "ledger/pages"), { recursive: true });
    for (const [file, bytes] of committed) writeFileSync(join(path, "ledger", file), bytes);
  };
  for (const stopAfterWrites of [0, 1, 2]) {
    rewind(source);
    expect(() => exportSnapshot(source, { layout: onePerPage, stopAfterWrites })).toThrow("test hook");
    expect(existsSync(join(source, "ledger/.stage"))).toBe(true);
    if (stopAfterWrites === 0) expect(checkSnapshot(source, { requireCurrentDocuments: false }).verification.operations).toBe(2);
    else expect(() => checkSnapshot(source)).toThrow("interrupted export; rerun oh:export");
    const resumed = exportSnapshot(source, { layout: onePerPage });
    expect(resumed.written).toEqual(["ledger/pages/0003.json", "ledger/pages/0004.json", "ledger/manifest.json"].slice(stopAfterWrites));
    expect(resumed.verification).toEqual(complete.verification);
    expect(existsSync(join(source, "ledger/.stage"))).toBe(false);
    expectUnchanged(source, expected);
  }
  // A grown open page is also an interruption: the listed interval stays an exact prefix.
  const grownSource = root();
  bootstrap(grownSource);
  exportSnapshot(grownSource, { layout: onePage });
  const single = readManifest(grownSource);
  advanceDocs(grownSource, "grows the open page");
  exportSnapshot(grownSource, { layout: onePage });
  const full = ledgerBytes(grownSource);
  writeManifest(grownSource, single);
  expect(() => checkSnapshot(grownSource)).toThrow("interrupted export; rerun oh:export");
  expect(exportSnapshot(grownSource, { layout: onePage }).written).toEqual(["ledger/manifest.json"]);
  expectUnchanged(grownSource, full);
  // A local history that diverges from the pending files is refused without writes.
  const diverged = root();
  rewind(source);
  copySnapshot(source, diverged);
  restoreSnapshot(diverged);
  advanceDocs(diverged, "a different third operation");
  for (const [file, bytes] of expected) if (file.startsWith("pages/")) writeFileSync(join(diverged, "ledger", file), bytes);
  expect(() => checkSnapshot(diverged)).toThrow("interrupted export; rerun oh:export");
  const files = ledgerBytes(diverged);
  const before = localOperations(diverged).verification;
  expect(before.operations).toBe(3);
  expect(() => exportSnapshot(diverged, { layout: onePerPage })).toThrow("interrupted export left operations absent from the local history; inspect before retrying");
  expectUnchanged(diverged, files);
  expect(existsSync(join(diverged, "ledger/.stage"))).toBe(false);
  expect(localOperations(diverged).verification).toEqual(before);
});

test("migration of a legacy layout yields the same verification and operations, removes the legacy file, and is idempotent", () => {
  const path = root();
  bootstrap(path);
  advanceDocs(path, "third operation", 0);
  advanceDocs(path, "fourth operation", 1);
  const legacy = writeLegacy(path);
  expect(checkSnapshot(path).state).toBe("S0");
  expect(() => exportSnapshot(path, { layout: onePerPage })).toThrow("run oh:migrate");
  const migrated = migrateSnapshot(path, { layout: onePerPage });
  expect(migrated.status).toBe("migrated");
  expect(migrated.pages).toEqual(["ledger/pages/0001.json", "ledger/pages/0002.json", "ledger/pages/0003.json", "ledger/pages/0004.json"]);
  expect(canonicalJson(migrated.verification)).toBe(canonicalJson(legacy.verification));
  expect(existsSync(join(path, "ledger/operations.json"))).toBe(false);
  expect(existsSync(join(path, "ledger/.stage"))).toBe(false);
  const checked = checkSnapshot(path);
  expect(checked.state).toBe("S3");
  expect(checked.bundle.operations.map(operation => canonicalJson(operation))).toEqual(legacy.operations.map(operation => canonicalJson(operation)));
  expect(readManifest(path).migration).toBeUndefined();
  const files = ledgerBytes(path);
  expect(migrateSnapshot(path, { layout: onePerPage }).status).toBe("already-migrated");
  expectUnchanged(path, files);
  expect(exportSnapshot(path, { layout: onePerPage }).written).toEqual([]);
  expectUnchanged(path, files);
  const clone = root();
  copySnapshot(path, clone);
  expect(restoreSnapshot(clone).verification).toEqual(legacy.verification);
});

test("migration interrupted after each step is accepted by check, refused by export, and completed identically by the rerun", () => {
  const source = root();
  bootstrap(source);
  advanceDocs(source, "third operation", 0);
  advanceDocs(source, "fourth operation", 1);
  writeLegacy(source);
  const reference = root();
  copySnapshot(source, reference);
  migrateSnapshot(reference, { layout: onePerPage });
  const expected = ledgerBytes(reference);
  for (const [stopAfter, state, legacyPresent] of [["pages", "S1", true], ["manifest", "S2", true], ["unlink", "S2b", false]]) {
    const path = root();
    copySnapshot(source, path);
    expect(() => migrateSnapshot(path, { layout: onePerPage, stopAfter })).toThrow("test hook");
    const checked = checkSnapshot(path);
    expect(checked.state).toBe(state);
    expect(checked.pages.length).toBe(4);
    expect(existsSync(join(path, "ledger/operations.json"))).toBe(legacyPresent);
    if (state !== "S1") expect(readManifest(path).migration.legacy.path).toBe("ledger/operations.json");
    expect(() => exportSnapshot(path, { layout: onePerPage })).toThrow("finish oh:migrate");
    const clone = root();
    copySnapshot(path, clone);
    expect(restoreSnapshot(clone).imported).toBe(4);
    expect(migrateSnapshot(path, { layout: onePerPage }).status).toBe("migrated");
    expectUnchanged(path, expected);
    expect(existsSync(join(path, "ledger/.stage"))).toBe(false);
  }
});

test("migration refuses pages that do not equal the legacy operations, a legacy file that does not match its record, and an unrecognized layout", () => {
  const path = root();
  bootstrap(path);
  advanceDocs(path, "third operation", 0);
  advanceDocs(path, "fourth operation", 1);
  writeLegacy(path);
  expect(() => migrateSnapshot(path, { layout: onePerPage, stopAfter: "pages" })).toThrow("test hook");
  const second = readFileSync(page(path, 2));
  rewritePage(page(path, 2), value => { value.operations[0].changes[0].record.value.body = "changed after paging"; });
  const altered = ledgerBytes(path);
  expect(() => migrateSnapshot(path, { layout: onePerPage })).toThrow("Invalid public peqnp operation bundle");
  expectUnchanged(path, altered);
  writeFileSync(page(path, 2), second);
  unlinkSync(page(path, 4));
  const truncated = ledgerBytes(path);
  expect(() => migrateSnapshot(path, { layout: onePerPage })).toThrow("ledger/pages does not match ledger/operations.json; inspect");
  expect(() => checkSnapshot(path)).toThrow("ledger/pages does not match ledger/operations.json; inspect");
  expectUnchanged(path, truncated);
  const manifest = readManifest(path);
  manifest.layout = onePerPage;
  writeManifest(path, manifest);
  const unrecognized = ledgerBytes(path);
  expect(() => migrateSnapshot(path, { layout: onePerPage })).toThrow("unrecognized ledger layout; inspect");
  expect(() => checkSnapshot(path)).toThrow("unrecognized ledger layout; inspect");
  expectUnchanged(path, unrecognized);
  expect(existsSync(join(path, "ledger/.stage"))).toBe(false);
  const recorded = root();
  bootstrap(recorded);
  writeLegacy(recorded);
  expect(() => migrateSnapshot(recorded, { layout: onePerPage, stopAfter: "manifest" })).toThrow("test hook");
  expect(checkSnapshot(recorded).state).toBe("S2");
  writeFileSync(join(recorded, "ledger/operations.json"), readFileSync(join(recorded, "ledger/operations.json"), "utf8").replace("Synthetic test document", "Altered legacy"));
  const mismatched = ledgerBytes(recorded);
  expect(() => checkSnapshot(recorded)).toThrow("does not match migration.legacy.sha256");
  expect(() => migrateSnapshot(recorded, { layout: onePerPage })).toThrow("does not match migration.legacy.sha256");
  expectUnchanged(recorded, mismatched);
});

test("a leftover ledger/.stage is ignored by check and removed by export and migrate", () => {
  const path = root();
  bootstrap(path);
  exportSnapshot(path, { layout: onePerPage });
  const stage = join(path, "ledger/.stage/export-leftover");
  mkdirSync(stage, { recursive: true });
  writeFileSync(join(stage, "0009.json"), "not a page\n");
  expect(checkSnapshot(path).verification.operations).toBe(2);
  expect(existsSync(stage)).toBe(true);
  expect(exportSnapshot(path, { layout: onePerPage }).written).toEqual([]);
  expect(existsSync(join(path, "ledger/.stage"))).toBe(false);
  const legacy = root();
  bootstrap(legacy);
  writeLegacy(legacy);
  mkdirSync(join(legacy, "ledger/.stage/migrate-leftover/pages"), { recursive: true });
  expect(checkSnapshot(legacy).state).toBe("S0");
  expect(migrateSnapshot(legacy, { layout: onePerPage }).status).toBe("migrated");
  expect(existsSync(join(legacy, "ledger/.stage"))).toBe(false);
});

test("a profile-labeled private note in history cannot enter the public snapshot", () => {
  const path = root();
  bootstrap(path);
  exportSnapshot(path, { layout: onePerPage });
  const files = ledgerBytes(path);
  const oh = openLedger(path);
  let after;
  try {
    commitAdditive(oh, [createKnowledgeGraphRecordV1({ v: 1, key: "context:private-test-note", kind: "context", dependencies: [],
      value: JSON.parse(canonicalJson({ profile: PROFILE, body: "Private synthetic test input, not for publication." })) })], "private-test");
    after = oh.verify();
  } finally { oh.store.close(); }
  expect(() => exportSnapshot(path, { layout: onePerPage })).toThrow("Unreviewed or unrelated");
  expectUnchanged(path, files);
  const reopened = openLedger(path);
  expect(reopened.verify()).toEqual(after);
  reopened.store.close();
});

test("clean latest Markdown cannot conceal an unreviewed intermediate private edition", () => {
  const path = root();
  bootstrap(path);
  exportSnapshot(path, { layout: onePerPage });
  const files = ledgerBytes(path);
  const file = join(path, DOCUMENT_PATHS[0]);
  const original = readFileSync(file);
  advanceDocs(path, "Synthetic private intermediate content");
  writeFileSync(file, original);
  recordDocuments(path);
  const before = localOperations(path).verification;
  expect(() => exportSnapshot(path, { layout: onePerPage })).toThrow("Unreviewed intermediate document edition");
  expectUnchanged(path, files);
  expect(localOperations(path).verification).toEqual(before);
});

// The four newest experiments enter the public snapshot only through their
// reviewed constructors; a forged activity under the same prefix is refused.
function experimentFixture(path) {
  mkdirSync(join(path, "artifacts"), { recursive: true });
  mkdirSync(join(path, "src"), { recursive: true });
  for (const file of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]) writeFileSync(join(path, file), "# synthetic source fixture\n");
  writeFileSync(join(path, "src/lib.rs"), "// Synthetic source fixture, not experimental provenance.\n");
  for (const [artifact, protocol] of [
    ["indexed-transfer.json", "indexed-transfer-protocol.md"],
    ["implication-calibration.json", "implication-protocol.md"],
    ["fragment-interface.json", "fragment-interface-protocol.md"],
    ["extraction-cost.json", "extraction-cost-protocol.md"],
  ]) {
    writeFileSync(join(path, "artifacts", artifact), readFileSync(new URL("../artifacts/" + artifact, import.meta.url)));
    writeFileSync(join(path, "experiments", protocol), readFileSync(new URL("../experiments/" + protocol, import.meta.url)));
  }
}

test("indexed-transfer, implication-calibration, fragment-interface and extraction-cost observations are admitted through the allowlist and forged activities are refused", () => {
  const path = root();
  experimentFixture(path);
  bootstrap(path);
  const indexed = recordIndexedTransfer(path, "artifacts/indexed-transfer.json");
  const implication = recordImplicationCalibration(path, "artifacts/implication-calibration.json");
  const fragment = recordFragmentInterface(path, "artifacts/fragment-interface.json");
  const extraction = recordExtractionCost(path, "artifacts/extraction-cost.json");
  expect(indexed.inserted).toBe(5);
  expect(implication.inserted).toBe(5);
  expect(fragment.inserted).toBe(5);
  // The extraction report is stored as one edition plus observation parts.
  expect(extraction.inserted).toBeGreaterThan(5);
  // The code constants page this history: the extraction-cost operation alone exceeds the soft bound.
  const exported = exportSnapshot(path);
  expect(exported.pages.length).toBeGreaterThan(1);
  expect(readManifest(path).layout).toEqual({ pageSoftBytes: 4194304, pageHardBytes: 8388608, serialization: SERIALIZATION });
  expect(checkSnapshot(path).verification).toEqual(exported.verification);
  const clone = root();
  experimentFixture(clone);
  copySnapshot(path, clone);
  expect(restoreSnapshot(clone).verification).toEqual(exported.verification);
  const files = ledgerBytes(path);
  const editions = [...files.entries()].filter(([name]) => name.startsWith("pages/")).flatMap(([, bytes]) => JSON.parse(bytes).operations)
    .flatMap(operation => operation.changes).map(change => change.record)
    .filter(record => record.kind === "edition" && record.value.mediaType === "application/json").map(record => record.key);
  expect(editions).toEqual(expect.arrayContaining([
    "edition:indexed-transfer-" + indexed.reportSha256,
    "edition:implication-calibration-" + implication.reportSha256,
    "edition:fragment-interface-" + fragment.reportSha256,
    "edition:extraction-cost-" + extraction.reportSha256,
  ]));
  const oh = openLedger(path);
  let after;
  try {
    const activities = oh.list({ kind: "activity", limit: 20 });
    const forged = ["activity:indexed-transfer-", "activity:fragment-interface-", "activity:extraction-cost-"].map(prefix => {
      const genuine = activities.find(record => record.key.startsWith(prefix));
      return createKnowledgeGraphRecordV1({ v: 1, key: prefix + "f".repeat(64), kind: "activity", dependencies: genuine.dependencies,
        value: JSON.parse(canonicalJson({ ...genuine.value, authority: "Forged synthetic activity, not a reviewed ingestion." })) });
    });
    commitAdditive(oh, forged, "forged-test");
    after = oh.verify();
  } finally { oh.store.close(); }
  expect(() => exportSnapshot(path)).toThrow("Unreviewed or unrelated");
  expectUnchanged(path, files);
  const reopened = openLedger(path);
  expect(reopened.verify()).toEqual(after);
  reopened.store.close();
}, 120_000);
