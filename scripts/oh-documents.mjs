import { lstatSync, readFileSync, realpathSync } from "node:fs";
import { relative, resolve, sep } from "node:path";
import { canonicalJson, canonicalSha256, sha256Hex } from "@hraness/oh";
import { commitAdditive, openLedger, record } from "./oh-ledger.mjs";

export const DOCUMENT_PATHS = [
  "docs/research-proposal.md",
  "docs/knowledge-and-complexity.md",
  "docs/clue-transfer-theory.md",
  "experiments/unit-propagation.md",
  "experiments/clue-transfer.md",
  "experiments/clue-transfer-protocol.md",
].sort();
export const DOCUMENT_REGISTRY_PREFIX = "context:canonical-documents-";

export function readDocument(root, path) {
  if (!DOCUMENT_PATHS.includes(path)) throw new Error("Unsupported canonical document path.");
  const file = resolve(root, path);
  const info = lstatSync(file);
  const local = relative(realpathSync(root), realpathSync(file));
  if (!info.isFile() || info.isSymbolicLink() || info.size > 1024 * 1024 || local === ".." || local.startsWith(".." + sep)) {
    throw new Error("Canonical document must be a bounded regular repository file.");
  }
  const bytes = readFileSync(file);
  const body = bytes.toString("utf8");
  if (bytes.length > 1024 * 1024 || !Buffer.from(body).equals(bytes)) throw new Error("Canonical document must contain bounded UTF-8 text.");
  return { path, body, sha256: sha256Hex(bytes) };
}

export function documentRecords(documents, previousRegistry = null) {
  if (documents.length !== DOCUMENT_PATHS.length || documents.some((doc, index) => doc.path !== DOCUMENT_PATHS[index] ||
    typeof doc.body !== "string" || Buffer.byteLength(doc.body) > 1024 * 1024 || sha256Hex(doc.body) !== doc.sha256)) {
    throw new Error("Canonical documents require the exact path set and matching body hashes.");
  }
  const entries = documents.map(doc => ({ path: doc.path, sha256: doc.sha256,
    editionKey: "edition:document-" + canonicalSha256({ path: doc.path, sha256: doc.sha256 }) }));
  return [
    ...documents.map((doc, index) => record(entries[index].editionKey, "edition", {
      path: doc.path, body: doc.body, sha256: doc.sha256, mediaType: "text/markdown", encoding: "utf-8",
    })),
    record(DOCUMENT_REGISTRY_PREFIX + canonicalSha256({ documents: entries, previousRegistry }), "context", {
      schema: "peqnp.canonical-documents.v1",
      authority: "Canonical project research narratives; working Markdown must match these editions.",
      documents: entries,
      previousRegistry,
    }, [...entries.map(entry => entry.editionKey), ...(previousRegistry ? [previousRegistry] : [])]),
  ];
}

export function recordDocuments(root) {
  const documents = DOCUMENT_PATHS.map(path => readDocument(root, path));
  const oh = openLedger(root);
  try {
    const head = oh.verify();
    if (head.operations > 1000) throw new Error("Document activation exceeds the bounded operation history; inspect before extending.");
    const registries = oh.store.exportOperations(0, 1000).flatMap(operation => operation.changes)
      .filter(change => change.kind === "put" && change.record.key.startsWith(DOCUMENT_REGISTRY_PREFIX));
    const previous = registries.at(-1)?.record;
    const records = documentRecords(documents, previous?.key ?? null);
    if (previous && canonicalJson(previous.value.documents) === canonicalJson(records.at(-1).value.documents)) {
      return { inserted: 0, verification: oh.verify() };
    }
    return commitAdditive(oh, records, "canonical-documents-v1", head.head);
  }
  finally { oh.store.close(); }
}
