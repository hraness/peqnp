import { lstatSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { canonicalSha256, sha256Hex } from "@hraness/oh";
import { commitAdditive, openLedger, readReport, record, seedRecords, sourceManifest } from "./oh-ledger.mjs";
import { validateIndexedTransfer } from "./oh-indexed-report.mjs";
import { validateImplicationCalibration } from "./oh-implication-report.mjs";

const MAX_PROTOCOL_BYTES = 1024 * 1024;
const AUTHORITY = "Local report ingestion; input claims require independent experimental review.";

// Record constructors follow clueTransferRecords exactly: one shared statement
// per experiment, then edition, activity, assertion, and evidence per
// observation/source identity. Keys are stable identities for later research.
function observationRecords({ prefix, statement, proposition, domain, kind, evidence }, input, source) {
  const identity = canonicalSha256({ reportSha256: input.sha256, sourceSha256: source.sha256 });
  const edition = "edition:" + prefix + "-" + input.sha256;
  const activity = "activity:" + prefix + "-" + identity;
  const assertion = "assertion:" + prefix + "-" + identity;
  return [
    record(statement, "statement", { proposition, domain }, ["context:research-method"]),
    record(edition, "edition", { path: input.path, sha256: input.sha256, mediaType: "application/json", report: input.report }),
    record(activity, "activity", {
      experiment: input.report.experiment,
      status: "recorded-observation",
      source,
      authority: AUTHORITY,
    }, [edition, "inquiry:p-equals-np"]),
    record(assertion, "assertion", {
      statement,
      stance: "bounded-observation",
      scope: { experiment: input.report.experiment, reportSha256: input.sha256 },
      formalProofAccepted: false,
    }, [activity, statement]),
    record("evidence:" + prefix + "-" + identity, "evidence", {
      assertion,
      kind,
      reportSha256: input.sha256,
      frozenLibrarySha256: canonicalSha256(input.report.frozen_library.rules),
      ...evidence(input.report),
      limitations: input.report.limitations,
      interpretation: "Measured costs and outcomes remain observations, including regressions and unknowns; this is not a claim of a general speedup.",
      formalProofAccepted: false,
    }, [assertion, edition]),
  ];
}

export function indexedTransferRecords(input, source) {
  return observationRecords({
    prefix: "indexed-transfer",
    statement: "statement:indexed-transfer-v1-comparison",
    proposition: "The indexed-transfer-v1 experiment compares the same deterministic DPLL solver with no preprocessing, the v1 generic root matcher, and a metered sorted-array index applying the same frozen four-rule library on 160 fresh cases.",
    domain: "The fixed finite protocol and exact reports cited by each observation; generic and indexed matching are required to derive equal units, and no total-work improvement over the baseline is asserted by this statement.",
    kind: "finite-indexed-transfer-comparison-report",
    evidence: report => ({
      measuredSummary: report.summary,
      acquisitionWorkUnits: report.setup.acquisition.work_units,
      compilationWorkUnits: report.setup.compilation.work_units,
      setupPlusOnlineWorkUnits: report.setup_plus_online,
    }),
  }, input, source);
}

export function implicationCalibrationRecords(input, source) {
  return observationRecords({
    prefix: "implication-calibration",
    statement: "statement:implication-calibration-v1-comparison",
    proposition: "The implication-calibration-v1 experiment compares, on 120 fixed binary CNF cases, the frozen-library root preprocessor against complete implication-path backbone extraction for clue coverage, and the deterministic DPLL baseline against a certified SCC 2-SAT decision for cost.",
    domain: "The fixed finite protocol and exact reports cited by each observation; known 2-SAT reasoning is calibrated, and no speedup, novel inference rule, or general SAT complexity result is asserted by this statement.",
    kind: "finite-implication-calibration-report",
    evidence: report => ({
      measuredSummary: report.summary,
      acquisitionWorkUnits: report.setup.acquisition.work_units,
      outsideArmWorkUnits: report.outside_arms,
      coverage: report.summary.coverage,
      decisionComparison: report.decision_comparison,
    }),
  }, input, source);
}

function recordExperiment(root, relativePath, { validate, protocolPath, records, purpose }) {
  const input = readReport(root, relativePath, validate);
  const protocolFile = resolve(root, protocolPath);
  const info = lstatSync(protocolFile);
  if (!info.isFile() || info.isSymbolicLink() || info.size > MAX_PROTOCOL_BYTES ||
      sha256Hex(readFileSync(protocolFile)) !== input.report.protocol_sha256) {
    throw new Error("The local protocol does not match the report's fixed protocol identity.");
  }
  const source = sourceManifest(root);
  const oh = openLedger(root);
  try {
    for (const seed of seedRecords()) {
      if (oh.get(seed.key)?.recordSha256 !== seed.recordSha256) throw new Error("Bootstrap records are missing or changed; inspect before recording.");
    }
    return { reportSha256: input.sha256, ...commitAdditive(oh, records(input, source), purpose) };
  } finally { oh.store.close(); }
}

export function recordIndexedTransfer(root, relativePath) {
  return recordExperiment(root, relativePath, {
    validate: validateIndexedTransfer,
    protocolPath: "experiments/indexed-transfer-protocol.md",
    records: indexedTransferRecords,
    purpose: "indexed-transfer-v1",
  });
}

export function recordImplicationCalibration(root, relativePath) {
  return recordExperiment(root, relativePath, {
    validate: validateImplicationCalibration,
    protocolPath: "experiments/implication-protocol.md",
    records: implicationCalibrationRecords,
    purpose: "implication-calibration-v1",
  });
}
