import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { canonicalJson } from "@hraness/oh";
import { commitAdditive, openLedger } from "./oh-ledger.mjs";
import { recordDocuments } from "./oh-documents.mjs";
import { admitPublicHistory } from "./oh-public-records.mjs";
import { researchRecords, RESEARCH_REPORT_SHA256, RESEARCH_TRANSFER_EDITION_KEY, RESEARCH_TRANSFER_EVIDENCE_KEY } from "./research-records.mjs";
import {
  researchRecordsV2, IMPLICATION_EDITION_KEY, IMPLICATION_EVIDENCE_KEY, IMPLICATION_REPORT_SHA256,
  INDEXED_EDITION_KEY, INDEXED_EVIDENCE_KEY, INDEXED_REPORT_SHA256,
} from "./research-records-v2.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export function recordResearch(root) {
  const oh = openLedger(root);
  try {
    const verified = oh.verify();
    if (verified.operations > 1000) throw new Error("Research admission exceeds the bounded history; inspect before extending.");
    admitPublicHistory(root, oh.store.exportOperations(0, 1000), { requireCurrentDocuments: false });
    if (oh.get(RESEARCH_TRANSFER_EDITION_KEY)?.value.sha256 !== RESEARCH_REPORT_SHA256 ||
        oh.get(RESEARCH_TRANSFER_EVIDENCE_KEY)?.value.reportSha256 !== RESEARCH_REPORT_SHA256) {
      throw new Error("The exact reviewed transfer edition and evidence must already exist.");
    }
    return commitAdditive(oh, researchRecords(), "reviewed-research-v1", verified.head);
  } finally { oh.store.close(); }
}

export function recordResearchV2(root) {
  const oh = openLedger(root);
  try {
    const verified = oh.verify();
    if (verified.operations > 1000) throw new Error("Research admission exceeds the bounded history; inspect before extending.");
    admitPublicHistory(root, oh.store.exportOperations(0, 1000), { requireCurrentDocuments: false });
    for (const [edition, evidence, sha256] of [
      [INDEXED_EDITION_KEY, INDEXED_EVIDENCE_KEY, INDEXED_REPORT_SHA256],
      [IMPLICATION_EDITION_KEY, IMPLICATION_EVIDENCE_KEY, IMPLICATION_REPORT_SHA256],
    ]) {
      if (oh.get(edition)?.value.sha256 !== sha256 || oh.get(evidence)?.value.reportSha256 !== sha256) {
        throw new Error("The exact reviewed indexed-transfer and implication-calibration editions and evidence must already exist.");
      }
    }
    for (const key of ["assertion:clue-transfer-v1-total-cost-lesson", "assertion:long-binary-backbone-explanations-v1", "inquiry:clue-transfer-cost-and-coverage-v2"]) {
      if (!oh.get(key)) throw new Error("The reviewed v1 research records must already exist.");
    }
    return commitAdditive(oh, researchRecordsV2(), "reviewed-research-v2", verified.head);
  } finally { oh.store.close(); }
}

if (import.meta.main) {
  try {
    const args = process.argv.slice(2);
    if (args.length !== 1 || !["research", "research-v2", "docs"].includes(args[0])) throw new Error("Usage: bun scripts/oh-research.mjs <research|research-v2|docs>");
    const result = args[0] === "research" ? recordResearch(repositoryRoot)
      : args[0] === "research-v2" ? recordResearchV2(repositoryRoot) : recordDocuments(repositoryRoot);
    process.stdout.write(canonicalJson(result) + "\n");
  } catch (error) { process.stderr.write(String(error.message ?? error) + "\n"); process.exitCode = 1; }
}
