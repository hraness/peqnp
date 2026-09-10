import { lstatSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { canonicalJson, canonicalSha256, sha256Hex } from "@hraness/oh";
import { commitAdditive, openLedger, readReport, record, seedRecords, sourceManifest } from "./oh-ledger.mjs";
import { validateIndexedTransfer } from "./oh-indexed-report.mjs";
import { validateImplicationCalibration } from "./oh-implication-report.mjs";
import { validateFragmentInterface } from "./oh-fragment-report.mjs";
import { validateExtractionCost } from "./oh-extraction-report.mjs";

const MAX_PROTOCOL_BYTES = 1024 * 1024;
const AUTHORITY = "Local report ingestion; input claims require independent experimental review.";
// Pinned Oh v0.4.3 caps one graph record value at OH_GRAPH_LIMITS_V1.recordBytes
// (1 MiB canonical). A report whose canonical bytes fit is stored whole in its
// edition, byte for byte as before. A larger report (extraction-cost-v1 is
// 1.78 MB canonical) is stored as the same edition without `observations`
// plus ordered observation-part editions, each under the bound; the edition
// depends on its parts and `editionReport` reassembles the exact report so
// publication admission still revalidates it from the ledger alone.
const EDITION_REPORT_BYTES = 768 * 1024;

function observationParts(prefix, input) {
  const bytes = value => Buffer.byteLength(canonicalJson(value), "utf8");
  if (bytes(input.report) <= EDITION_REPORT_BYTES) return null;
  const parts = [[]];
  let size = 0;
  for (const row of input.report.observations) {
    const rowBytes = bytes(row) + 1;
    if (size + rowBytes > EDITION_REPORT_BYTES && parts.at(-1).length > 0) { parts.push([]); size = 0; }
    parts.at(-1).push(row);
    size += rowBytes;
  }
  return parts.map((observations, part) => record("edition:" + prefix + "-" + input.sha256 + "-part-" + part, "edition", {
    path: input.path, reportSha256: input.sha256, mediaType: "application/json", part, parts: parts.length, observations,
  }));
}

// The report stored by an edition, reassembled from its observation parts
// when the edition was split; `lookup` resolves a record key.
export function editionReport(edition, lookup) {
  const value = edition.value;
  if (!value.observationParts) return value.report;
  const parts = value.observationParts.map((key, index) => {
    const part = lookup(key);
    if (!part || part.kind !== "edition" || part.value.path !== value.path || part.value.reportSha256 !== value.sha256 ||
        part.value.part !== index || part.value.parts !== value.observationParts.length || !Array.isArray(part.value.observations)) {
      throw new Error("Observation part missing or inconsistent with its edition.");
    }
    return part.value.observations;
  });
  const observations = parts.flat();
  if (observations.length !== value.observationCount || parts.some(part => part.length === 0)) throw new Error("Observation parts do not reassemble the recorded report.");
  return { ...value.report, observations };
}

// Record constructors follow clueTransferRecords exactly: one shared statement
// per experiment, then edition, activity, assertion, and evidence per
// observation/source identity. Keys are stable identities for later research.
function observationRecords({ prefix, statement, proposition, domain, kind, evidence }, input, source) {
  const identity = canonicalSha256({ reportSha256: input.sha256, sourceSha256: source.sha256 });
  const edition = "edition:" + prefix + "-" + input.sha256;
  const activity = "activity:" + prefix + "-" + identity;
  const assertion = "assertion:" + prefix + "-" + identity;
  const parts = observationParts(prefix, input);
  const { observations, ...rest } = input.report;
  const stored = parts
    ? { report: rest, observationParts: parts.map(part => part.key), observationCount: observations.length }
    : { report: input.report };
  return [
    record(statement, "statement", { proposition, domain }, ["context:research-method"]),
    ...(parts ?? []),
    record(edition, "edition", { path: input.path, sha256: input.sha256, mediaType: "application/json", ...stored }, parts ? parts.map(part => part.key) : []),
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
      // The fragment interface uses fixed algorithmic knowledge and carries no frozen library.
      ...(input.report.frozen_library ? { frozenLibrarySha256: canonicalSha256(input.report.frozen_library.rules) } : {}),
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

export function fragmentInterfaceRecords(input, source) {
  return observationRecords({
    prefix: "fragment-interface",
    statement: "statement:fragment-interface-v1-comparison",
    proposition: "The fragment-interface-v1 experiment compares, on 162 fixed width-at-most-three CNF cases, the deterministic DPLL baseline on the original formula against the binary-fragment interface: implication-graph construction, components, a fragment certificate, forced-literal extraction, and unit appends followed by the same DPLL.",
    domain: "The fixed finite protocol and exact reports cited by each observation; a known 2-SAT interface is calibrated as a preprocessor, and no total-work improvement over the baseline, novel inference rule, or general SAT complexity result is asserted by this statement.",
    kind: "finite-fragment-interface-report",
    evidence: report => ({
      measuredSummary: report.summary,
      comparison: report.comparison,
      phaseWorkUnits: report.summary.arms.fragment.phases,
      outsideArmWorkUnits: report.outside_arms,
      enumerationRatios: report.summary.enumeration_ratio,
    }),
  }, input, source);
}

export function extractionCostRecords(input, source) {
  return observationRecords({
    prefix: "extraction-cost",
    statement: "statement:extraction-cost-v1-comparison",
    proposition: "The extraction-cost-v1 experiment compares, on 176 fresh width-at-most-three CNF cases and the 162 fragment-interface-v1 cases, three arms under one budget: the deterministic DPLL baseline on the original formula, the unchanged per-literal fragment-interface arm, and a settled extraction that differs from it in the extraction phase alone.",
    domain: "The fixed finite protocol and exact reports cited by each observation; both extraction procedures share the known O(n(n + m2)) worst case, and no total-work improvement over the baseline, linear extraction bound, novel inference rule, or general SAT complexity result is asserted by this statement.",
    kind: "finite-extraction-cost-report",
    evidence: report => ({
      measuredSummary: { primary: report.primary.summary, secondary: report.secondary.summary },
      comparison: report.comparison,
      comparisons: { primary: report.primary.summary.comparisons, secondary: report.secondary.summary.comparisons },
      extractionWorkUnits: Object.fromEntries(["primary", "secondary"].map(section => [section, {
        per_literal: report[section].summary.arms.per_literal.phases.extraction_work_units,
        settled: report[section].summary.arms.settled.phases.extraction_work_units,
      }])),
      settledStats: { primary: report.primary.summary.settled_stats, secondary: report.secondary.summary.settled_stats },
      outsideArmWorkUnits: report.outside_arms,
      enumerationRatios: { primary: report.primary.summary.enumeration_ratio, secondary: report.secondary.summary.enumeration_ratio },
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
    return { reportSha256: input.sha256, ...commitInParts(oh, records(input, source), purpose) };
  } finally { oh.store.close(); }
}

// Edition size policy: each observation part is committed as its own
// operation (purpose `<experiment>-part-<i>`) before the remaining records,
// so that no single ledger operation exceeds one part (786,432 canonical
// bytes) and a report up to the 4 MiB bound pages part by part. Parts are
// dependency-free and additive, so an interrupted ingestion resumes: parts
// already present are skipped and the final operation carries the rest.
function commitInParts(oh, records, purpose) {
  const isPart = record => record.kind === "edition" && Number.isInteger(record.value.part);
  const parts = records.filter(isPart);
  const rest = records.filter(record => !isPart(record));
  let inserted = 0;
  for (const [index, part] of parts.entries()) {
    inserted += commitAdditive(oh, [part], `${purpose}-part-${index}`).inserted;
  }
  const last = commitAdditive(oh, rest, purpose);
  return { inserted: inserted + last.inserted, verification: last.verification };
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

export function recordFragmentInterface(root, relativePath) {
  return recordExperiment(root, relativePath, {
    validate: validateFragmentInterface,
    protocolPath: "experiments/fragment-interface-protocol.md",
    records: fragmentInterfaceRecords,
    purpose: "fragment-interface-v1",
  });
}

export function recordExtractionCost(root, relativePath) {
  return recordExperiment(root, relativePath, {
    validate: validateExtractionCost,
    protocolPath: "experiments/extraction-cost-protocol.md",
    records: extractionCostRecords,
    purpose: "extraction-cost-v1",
  });
}
