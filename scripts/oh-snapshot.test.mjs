import { afterEach, expect, test } from "bun:test";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { canonicalJson, createKnowledgeGraphRecordV1, createOhSyncBundleV1 } from "@hraness/oh";
import { PROFILE, SPACE, commitAdditive, initializeLedger, openLedger } from "./oh-ledger.mjs";
import { DOCUMENT_PATHS, recordDocuments } from "./oh-documents.mjs";
import { recordImplicationCalibration, recordIndexedTransfer } from "./oh-experiment-records.mjs";
import { checkSnapshot, exportSnapshot, restoreSnapshot } from "./oh-snapshot.mjs";

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
function bootstrap(path) { initializeLedger(path); return recordDocuments(path); }
function advanceDocs(path, value) {
  const file = join(path, DOCUMENT_PATHS[0]);
  writeFileSync(file, readFileSync(file, "utf8") + value + "\n");
  return recordDocuments(path);
}
function copySnapshot(source, clone) {
  for (const directory of ["ledger", "docs", "experiments"]) cpSync(join(source, directory), join(clone, directory), { recursive: true });
}

test("fresh clone replay preserves complete identities and never seeds an independent genesis", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source);
  advanceDocs(source, "second document edition");
  const exported = exportSnapshot(source);
  const bytes = readFileSync(join(source, "ledger/operations.json"));
  exportSnapshot(source);
  expect(readFileSync(join(source, "ledger/operations.json"))).toEqual(bytes);
  const clone = root();
  copySnapshot(source, clone);
  expect(checkSnapshot(clone).verification).toEqual(exported.verification);
  expect(existsSync(join(clone, ".oh"))).toBe(false);
  const restored = restoreSnapshot(clone);
  expect(restored.imported).toBe(3);
  expect(restored.verification).toEqual(exported.verification);
  expect(restoreSnapshot(clone).imported).toBe(0);
  exportSnapshot(clone);
  expect(readFileSync(join(clone, "ledger/operations.json"))).toEqual(bytes);
});

test("edited Markdown requires explicit recording and export preserves older document history", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source);
  const original = checkSnapshot(source).bundle.operations;
  const file = join(source, DOCUMENT_PATHS[0]);
  writeFileSync(file, readFileSync(file, "utf8") + "reviewed update\n");
  expect(() => checkSnapshot(source)).toThrow("Markdown differs");
  expect(() => exportSnapshot(source)).toThrow("Markdown differs");
  recordDocuments(source);
  exportSnapshot(source);
  expect(checkSnapshot(source).bundle.operations.slice(0, original.length)).toEqual(original);
  expect(recordDocuments(source).inserted).toBe(0);
});

test("A to B to A creates a new registry activation while reusing the original edition", () => {
  const path = root();
  bootstrap(path);
  exportSnapshot(path);
  const file = join(path, DOCUMENT_PATHS[0]);
  const original = readFileSync(file);
  const first = checkSnapshot(path);
  advanceDocs(path, "B");
  exportSnapshot(path);
  writeFileSync(file, original);
  const reverted = recordDocuments(path);
  expect(reverted.inserted).toBe(1);
  exportSnapshot(path);
  const final = checkSnapshot(path);
  expect(final.verification.operations).toBe(first.verification.operations + 2);
  expect(recordDocuments(path).inserted).toBe(0);
});

test("restore fast-forwards an exact prefix and explicitly preserves unpublished local-ahead state", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source);
  const clone = root();
  copySnapshot(source, clone);
  restoreSnapshot(clone);
  advanceDocs(source, "next published edition");
  exportSnapshot(source);
  copySnapshot(source, clone);
  expect(restoreSnapshot(clone).imported).toBe(1);
  advanceDocs(clone, "unpublished local edition");
  const newer = restoreSnapshot(clone);
  expect(newer.imported).toBe(0);
  expect(newer.status).toBe("local-ahead");
  expect(newer.verification.operations).toBe(4);
});

test("divergent destination and existing public history remain unchanged on refusal", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source);
  const clone = root();
  copySnapshot(source, clone);
  restoreSnapshot(clone);
  advanceDocs(source, "left history");
  exportSnapshot(source);
  advanceDocs(clone, "right history");
  copySnapshot(source, clone);
  const oh = openLedger(clone);
  const before = oh.verify();
  oh.store.close();
  expect(() => restoreSnapshot(clone)).toThrow("Divergent");
  const bytes = readFileSync(join(clone, "ledger/operations.json"));
  expect(() => exportSnapshot(clone)).toThrow("Divergent");
  expect(readFileSync(join(clone, "ledger/operations.json"))).toEqual(bytes);
  const reopened = openLedger(clone);
  expect(reopened.verify()).toEqual(before);
  reopened.store.close();
});

test("tampered, wrong-space, wrong-contract and incomplete bundles fail before destination creation", () => {
  const source = root();
  bootstrap(source);
  exportSnapshot(source);
  const clone = root();
  copySnapshot(source, clone);
  const path = join(clone, "ledger/operations.json");
  const original = JSON.parse(readFileSync(path));
  for (const alter of [
    value => { value.operations[1].actorId = "tampered"; },
    value => { value.spaceId = "other-project"; },
    value => { value.contractSha256 = "0".repeat(64); },
    value => { value.operations.reverse(); },
  ]) {
    const changed = structuredClone(original);
    alter(changed);
    writeFileSync(path, JSON.stringify(changed));
    expect(() => restoreSnapshot(clone)).toThrow();
    expect(existsSync(join(clone, ".oh"))).toBe(false);
  }
  writeFileSync(path, JSON.stringify(createOhSyncBundleV1(SPACE, original.operations.slice(1))));
  expect(() => restoreSnapshot(clone)).toThrow("complete ordered history");
  writeFileSync(path, JSON.stringify(createOhSyncBundleV1(SPACE, original.operations.slice(0, 1))));
  expect(() => restoreSnapshot(clone)).toThrow("manifest");
  expect(existsSync(join(clone, ".oh"))).toBe(false);
});

test("partial manifests and linked snapshot paths fail closed without database or snapshot replacement", () => {
  const source = root();
  expect(() => restoreSnapshot(source)).toThrow("Missing");
  expect(existsSync(join(source, ".oh"))).toBe(false);
  bootstrap(source);
  exportSnapshot(source);
  const clone = root();
  copySnapshot(source, clone);
  const bytes = readFileSync(join(clone, "ledger/operations.json"));
  unlinkSync(join(clone, "ledger/manifest.json"));
  expect(() => restoreSnapshot(clone)).toThrow("Missing");
  expect(() => exportSnapshot(clone)).toThrow("Missing");
  expect(readFileSync(join(clone, "ledger/operations.json"))).toEqual(bytes);
  expect(existsSync(join(clone, ".oh"))).toBe(false);
  const linked = root();
  symlinkSync(join(source, "ledger"), join(linked, "ledger"));
  expect(() => checkSnapshot(linked)).toThrow("real directory");
});

test("a profile-labeled private note in history cannot enter the public snapshot", () => {
  const path = root();
  bootstrap(path);
  exportSnapshot(path);
  const bytes = readFileSync(join(path, "ledger/operations.json"));
  const manifest = readFileSync(join(path, "ledger/manifest.json"));
  const oh = openLedger(path);
  let after;
  try {
    commitAdditive(oh, [createKnowledgeGraphRecordV1({ v: 1, key: "context:private-test-note", kind: "context", dependencies: [],
      value: JSON.parse(canonicalJson({ profile: PROFILE, body: "Private synthetic test input, not for publication." })) })], "private-test");
    after = oh.verify();
  } finally { oh.store.close(); }
  expect(() => exportSnapshot(path)).toThrow("Unreviewed or unrelated");
  expect(readFileSync(join(path, "ledger/operations.json"))).toEqual(bytes);
  expect(readFileSync(join(path, "ledger/manifest.json"))).toEqual(manifest);
  const reopened = openLedger(path);
  expect(reopened.verify()).toEqual(after);
  reopened.store.close();
});

test("clean latest Markdown cannot conceal an unreviewed intermediate private edition", () => {
  const path = root();
  bootstrap(path);
  exportSnapshot(path);
  const bytes = readFileSync(join(path, "ledger/operations.json"));
  const manifest = readFileSync(join(path, "ledger/manifest.json"));
  const file = join(path, DOCUMENT_PATHS[0]);
  const original = readFileSync(file);
  advanceDocs(path, "Synthetic private intermediate content");
  writeFileSync(file, original);
  recordDocuments(path);
  const oh = openLedger(path);
  const before = oh.verify();
  oh.store.close();
  expect(() => exportSnapshot(path)).toThrow("Unreviewed intermediate document edition");
  expect(readFileSync(join(path, "ledger/operations.json"))).toEqual(bytes);
  expect(readFileSync(join(path, "ledger/manifest.json"))).toEqual(manifest);
  const reopened = openLedger(path);
  expect(reopened.verify()).toEqual(before);
  reopened.store.close();
});

// The two newest experiments enter the public snapshot only through their
// reviewed constructors; a forged activity under the same prefix is refused.
function experimentFixture(path) {
  mkdirSync(join(path, "artifacts"), { recursive: true });
  mkdirSync(join(path, "src"), { recursive: true });
  for (const file of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]) writeFileSync(join(path, file), "# synthetic source fixture\n");
  writeFileSync(join(path, "src/lib.rs"), "// Synthetic source fixture, not experimental provenance.\n");
  for (const [artifact, protocol] of [["indexed-transfer.json", "indexed-transfer-protocol.md"], ["implication-calibration.json", "implication-protocol.md"]]) {
    writeFileSync(join(path, "artifacts", artifact), readFileSync(new URL("../artifacts/" + artifact, import.meta.url)));
    writeFileSync(join(path, "experiments", protocol), readFileSync(new URL("../experiments/" + protocol, import.meta.url)));
  }
}

test("indexed-transfer and implication-calibration observations are admitted through the allowlist and forged activities are refused", () => {
  const path = root();
  experimentFixture(path);
  bootstrap(path);
  const indexed = recordIndexedTransfer(path, "artifacts/indexed-transfer.json");
  const implication = recordImplicationCalibration(path, "artifacts/implication-calibration.json");
  expect(indexed.inserted).toBe(5);
  expect(implication.inserted).toBe(5);
  const exported = exportSnapshot(path);
  expect(checkSnapshot(path).verification).toEqual(exported.verification);
  const clone = root();
  experimentFixture(clone);
  copySnapshot(path, clone);
  expect(restoreSnapshot(clone).verification).toEqual(exported.verification);
  const bytes = readFileSync(join(path, "ledger/operations.json"));
  const bundle = JSON.parse(bytes);
  const editions = bundle.operations.flatMap(operation => operation.changes).map(change => change.record)
    .filter(record => record.kind === "edition" && record.value.mediaType === "application/json").map(record => record.key);
  expect(editions).toEqual(expect.arrayContaining(["edition:indexed-transfer-" + indexed.reportSha256, "edition:implication-calibration-" + implication.reportSha256]));
  const oh = openLedger(path);
  let after;
  try {
    const genuine = oh.list({ kind: "activity", limit: 20 }).find(record => record.key.startsWith("activity:indexed-transfer-"));
    const forged = createKnowledgeGraphRecordV1({ v: 1, key: "activity:indexed-transfer-" + "f".repeat(64), kind: "activity", dependencies: genuine.dependencies,
      value: JSON.parse(canonicalJson({ ...genuine.value, authority: "Forged synthetic activity, not a reviewed ingestion." })) });
    commitAdditive(oh, [forged], "forged-test");
    after = oh.verify();
  } finally { oh.store.close(); }
  expect(() => exportSnapshot(path)).toThrow("Unreviewed or unrelated");
  expect(readFileSync(join(path, "ledger/operations.json"))).toEqual(bytes);
  const reopened = openLedger(path);
  expect(reopened.verify()).toEqual(after);
  reopened.store.close();
}, 120_000);
