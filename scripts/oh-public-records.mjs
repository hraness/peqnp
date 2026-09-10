import { canonicalJson, canonicalSha256 } from "@hraness/oh";
import { calibrationRecords, clueTransferRecords, readReport, seedRecords, validateCalibration } from "./oh-ledger.mjs";
import { validateClueTransfer } from "./oh-transfer-report.mjs";
import { documentRecords, DOCUMENT_PATHS, DOCUMENT_REGISTRY_PREFIX, readDocument } from "./oh-documents.mjs";
import { researchRecords } from "./research-records.mjs";

function sourceIdentity(source) {
  const meaning = "Source bytes observed at ingestion; not an attestation that these bytes produced the supplied result.";
  if (!source || source.meaning !== meaning || !Array.isArray(source.files) || !source.files.length ||
      canonicalJson(Object.keys(source).sort()) !== canonicalJson(["files", "meaning", "sha256"]) ||
      source.sha256 !== canonicalSha256(source.files)) throw new Error("Invalid observed source identity.");
  const paths = source.files.map(file => file.path);
  if (new Set(paths).size !== paths.length || canonicalJson([...paths].sort()) !== canonicalJson(paths) ||
      source.files.some(file => !file || canonicalJson(Object.keys(file).sort()) !== canonicalJson(["path", "sha256"]) || typeof file.path !== "string" ||
        !/^(?:Cargo\.(?:toml|lock)|rust-toolchain\.toml|(?:src|tests)\/[A-Za-z0-9_/-]+\.rs)$/.test(file.path) ||
        file.path.split("/").some(part => ["", ".", ".."].includes(part)) || !/^[a-f0-9]{64}$/.test(file.sha256)) ||
      ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"].some(path => !paths.includes(path))) throw new Error("Unsupported source manifest path or digest.");
}

// Admission reconstructs every historical put from a reviewed project
// constructor. A profile label alone cannot make an arbitrary note public.
export function admitPublicHistory(root, operations, { requireCurrentDocuments = true } = {}) {
  const actual = new Map();
  const currentReports = new Map();
  let latestRegistry;
  for (const operation of operations) for (const change of operation.changes) {
    if (change.kind !== "put") throw new Error("Public project history cannot contain tombstones or other mutations.");
    const previous = actual.get(change.record.key);
    if (previous && canonicalJson(previous) !== canonicalJson(change.record)) throw new Error("Public history must preserve immutable record identities.");
    actual.set(change.record.key, change.record);
    if (change.record.key.startsWith(DOCUMENT_REGISTRY_PREFIX)) {
      if (change.record.value.previousRegistry !== (latestRegistry?.key ?? null)) throw new Error("Document activation must extend the previous canonical registry.");
      latestRegistry = change.record;
    }
    if (change.record.kind === "edition" && change.record.value.mediaType === "application/json") {
      currentReports.set(change.record.value.path, change.record);
    }
  }
  const expected = new Map();
  const add = records => { for (const record of records) expected.set(record.key, record); };
  add(seedRecords());
  add(researchRecords());
  for (const activity of actual.values()) {
    if (activity.kind !== "activity") continue;
    const transfer = activity.key.startsWith("activity:clue-transfer-");
    const calibration = activity.key.startsWith("activity:calibration-");
    if (!transfer && !calibration) continue;
    const edition = activity.dependencies.map(key => actual.get(key)).find(record => record?.kind === "edition");
    const path = transfer ? "artifacts/clue-transfer.json" : "artifacts/calibration.json";
    if (!edition || edition.value.path !== path) throw new Error("Unrecognized experiment edition path.");
    const input = { path, sha256: edition.value.sha256, report: edition.value.report };
    if (!/^[a-f0-9]{64}$/.test(input.sha256)) throw new Error("Invalid report byte identity.");
    (transfer ? validateClueTransfer : validateCalibration)(input.report);
    sourceIdentity(activity.value.source);
    add(transfer ? clueTransferRecords(input, activity.value.source) : calibrationRecords(input, activity.value.source));
  }
  for (const registry of actual.values()) {
    if (!registry.key.startsWith(DOCUMENT_REGISTRY_PREFIX)) continue;
    if (!Array.isArray(registry.value.documents)) throw new Error("Malformed canonical-document registry.");
    const documents = registry.value.documents.map(entry => {
      const edition = actual.get(entry.editionKey);
      if (!edition || edition.value.path !== entry.path || edition.value.sha256 !== entry.sha256) throw new Error("Document registry dependency mismatch.");
      return { path: entry.path, sha256: entry.sha256, body: edition.value.body };
    });
    add(documentRecords(documents, registry.value.previousRegistry));
  }
  for (const [key, record] of actual) {
    if (!expected.has(key) || canonicalJson(expected.get(key)) !== canonicalJson(record)) {
      throw new Error("Unreviewed or unrelated historical record cannot be published: " + key);
    }
  }
  for (const seed of seedRecords()) if (actual.get(seed.key)?.recordSha256 !== seed.recordSha256) throw new Error("Required project bootstrap record is missing.");
  if (requireCurrentDocuments) {
    if (!latestRegistry) throw new Error("Canonical document registry missing; run oh:record:docs after narrative review.");
    const current = documentRecords(DOCUMENT_PATHS.map(path => readDocument(root, path)), latestRegistry.value.previousRegistry).at(-1);
    if (canonicalJson(latestRegistry) !== canonicalJson(current)) throw new Error("Markdown differs from the latest canonical document editions; review and record the change explicitly.");
    for (const [path, edition] of currentReports) {
      const input = readReport(root, path, path === "artifacts/clue-transfer.json" ? validateClueTransfer : validateCalibration);
      if (input.sha256 !== edition.value.sha256 || canonicalJson(input.report) !== canonicalJson(edition.value.report)) {
        throw new Error("Published report bytes disagree with the latest stored experiment edition.");
      }
    }
  }
  return { records: actual.size, canonicalDocuments: latestRegistry?.value.documents.length ?? 0 };
}

export function admitUnpublishedDocuments(root, operations, previouslyExported = []) {
  const previouslyExportedRecords = new Set(previouslyExported.flatMap(operation => operation.changes)
    .filter(change => change.kind === "put").map(change => change.record.recordSha256));
  const current = new Map(DOCUMENT_PATHS.map(path => [path, readDocument(root, path)]));
  for (const operation of operations) for (const change of operation.changes) {
    const record = change.record;
    if (change.kind !== "put" || record.kind !== "edition" || !DOCUMENT_PATHS.includes(record.value.path) || previouslyExportedRecords.has(record.recordSha256)) continue;
    const document = current.get(record.value.path);
    if (record.value.sha256 !== document.sha256 || record.value.body !== document.body) {
      throw new Error("Unreviewed intermediate document edition in unpublished history; preserve it and explicitly review publication scope.");
    }
  }
}
