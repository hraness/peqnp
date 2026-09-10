import { lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import {
  OH_CONTRACT_MANIFEST_V1,
  canonicalJson,
  canonicalSha256,
  createKnowledgeGraphRecordV1,
  sha256Hex,
} from "@hraness/oh";
import { Oh } from "@hraness/oh/sdk";
import { validateClueTransfer } from "./oh-transfer-report.mjs";

export const CONTRACT_SHA256 = "e53ae573c2af417082be9f554d0f6f3e317f054daf745181f462608e3f622594";
export const SPACE = "peqnp";
export const PROFILE = "peqnp.research-ledger.v1";
const MAX_REPORT_BYTES = 1024 * 1024;
const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const CALIBRATION_PROGRAMS = ["unit", "(neg unit)"].flatMap(polarity =>
  [false, true].flatMap(removeSatisfied => [false, true].map(removeFalse =>
    `(with-unit input (rewrite input ${polarity} ${removeSatisfied} ${removeFalse}))`)));
const SELECTED_PROGRAM = "(with-unit input (rewrite input unit true true))";
const FALSE_COMPLETENESS_INPUT = [[1, 2], [1, -2], [-1, 2], [-1, -2]];

export function inspectContract() {
  const manifest = OH_CONTRACT_MANIFEST_V1;
  if (manifest.contractId !== "oh.ontology.v1" || manifest.contractSha256 !== CONTRACT_SHA256) {
    throw new Error("Oh contract mismatch; preserve the database and qualify the pinned runtime.");
  }
  return manifest;
}

function existing(path) {
  try { return lstatSync(path); } catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}

export function openLedger(root, { initialize = false } = {}) {
  inspectContract();
  const directory = resolve(root, ".oh");
  const databasePath = resolve(directory, "research.sqlite");
  const folder = existing(directory);
  if (folder && (!folder.isDirectory() || folder.isSymbolicLink())) {
    throw new Error(".oh must be a real directory.");
  }
  const database = existing(databasePath);
  if (database && (!database.isFile() || database.isSymbolicLink() || database.nlink !== 1)) {
    throw new Error("The selected Oh database must be one regular, unlinked file.");
  }
  if (!database && !initialize) throw new Error("No ledger exists; run bun run oh:init explicitly.");
  for (const suffix of ["-wal", "-shm"]) {
    const sidecar = existing(databasePath + suffix);
    if (sidecar && (!sidecar.isFile() || sidecar.isSymbolicLink() || sidecar.nlink !== 1)) {
      throw new Error("Unexpected SQLite sidecar; preserve state and inspect the selected path.");
    }
  }
  const previousMask = process.umask(0o077);
  try {
    if (!folder) mkdirSync(directory, { mode: 0o700 });
    const oh = Oh.open({ databasePath, spaceId: SPACE });
    try { oh.verify(); } catch (error) { oh.store.close(); throw error; }
    return oh;
  } finally { process.umask(previousMask); }
}

export function record(key, kind, value, dependencies = []) {
  return createKnowledgeGraphRecordV1({
    v: 1, key, kind, dependencies: [...dependencies].sort(),
    // Keep SDK indexing and replay on the same detached canonical property order.
    value: JSON.parse(canonicalJson({ profile: PROFILE, ...value })),
  });
}

export function seedRecords() {
  return [
    record("context:research-method", "context", {
      evidencePolicy: "Observations, conjectures, and proof receipts are different records.",
      target: "One deterministic uniform SAT algorithm, correct on every valid input, with a fixed polynomial worst-case bit-cost bound.",
      formalProofAccepted: false,
    }),
    record("inquiry:p-equals-np", "inquiry", {
      question: "Can a deterministic uniform polynomial-time algorithm decide SAT on every valid input?",
      status: "open",
    }, ["context:research-method"]),
    record("statement:polynomial-sat", "statement", {
      proposition: "There exists a deterministic uniform polynomial-time algorithm deciding SAT.",
    }, ["context:research-method"]),
    record("assertion:p-equals-np-hypothesis", "assertion", {
      statement: "statement:polynomial-sat",
      stance: "research-hypothesis",
      justification: "A direction for exploration, never an assumption in a proof of this proposition.",
    }, ["inquiry:p-equals-np", "statement:polynomial-sat"]),
    record("statement:unit-propagation-preserves-sat", "statement", {
      proposition: "Assigning a unit literal and simplifying the CNF preserves satisfiability.",
      domain: "Finite CNF formulas with the assignment-restriction and extension interpretation.",
    }, ["context:research-method"]),
    record("assertion:unit-propagation-calibration-target", "assertion", {
      statement: "statement:unit-propagation-preserves-sat",
      stance: "calibration-target",
      formalProofAccepted: false,
    }, ["statement:unit-propagation-preserves-sat"]),
  ];
}

export function commitAdditive(oh, records, purpose, expectedHead) {
  const verified = oh.verify();
  if (expectedHead && canonicalJson(expectedHead) !== canonicalJson(verified.head)) {
    throw new Error("Ledger changed after the reviewed snapshot; inspect before retrying.");
  }
  const changes = [];
  for (const proposed of records) {
    const current = oh.get(proposed.key);
    if (current && current.recordSha256 !== proposed.recordSha256) {
      throw new Error("Existing record conflicts at " + proposed.key + "; no records were changed.");
    }
    if (!current) changes.push({ v: 1, kind: "put", record: proposed });
  }
  if (changes.length) {
    const identity = canonicalSha256({ changes, purpose });
    oh.store.commit({
      actorId: "peqnp.local-research",
      operationId: "op_peqnp_" + identity,
      expectedHead: verified.head,
      changes,
    });
  }
  return { inserted: changes.length, verification: oh.verify() };
}

export function initializeLedger(root) {
  const oh = openLedger(root, { initialize: true });
  try { return commitAdditive(oh, seedRecords(), "bootstrap-v1"); }
  finally { oh.store.close(); }
}

function object(value, name) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(name + " must be an object.");
}
function count(value, name) {
  if (!Number.isSafeInteger(value) || value < 0) throw new Error(name + " must be a nonnegative safe integer.");
}
function finiteCnf(value) {
  return Array.isArray(value) && value.length <= 16 && value.every(clause =>
    Array.isArray(clause) && clause.length <= 4 && clause.every(literal =>
      Number.isInteger(literal) && [1, -1, 2, -2].includes(literal)));
}

export function validateCalibration(report) {
  object(report, "Report");
  if (report.schema_version !== 1 || report.experiment !== "unit-propagation-calibration-v1" ||
      report.status !== "bounded-tested" || report.proof_status !== "informal-general-argument-not-kernel-checked") {
    throw new Error("Expected the bounded calibration-v1 report, without a formal-proof claim.");
  }
  object(report.corpus, "Corpus");
  const expected = { variables: 2, canonical_clauses: 16, formulas: 65536, assignments_per_formula: 4 };
  for (const [name, value] of Object.entries(expected)) {
    if (report.corpus[name] !== value) throw new Error("Unexpected calibration corpus " + name + ".");
  }
  if (report.grammar_size !== 8 || !Array.isArray(report.candidates) || report.candidates.length !== 8) {
    throw new Error("Expected exactly eight declared calibration candidates.");
  }
  const programs = new Set();
  for (const candidate of report.candidates) {
    object(candidate, "Candidate");
    if (!CALIBRATION_PROGRAMS.includes(candidate.program) || programs.has(candidate.program)) {
      throw new Error("Candidate programs must be the exact distinct calibration grammar.");
    }
    programs.add(candidate.program);
    for (const name of ["preservation_passes", "progress_passes", "checked_formulas", "work_units"]) count(candidate[name], name);
    if (candidate.checked_formulas !== report.corpus.formulas ||
        candidate.preservation_passes > candidate.checked_formulas ||
        candidate.progress_passes > candidate.checked_formulas) throw new Error("Inconsistent candidate coverage.");
    const counterexample = candidate.first_counterexample;
    if (candidate.preservation_passes === candidate.checked_formulas) {
      if (counterexample !== null) throw new Error("Preserved corpus cannot also declare a counterexample.");
    } else {
      object(counterexample, "Counterexample");
      if (!finiteCnf(counterexample.input) || !finiteCnf(counterexample.output) ||
          typeof counterexample.input_sat !== "boolean" || typeof counterexample.output_sat !== "boolean" ||
          counterexample.input_sat === counterexample.output_sat) throw new Error("Invalid preservation counterexample.");
    }
  }
  if (report.selected_program !== SELECTED_PROGRAM) throw new Error("Selected program must be the calibration unit-propagation rule.");
  const chosen = report.candidates.find(candidate => candidate.program === report.selected_program);
  if (chosen.preservation_passes !== chosen.checked_formulas || chosen.progress_passes !== chosen.checked_formulas) {
    throw new Error("Selected program does not pass both finite calibration properties.");
  }
  if (report.candidates.filter(candidate => candidate.preservation_passes === candidate.checked_formulas &&
      candidate.progress_passes === candidate.checked_formulas).length !== 1) {
    throw new Error("Calibration must identify one survivor of both finite properties.");
  }
  const falseClaim = report.false_completeness_claim;
  object(falseClaim, "False completeness claim");
  if (canonicalJson(falseClaim.input) !== canonicalJson(FALSE_COMPLETENESS_INPUT) ||
      falseClaim.satisfiable !== false || falseClaim.has_unit !== false ||
      falseClaim.unchanged !== true || falseClaim.rejected !== true) throw new Error("Planted false claim was not rejected.");
  if (!Array.isArray(report.limitations) || !report.limitations.length ||
      report.limitations.some(value => typeof value !== "string")) throw new Error("Report must state its limitations.");
  return report;
}

export function readReport(root, relativePath, validate = validateCalibration) {
  if (isAbsolute(relativePath)) throw new Error("Use a repository-relative result path.");
  const path = resolve(root, relativePath);
  const local = relative(resolve(root), path);
  if (local === ".." || local.startsWith(".." + sep)) throw new Error("Report path escapes the repository.");
  const info = lstatSync(path);
  if (!info.isFile() || info.isSymbolicLink() || info.size > MAX_REPORT_BYTES) throw new Error("Expected a bounded regular report file.");
  const real = relative(realpathSync(root), realpathSync(path));
  if (real === ".." || real.startsWith(".." + sep)) throw new Error("Report resolves outside the repository.");
  const bytes = readFileSync(path);
  if (bytes.length > MAX_REPORT_BYTES) throw new Error("Report grew beyond its size limit.");
  const report = validate(JSON.parse(bytes.toString("utf8")));
  return { report, path: local.split(sep).join("/"), sha256: sha256Hex(bytes) };
}

export function sourceManifest(root) {
  const files = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"];
  for (const directory of ["src", "tests"]) {
    const walk = path => {
      for (const entry of readdirSync(resolve(root, path), { withFileTypes: true })) {
        const child = path + "/" + entry.name;
        if (entry.isSymbolicLink()) throw new Error("Source identity refuses symbolic links.");
        if (entry.isDirectory()) walk(child);
        else if (entry.isFile() && entry.name.endsWith(".rs")) files.push(child);
      }
    };
    if (existing(resolve(root, directory))) walk(directory);
  }
  const manifest = files.sort().map(path => {
    const info = lstatSync(resolve(root, path));
    if (!info.isFile() || info.isSymbolicLink()) throw new Error("Source identity requires regular files.");
    return { path, sha256: sha256Hex(readFileSync(resolve(root, path))) };
  });
  return { files: manifest, sha256: canonicalSha256(manifest), meaning: "Source bytes observed at ingestion; not an attestation that these bytes produced the supplied result." };
}

export function calibrationRecords(input, source) {
  const identity = canonicalSha256({ reportSha256: input.sha256, sourceSha256: source.sha256 });
  const edition = "edition:calibration-" + input.sha256;
  const activity = "activity:calibration-" + identity;
  const assertion = "assertion:calibration-" + identity;
  return [
    record(edition, "edition", { path: input.path, sha256: input.sha256, mediaType: "application/json", report: input.report }),
    record(activity, "activity", {
      experiment: input.report.experiment,
      status: "recorded-observation",
      source,
      authority: "Local report ingestion; input claims require independent experimental review.",
    }, [edition, "inquiry:p-equals-np"]),
    record(assertion, "assertion", {
      statement: "statement:unit-propagation-preserves-sat",
      stance: "bounded-tested",
      scope: input.report.corpus,
      formalProofAccepted: false,
    }, [activity, "statement:unit-propagation-preserves-sat"]),
    record("evidence:calibration-" + identity, "evidence", {
      assertion,
      kind: "finite-exhaustive-calibration-report",
      selectedProgram: input.report.selected_program,
      reportSha256: input.sha256,
      limitations: input.report.limitations,
      formalProofAccepted: false,
    }, [assertion, edition]),
  ];
}

export function clueTransferRecords(input, source) {
  const identity = canonicalSha256({ reportSha256: input.sha256, sourceSha256: source.sha256 });
  const edition = "edition:clue-transfer-" + input.sha256;
  const activity = "activity:clue-transfer-" + identity;
  const assertion = "assertion:clue-transfer-" + identity;
  const statement = "statement:clue-transfer-v1-comparison";
  return [
    record(statement, "statement", {
      proposition: "The clue-transfer-v1 experiment compares the same deterministic DPLL solver with and without one root preprocessing pass using a frozen mined binary-pair rule library.",
      domain: "The fixed finite protocol and exact reports cited by each observation; no total-work improvement is asserted by this statement.",
    }, ["context:research-method"]),
    record(edition, "edition", { path: input.path, sha256: input.sha256, mediaType: "application/json", report: input.report }),
    record(activity, "activity", {
      experiment: input.report.experiment,
      status: "recorded-observation",
      source,
      authority: "Local report ingestion; input claims require independent experimental review.",
    }, [edition, "inquiry:p-equals-np"]),
    record(assertion, "assertion", {
      statement,
      stance: "bounded-observation",
      scope: { experiment: input.report.experiment, reportSha256: input.sha256 },
      formalProofAccepted: false,
    }, [activity, statement]),
    record("evidence:clue-transfer-" + identity, "evidence", {
      assertion,
      kind: "finite-clue-transfer-comparison-report",
      reportSha256: input.sha256,
      frozenLibrarySha256: canonicalSha256(input.report.frozen_library.rules),
      measuredSummary: input.report.summary,
      acquisitionWorkUnits: input.report.training.acquisition.work_units,
      acquisitionPlusTransferWorkUnits: input.report.acquisition_plus_transfer_work_units,
      limitations: input.report.limitations,
      interpretation: "Measured costs and outcomes remain observations, including regressions and unknowns; this is not a claim of a general speedup.",
      formalProofAccepted: false,
    }, [assertion, edition]),
  ];
}

export function recordCalibration(root, relativePath) {
  const input = readReport(root, relativePath);
  const source = sourceManifest(root);
  const oh = openLedger(root);
  try {
    for (const seed of seedRecords()) {
      if (oh.get(seed.key)?.recordSha256 !== seed.recordSha256) throw new Error("Bootstrap records are missing or changed; inspect before recording.");
    }
    return { reportSha256: input.sha256, ...commitAdditive(oh, calibrationRecords(input, source), "calibration-v1") };
  } finally { oh.store.close(); }
}

export function recordClueTransfer(root, relativePath) {
  const input = readReport(root, relativePath, validateClueTransfer);
  const protocolPath = resolve(root, "experiments/clue-transfer-protocol.md");
  const protocolFile = lstatSync(protocolPath);
  if (!protocolFile.isFile() || protocolFile.isSymbolicLink() || protocolFile.size > MAX_REPORT_BYTES ||
      sha256Hex(readFileSync(protocolPath)) !== input.report.protocol_sha256) {
    throw new Error("The local protocol does not match the report's fixed protocol identity.");
  }
  const source = sourceManifest(root);
  const oh = openLedger(root);
  try {
    for (const seed of seedRecords()) {
      if (oh.get(seed.key)?.recordSha256 !== seed.recordSha256) throw new Error("Bootstrap records are missing or changed; inspect before recording.");
    }
    return { reportSha256: input.sha256, ...commitAdditive(oh, clueTransferRecords(input, source), "clue-transfer-v1") };
  } finally { oh.store.close(); }
}

export async function main(args = process.argv.slice(2)) {
  const [command, input, ...extra] = args;
  if (extra.length || (!["record", "record-transfer"].includes(command) && input !== undefined)) throw new Error("Unexpected arguments.");
  if (command === "contract") return inspectContract();
  if (command === "init") {
    const { restoreSnapshot } = await import("./oh-snapshot.mjs");
    return restoreSnapshot(repositoryRoot);
  }
  if (command === "record" && input) return recordCalibration(repositoryRoot, input);
  if (command === "record-transfer" && input) return recordClueTransfer(repositoryRoot, input);
  if (command === "verify") {
    const oh = openLedger(repositoryRoot);
    try { return { database: ".oh/research.sqlite", space: SPACE, verification: oh.verify() }; }
    finally { oh.store.close(); }
  }
  throw new Error("Usage: bun scripts/oh-ledger.mjs <contract|init|verify|record relative-report.json|record-transfer relative-report.json>");
}

if (import.meta.main) {
  try { process.stdout.write(canonicalJson(await main()) + "\n"); }
  catch (error) { process.stderr.write(String(error.message ?? error) + "\n"); process.exitCode = 1; }
}
