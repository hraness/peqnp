import { afterEach, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Oh } from "@hraness/oh/sdk";
import { admitPublicHistory } from "./oh-public-records.mjs";
import { commitAdditive, initializeLedger, openLedger } from "./oh-ledger.mjs";
import {
  documentRecords, documentRecordsForSchema, documentRecordsV2, documentRecordsV3, documentRecordsV4, DOCUMENT_PATHS, DOCUMENT_PATHS_V1, DOCUMENT_PATHS_V2,
  DOCUMENT_PATHS_V3, readDocument, recordDocuments,
} from "./oh-documents.mjs";

const roots = [];
const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
function root() {
  const path = mkdtempSync(join(tmpdir(), "peqnp-document-versions-"));
  roots.push(path);
  for (const file of DOCUMENT_PATHS) {
    mkdirSync(dirname(join(path, file)), { recursive: true });
    writeFileSync(join(path, file), "Synthetic document-version fixture: " + file + "\n");
  }
  return path;
}
afterEach(() => { for (const path of roots.splice(0)) rmSync(path, { recursive: true, force: true }); });

test("the original five operations and 35 records remain admissible and replay to their exact head", () => {
  const operations = JSON.parse(readFileSync(new URL("../ledger/pages/0001.json", import.meta.url))).operations.slice(0, 5);
  expect(operations.length).toBe(5);
  expect(operations.at(-1).operationSha256).toBe("a826190be2b4cbb50920deb43c91676dcc7095219355596104c3fd916b30a9f2");
  expect(admitPublicHistory(repositoryRoot, operations, { requireCurrentDocuments: false }).records).toBe(35);
  const path = root();
  const oh = Oh.open({ databasePath: join(path, "original-history.sqlite"), spaceId: "peqnp" });
  try {
    oh.store.importOperations({ expectedHead: { sequence: 0, operationSha256: null }, operations });
    const verified = oh.verify();
    expect(verified.records).toBe(35);
    expect(verified.head.recordsSha256).toBe("fa3be6a8a326d960d208d30bcf350da14097fdda59659a701c876e89756b094f");
    expect(verified.head.graphRevisionSha256).toBe("df6ee8e17d4a9abbde832f59a4e53a66f9df4caf17ebea249782a930e54f55f3");
  } finally { oh.store.close(); }
});

test("v1 to v4 activation preserves old editions and adds only the eight new documents plus registry", () => {
  const path = root();
  initializeLedger(path);
  const v1 = documentRecords(DOCUMENT_PATHS_V1.map(file => readDocument(path, file)));
  const oh = openLedger(path);
  try { commitAdditive(oh, v1, "document-version-fixture"); }
  finally { oh.store.close(); }
  const upgraded = recordDocuments(path);
  expect(upgraded.inserted).toBe(9);
  const current = openLedger(path);
  try {
    for (const record of v1) expect(current.get(record.key)?.recordSha256).toBe(record.recordSha256);
    const last = current.store.exportOperations(0, 1000).at(-1).changes.find(change => change.record.kind === "context").record;
    expect(last.value.schema).toBe("peqnp.canonical-documents.v4");
    expect(last.value.previousRegistry).toBe(v1.at(-1).key);
    expect(last.value.documents.length).toBe(14);
    const history = current.store.exportOperations(0, 1000);
    expect(admitPublicHistory(path, history).canonicalDocuments).toBe(14);
  } finally { current.store.close(); }
  expect(recordDocuments(path).inserted).toBe(0);
  expect(() => documentRecords(DOCUMENT_PATHS.map(file => readDocument(path, file)))).toThrow("exact path set");
  expect(() => documentRecordsV2(DOCUMENT_PATHS.map(file => readDocument(path, file)))).toThrow("exact ten-file set");
  expect(() => documentRecordsV3(DOCUMENT_PATHS.map(file => readDocument(path, file)))).toThrow("exact twelve-file set");
  expect(() => documentRecordsV4(DOCUMENT_PATHS_V3.map(file => readDocument(path, file)))).toThrow("exact fourteen-file set");
  expect(() => documentRecordsForSchema("unknown", [], null)).toThrow("Unsupported");
});

test("v2 to v4 activation keeps the ten v2 editions and adds the four fragment and extraction documents plus registry", () => {
  const path = root();
  initializeLedger(path);
  const v2 = documentRecordsV2(DOCUMENT_PATHS_V2.map(file => readDocument(path, file)));
  const oh = openLedger(path);
  try { commitAdditive(oh, v2, "document-version-fixture"); }
  finally { oh.store.close(); }
  const upgraded = recordDocuments(path);
  expect(upgraded.inserted).toBe(5);
  const current = openLedger(path);
  try {
    for (const record of v2) expect(current.get(record.key)?.recordSha256).toBe(record.recordSha256);
    const last = current.store.exportOperations(0, 1000).at(-1).changes.find(change => change.record.kind === "context").record;
    expect(last.value.schema).toBe("peqnp.canonical-documents.v4");
    expect(last.value.previousRegistry).toBe(v2.at(-1).key);
    expect(last.value.documents.length).toBe(14);
    expect(last.value.documents.map(entry => entry.path)).toEqual(DOCUMENT_PATHS);
    expect(admitPublicHistory(path, current.store.exportOperations(0, 1000)).canonicalDocuments).toBe(14);
  } finally { current.store.close(); }
  expect(recordDocuments(path).inserted).toBe(0);
});

test("v3 to v4 activation keeps the twelve v3 editions and adds the two extraction documents plus registry", () => {
  const path = root();
  initializeLedger(path);
  const v3 = documentRecordsV3(DOCUMENT_PATHS_V3.map(file => readDocument(path, file)));
  const oh = openLedger(path);
  try { commitAdditive(oh, v3, "document-version-fixture"); }
  finally { oh.store.close(); }
  const upgraded = recordDocuments(path);
  expect(upgraded.inserted).toBe(3);
  const current = openLedger(path);
  try {
    for (const record of v3) expect(current.get(record.key)?.recordSha256).toBe(record.recordSha256);
    const last = current.store.exportOperations(0, 1000).at(-1).changes.find(change => change.record.kind === "context").record;
    expect(last.value.schema).toBe("peqnp.canonical-documents.v4");
    expect(last.value.previousRegistry).toBe(v3.at(-1).key);
    expect(last.value.documents.length).toBe(14);
    expect(last.value.documents.map(entry => entry.path)).toEqual(DOCUMENT_PATHS);
    expect(admitPublicHistory(path, current.store.exportOperations(0, 1000)).canonicalDocuments).toBe(14);
  } finally { current.store.close(); }
  expect(recordDocuments(path).inserted).toBe(0);
});
