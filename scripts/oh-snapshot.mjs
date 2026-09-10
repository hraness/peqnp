import { lstatSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { canonicalJson, createOhSyncBundleV1, parseOhSyncBundleV1, sha256Hex } from "@hraness/oh";
import { Oh } from "@hraness/oh/sdk";
import { CONTRACT_SHA256, PROFILE, SPACE, inspectContract, openLedger } from "./oh-ledger.mjs";
import { admitPublicHistory, admitUnpublishedDocuments } from "./oh-public-records.mjs";

// Reviewed bound on the pretty-printed bundle. Raised from 8 MiB to 16 MiB,
// the pinned runtime's own canonical-JSON parse bound, when extraction-cost-v1
// was admitted: its observation-part editions add about 5 MB of indented
// JSON to a bundle that already held 6.3 MB. Restore and check parse the
// whole file, so the bound stays a size review, never a silent truncation.
const MAX_SNAPSHOT_BYTES = 16 * 1024 * 1024;
const MAX_OPERATIONS = 1000;
const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const GENESIS = { sequence: 0, operationSha256: null };

function info(path) {
  try { return lstatSync(path); } catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}

function paths(root, create = false) {
  const directory = resolve(root, "ledger");
  const folder = info(directory);
  if (folder && (!folder.isDirectory() || folder.isSymbolicLink())) throw new Error("ledger must be a real directory.");
  if (!folder && create) mkdirSync(directory);
  for (const name of ["operations.json", "manifest.json"]) {
    const file = info(join(directory, name));
    if (file && (!file.isFile() || file.isSymbolicLink() || file.nlink !== 1)) throw new Error("Snapshot requires regular unlinked files.");
  }
  return { directory, bundlePath: join(directory, "operations.json"), manifestPath: join(directory, "manifest.json") };
}

function readBounded(path) {
  const file = info(path);
  if (!file?.isFile() || file.isSymbolicLink() || file.size > MAX_SNAPSHOT_BYTES) throw new Error("Missing or oversized ledger snapshot file.");
  const bytes = readFileSync(path);
  if (bytes.length > MAX_SNAPSHOT_BYTES) throw new Error("Snapshot grew beyond its bound.");
  return bytes;
}

function validateBundle(value) {
  inspectContract();
  const bundle = parseOhSyncBundleV1(value);
  if (!bundle || bundle.spaceId !== SPACE || bundle.contractSha256 !== CONTRACT_SHA256 || !bundle.operations.length) {
    throw new Error("Invalid public peqnp operation bundle or contract.");
  }
  let prior = GENESIS;
  for (const operation of bundle.operations) {
    if (operation.sequence !== prior.sequence + 1 || operation.parentOperationSha256 !== prior.operationSha256) {
      throw new Error("Snapshot must contain the complete ordered history from genesis.");
    }
    if (operation.actorId !== "peqnp.local-research" || !/^op_peqnp_[a-f0-9]{64}$/.test(operation.operationId) || operation.changes.some(change =>
      change.kind !== "put" || change.record.value.profile !== PROFILE)) {
      throw new Error("Snapshot contains an unsupported actor, mutation, or research profile; review its public scope.");
    }
    prior = operation;
  }
  return bundle;
}

function replay(bundle) {
  const temporary = mkdtempSync(join(tmpdir(), "peqnp-ledger-replay-"));
  let oh;
  try {
    oh = Oh.open({ databasePath: join(temporary, "replay.sqlite"), spaceId: SPACE });
    oh.store.importOperations({ expectedHead: GENESIS, operations: bundle.operations });
    return oh.verify();
  } finally {
    oh?.store.close();
    rmSync(temporary, { recursive: true, force: true });
  }
}

function manifestFor(bytes, bundle, verification) {
  return {
    schema: "peqnp.public-ledger.v1",
    space: SPACE,
    ohVersion: "0.4.3",
    contractSha256: CONTRACT_SHA256,
    bundle: { path: "ledger/operations.json", sha256: sha256Hex(bytes), bundleSha256: bundle.bundleSha256 },
    verification,
    interpretation: "Replay verifies record history and integrity, not mathematical truth or experimental provenance.",
  };
}

export function checkSnapshot(root, { requireCurrentDocuments = true } = {}) {
  const { bundlePath, manifestPath } = paths(root);
  const bytes = readBounded(bundlePath);
  const bundle = validateBundle(JSON.parse(bytes.toString("utf8")));
  const manifest = JSON.parse(readBounded(manifestPath).toString("utf8"));
  admitPublicHistory(root, bundle.operations, { requireCurrentDocuments });
  const verification = replay(bundle);
  if (canonicalJson(manifest) !== canonicalJson(manifestFor(bytes, bundle, verification))) {
    throw new Error("Snapshot manifest does not match bundle bytes and exact replayed head.");
  }
  return { bundle, manifest, verification };
}

function assertPrefix(prefix, operations) {
  if (prefix.length > operations.length || prefix.some((operation, index) =>
    canonicalJson(operation) !== canonicalJson(operations[index]))) {
    throw new Error("Divergent ledger history; preserve both histories and reconcile explicitly.");
  }
}

export function exportSnapshot(root) {
  const target = paths(root);
  const previous = info(target.bundlePath) || info(target.manifestPath)
    ? checkSnapshot(root, { requireCurrentDocuments: false }) : null;
  const oh = openLedger(root);
  let bundle, verification;
  try {
    verification = oh.verify();
    if (verification.operations > MAX_OPERATIONS) throw new Error("Snapshot exceeds 1,000 operations; implement reviewed pagination before exporting.");
    const operations = oh.store.exportOperations(0, MAX_OPERATIONS);
    if (operations.length !== verification.operations) throw new Error("Operation export is incomplete.");
    if (previous) assertPrefix(previous.bundle.operations, operations);
    bundle = validateBundle(createOhSyncBundleV1(SPACE, operations));
    admitPublicHistory(root, bundle.operations);
    admitUnpublishedDocuments(root, bundle.operations, previous?.bundle.operations ?? []);
    if (canonicalJson(oh.verify()) !== canonicalJson(verification)) throw new Error("Local ledger changed during export; inspect before retrying.");
  } finally { oh.store.close(); }
  const verifiedReplay = replay(bundle);
  if (canonicalJson(verifiedReplay) !== canonicalJson(verification)) throw new Error("Export failed exact replay equivalence.");
  const bytes = Buffer.from(JSON.stringify(bundle, null, 2) + "\n");
  if (bytes.length > MAX_SNAPSHOT_BYTES) throw new Error("Snapshot exceeds the adapter byte bound.");
  const manifest = manifestFor(bytes, bundle, verification);
  paths(root, true);
  // Each file is atomically replaced. An interruption between renames is detected
  // by the manifest check; preserve both files and inspect before recovery.
  for (const [path, content] of [[target.bundlePath, bytes], [target.manifestPath, JSON.stringify(manifest, null, 2) + "\n"]]) {
    if (info(path) && readFileSync(path).equals(Buffer.from(content))) continue;
    const temporary = mkdtempSync(join(target.directory, ".snapshot-"));
    try {
      const staged = join(temporary, "file");
      writeFileSync(staged, content, { flag: "wx", mode: 0o644 });
      renameSync(staged, path);
    } finally { rmSync(temporary, { recursive: true, force: true }); }
  }
  return { bundle: "ledger/operations.json", manifest: "ledger/manifest.json", verification };
}

export function restoreSnapshot(root) {
  // Validate the full bundle in a disposable database before opening/creating
  // the selected runtime. This uses the v0.4.3 atomic SDK import interval.
  const snapshot = checkSnapshot(root, { requireCurrentDocuments: false });
  const oh = openLedger(root, { initialize: true });
  try {
    const before = oh.verify();
    if (before.operations > MAX_OPERATIONS) throw new Error("Local ledger exceeds the restore comparison bound; preserve it and inspect.");
    const current = oh.store.exportOperations(0, MAX_OPERATIONS);
    if (current.length !== before.operations) throw new Error("Local history changed during restore preflight.");
    if (current.length >= snapshot.bundle.operations.length) {
      assertPrefix(snapshot.bundle.operations, current);
      return { imported: 0, status: current.length === snapshot.bundle.operations.length ? "already-present" : "local-ahead", verification: oh.verify() };
    }
    assertPrefix(current, snapshot.bundle.operations);
    const result = oh.store.importOperations({ expectedHead: before.head, operations: snapshot.bundle.operations.slice(current.length) });
    const verification = oh.verify();
    if (canonicalJson(verification) !== canonicalJson(snapshot.verification)) throw new Error("Restored state differs from the reviewed snapshot; preserve it and inspect.");
    return { imported: result.imported, status: result.status, verification };
  } finally { oh.store.close(); }
}

export function main(args = process.argv.slice(2)) {
  if (args.length !== 1) throw new Error("Usage: bun scripts/oh-snapshot.mjs <export|restore|check>");
  if (args[0] === "export") return exportSnapshot(repositoryRoot);
  if (args[0] === "restore") return restoreSnapshot(repositoryRoot);
  if (args[0] === "check") return { snapshot: "ledger/operations.json", verification: checkSnapshot(repositoryRoot).verification };
  throw new Error("Unknown snapshot command.");
}

if (import.meta.main) {
  try { process.stdout.write(canonicalJson(main()) + "\n"); }
  catch (error) { process.stderr.write(String(error.message ?? error) + "\n"); process.exitCode = 1; }
}
