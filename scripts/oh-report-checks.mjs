import { createHash } from "node:crypto";
import { canonicalJson } from "@hraness/oh";

// Structural and numeric checks shared by the indexed-transfer and
// implication-calibration validators. They mirror oh-transfer-report.mjs,
// whose v1 validator intentionally keeps its own frozen copy.
export const BUDGET = 1_000_000;
export const WORK_EVENTS = ["formula_checks", "clause_reads", "literal_reads", "clause_writes", "literal_writes", "assignments", "pair_checks", "rule_attempts", "search_nodes"];
export const COMPARISON_FIELDS = ["both_complete", "left_better", "tied", "left_worse", "left_complete_only", "right_complete_only", "both_unknown"];
const TRAINING_CLAUSES = [[1, 2], [1, -2], [-1, 2], [-1, -2]];

export function require(condition, message) { if (!condition) throw new Error(message); }
export function keys(value, fields, name) {
  require(value && typeof value === "object" && !Array.isArray(value) &&
    canonicalJson(Object.keys(value).sort()) === canonicalJson([...fields].sort()), "Unexpected " + name + " fields.");
}
export function count(value) { require(Number.isSafeInteger(value) && value >= 0, "Expected a nonnegative safe event count."); }
export function sum(values) {
  const result = values.reduce((total, value) => { count(value); return total + value; }, 0);
  count(result);
  return result;
}
export function work(value) {
  keys(value, ["work_units", ...WORK_EVENTS], "work counter");
  count(value.work_units);
  require(value.work_units === sum(WORK_EVENTS.map(key => value[key])), "Event categories do not sum to work_units.");
}
export function literal(value, variables) { return Number.isInteger(value) && value !== 0 && Math.abs(value) <= variables; }
export function equal(actual, expected, message) { require(canonicalJson(actual) === canonicalJson(expected), message); }
export function distinctLiterals(values, variables) {
  return Array.isArray(values) && values.length <= 2 * variables && values.every(value => literal(value, variables)) &&
    new Set(values).size === values.length;
}

// The v1 accepted rules: every entailed two-premise binary implication over
// the fixed two-variable training clauses, in mining order.
export function acceptedRules() {
  const litValue = (lit, assignment) => Boolean(assignment & (1 << (Math.abs(lit) - 1))) === (lit > 0);
  const accepted = [];
  for (let i = 0; i < TRAINING_CLAUSES.length; i++) for (let j = i + 1; j < TRAINING_CLAUSES.length; j++) for (const conclusion of [1, -1, 2, -2]) {
    const rule = { premises: [TRAINING_CLAUSES[i], TRAINING_CLAUSES[j]], conclusion };
    const models = [0, 1, 2, 3].filter(assignment => rule.premises.every(clause => clause.some(lit => litValue(lit, assignment))));
    if (models.every(assignment => litValue(conclusion, assignment))) accepted.push(rule);
  }
  return accepted;
}

export function validateFrozenRules(rules) {
  const accepted = acceptedRules();
  require(accepted.length === 4, "Accepted-rule enumeration must yield exactly four rules.");
  require(Array.isArray(rules) && rules.length === 4, "Frozen library must contain exactly four rule instances.");
  for (const rule of rules) keys(rule, ["premises", "conclusion"], "frozen rule");
  equal(rules, accepted, "Frozen library differs from the accepted mining rules.");
}

export function validateAcquisition(acquisition) {
  work(acquisition);
  require(acquisition.assignments === 96 && acquisition.rule_attempts === 24, "Incomplete training coverage counters.");
}

// Shared DPLL arm shape from transfer-v1: outcome, preprocessing, residual, total, derived units.
export function solverArm(arm, { referenceSat, variables, baseline = false }) {
  keys(arm, ["outcome", "preprocessing", "residual", "total_work_units", "derived_units"], "solver arm");
  work(arm.preprocessing);
  work(arm.residual);
  require(arm.total_work_units === sum([arm.preprocessing.work_units, arm.residual.work_units]) && arm.total_work_units <= BUDGET, "Arm total is inconsistent or exceeds the shared budget.");
  require(["sat", "unsat", "unknown-budget"].includes(arm.outcome), "Invalid solver outcome.");
  if (arm.outcome === "unknown-budget") require(arm.total_work_units === BUDGET, "Unknown-budget requires exhausted work budget.");
  else require((arm.outcome === "sat") === referenceSat, "Solver decision disagrees with the supplied reference label.");
  require(distinctLiterals(arm.derived_units, variables), "Invalid derived-unit list.");
  if (baseline) require(arm.preprocessing.work_units === 0 && arm.derived_units.length === 0, "Baseline cannot include learned preprocessing.");
}

// Cost comparison restricted to completed pairs; null marks an unknown arm.
export function comparison(pairs) {
  const result = Object.fromEntries(COMPARISON_FIELDS.map(key => [key, 0]));
  const add = key => { result[key] = sum([result[key], 1]); };
  for (const [left, right] of pairs) {
    if (left !== null && right !== null) {
      add("both_complete");
      add(left < right ? "left_better" : left > right ? "left_worse" : "tied");
    } else if (left !== null) add("left_complete_only");
    else if (right !== null) add("right_complete_only");
    else add("both_unknown");
  }
  return result;
}

export function validateLimitations(limitations) {
  require(Array.isArray(limitations) && limitations.length > 0 && limitations.every(value => typeof value === "string" && value.length > 0), "Report must state its limitations.");
}

// ---------------------------------------------------------------------------
// Reference oracle above the truth-table bound (shared by every future
// validator; the five frozen validators do not use it). The pins duplicate
// `src/oracle.rs`; `oh-report-checks.test.mjs` requires the two to agree.
// ---------------------------------------------------------------------------
export const ORACLE_PINS = Object.freeze({
  name: "cadical",
  version: "3.0.1",
  path: "/opt/homebrew/bin/cadical",
  sha256: "601c9fa8ba5d09fd81bb00c89b3e54832f138bccc3422bd8652e8cda4d74d1fa",
  arguments: Object.freeze(["-q", "--lrat", "--binary=false"]),
});
export const TRUTH_TABLE_MAX_VARIABLES = 12;
export const PROOF_TEXT_CAP_BYTES = 2_097_152;
export const ANSWER_FILE_CAP_BYTES = 16_777_216;
export const REFERENCE_MODEL = "truth-table<=12;definition;oracle:cadical-3.0.1+model-check+lrat-check";
export const UNKNOWN_REASONS = ["conflict-limit", "proof-over-cap"];
const DIGEST = /^[0-9a-f]{64}$/;
const LRAT_TEXT = /^[0-9 d\n-]*$/;
const DECIMAL = /^(0|[1-9][0-9]*)$/;

export function sha256Hex(text) { return createHash("sha256").update(text).digest("hex"); }

// The exact `write_dimacs` layout: header, then each clause's literals each
// followed by one space, then `0` and a newline; an empty clause is `0`.
export function writeDimacs(input, variables) {
  let text = `p cnf ${variables} ${input.length}\n`;
  for (const clause of input) text += clause.map(lit => `${lit} `).join("") + "0\n";
  return text;
}

// `(a * 1000 + b / 2) / b` in BigInt, rendered `q.rrr`; null when b is zero.
export function integerThousandths(numerator, denominator) {
  count(numerator);
  count(denominator);
  if (denominator === 0) return null;
  const scaled = (BigInt(numerator) * 1000n + BigInt(denominator) / 2n) / BigInt(denominator);
  return `${scaled / 1000n}.${String(scaled % 1000n).padStart(3, "0")}`;
}

// A RatioPair object or null; the thousandths string must be the integer rounding.
export function ratioPair(value, name = "ratio") {
  if (value === null) return null;
  keys(value, ["numerator", "denominator", "thousandths"], name);
  count(value.numerator);
  count(value.denominator);
  require(value.denominator > 0, name + " has a zero denominator but is not null.");
  require(value.thousandths === integerThousandths(value.numerator, value.denominator), name + " thousandths disagree with integer rounding.");
  return value;
}

function integersOnly(value, where) {
  if (typeof value === "number") require(Number.isSafeInteger(value), "Non-integer number in " + where + ".");
  else if (Array.isArray(value)) value.forEach((item, index) => integersOnly(item, where + "[" + index + "]"));
  else if (value && typeof value === "object") for (const [key, item] of Object.entries(value)) integersOnly(item, where + "." + key);
}

function ascendingCompleteModel(model, variables) {
  return Array.isArray(model) && model.length === variables &&
    model.every((lit, index) => Number.isSafeInteger(lit) && Math.abs(lit) === index + 1);
}

function satisfies(input, model) {
  const positive = new Map(model.map(lit => [Math.abs(lit), lit > 0]));
  return input.every(clause => clause.some(lit => positive.get(Math.abs(lit)) === (lit > 0)));
}

// Structural check of an oracle-answer file: pinned oracle block, cap,
// sorted unique digests, integer-only values, and each entry's payload.
// Returns { conflictLimit, entries: Map<digest, entry> }.
export function validateAnswerFile(file, byteLength = null) {
  if (byteLength !== null) require(byteLength <= ANSWER_FILE_CAP_BYTES, "Oracle-answer file exceeds " + ANSWER_FILE_CAP_BYTES + " bytes.");
  keys(file, ["schema_version", "experiment", "oracle", "proof_text_cap_bytes", "entries"], "oracle-answer file");
  integersOnly(file, "answer file");
  require(file.schema_version === 1, "Unsupported oracle-answer schema.");
  require(typeof file.experiment === "string" && /^[A-Za-z0-9._-]+$/.test(file.experiment), "Invalid experiment id.");
  keys(file.oracle, ["name", "version", "path", "sha256", "arguments"], "oracle block");
  for (const field of ["name", "version", "path", "sha256"]) require(file.oracle[field] === ORACLE_PINS[field], "Oracle " + field + " differs from the pin.");
  const args = file.oracle.arguments;
  require(Array.isArray(args) && args.length === 5 && args.slice(0, 3).every((arg, i) => arg === ORACLE_PINS.arguments[i]) && args[3] === "-c" && DECIMAL.test(args[4]), "Oracle arguments differ from the pinned invocation.");
  const conflictLimit = Number(args[4]);
  count(conflictLimit);
  require(file.proof_text_cap_bytes === PROOF_TEXT_CAP_BYTES, "Proof text cap differs from the pin.");
  require(Array.isArray(file.entries), "Entries must be an array.");
  const entries = new Map();
  let previous = "";
  for (const entry of file.entries) {
    const common = ["dimacs_sha256", "variables", "clauses", "conflict_limit", "label"];
    require(entry && typeof entry === "object" && DIGEST.test(entry.dimacs_sha256), "Invalid entry digest.");
    require(entry.dimacs_sha256 > previous, "Entries are not sorted and unique by digest.");
    previous = entry.dimacs_sha256;
    count(entry.variables);
    count(entry.clauses);
    require(entry.conflict_limit === conflictLimit, "Entry conflict limit differs from the file's invocation.");
    const name = "entry " + entry.dimacs_sha256;
    if (entry.label === "sat") {
      keys(entry, [...common, "model"], name);
      require(ascendingCompleteModel(entry.model, entry.variables), name + " model is not ascending and complete.");
    } else if (entry.label === "unsat") {
      keys(entry, [...common, "lrat_sha256", "lrat_bytes", "lrat"], name);
      require(typeof entry.lrat === "string" && LRAT_TEXT.test(entry.lrat), name + " lrat text has a character outside [0-9 d\\n-].");
      require(entry.lrat_bytes === Buffer.byteLength(entry.lrat) && entry.lrat_bytes <= PROOF_TEXT_CAP_BYTES, name + " lrat_bytes disagrees with the text or exceeds the cap.");
      require(entry.lrat_sha256 === sha256Hex(entry.lrat), name + " lrat_sha256 disagrees with the text.");
    } else if (entry.label === "unknown") {
      require(UNKNOWN_REASONS.includes(entry.reason), name + " has an unknown reason.");
      if (entry.reason === "proof-over-cap") {
        keys(entry, [...common, "reason", "lrat_bytes"], name);
        require(Number.isSafeInteger(entry.lrat_bytes) && entry.lrat_bytes > PROOF_TEXT_CAP_BYTES, name + " proof-over-cap within the cap.");
      } else keys(entry, [...common, "reason"], name);
    } else throw new Error(name + " has an invalid label.");
    entries.set(entry.dimacs_sha256, entry);
  }
  return { conflictLimit, entries };
}

const indexed = new WeakMap();
function answerIndex(answers) {
  if (answers instanceof Map) return answers;
  let index = indexed.get(answers);
  if (!index) {
    index = validateAnswerFile(answers).entries;
    indexed.set(answers, index);
  }
  return index;
}

// One case's `reference` object. `row` supplies `input` (raw clauses) and
// `variables`; `pins` is ORACLE_PINS; `limits` is { conflicts }; `answers`
// is the parsed oracle-answer file (or its entry Map). Returns the label.
export function oracleReference(reference, row, pins, limits, answers) {
  require(reference && typeof reference === "object", "Missing reference.");
  const { input, variables } = row;
  require(Array.isArray(input) && Number.isSafeInteger(variables) && variables >= 0, "Invalid case shape.");
  if (reference.kind === "truth-table") {
    keys(reference, ["kind", "model_count", "sat", "backbone", "work"], "truth-table reference");
    require(variables <= TRUTH_TABLE_MAX_VARIABLES, "Truth-table reference above the truth-table bound.");
    work(reference.work);
    require(reference.work.assignments === 2 ** variables, "Incomplete truth-table coverage.");
    count(reference.model_count);
    require(reference.sat === (reference.model_count > 0), "Truth-table sat flag disagrees with the model count.");
    if (reference.model_count === 0) require(reference.backbone === null, "Backbone must be null on an unsatisfiable formula.");
    else {
      require(distinctLiterals(reference.backbone, variables), "Invalid backbone list.");
      require(reference.backbone.every((lit, i) => i === 0 || Math.abs(reference.backbone[i - 1]) < Math.abs(lit)), "Backbone is not in variable order.");
    }
    return reference.sat ? "sat" : "unsat";
  }
  if (reference.kind === "definition") {
    require(variables > TRUTH_TABLE_MAX_VARIABLES, "Definition reference at or below the truth-table bound.");
    if (reference.label === "unsat") {
      keys(reference, ["kind", "label", "certificate", "work"], "definition reference");
      keys(reference.certificate, ["kind", "clause_index"], "definition certificate");
      require(reference.certificate.kind === "empty-clause", "Unknown definition certificate.");
      count(reference.certificate.clause_index);
      const clause = input[reference.certificate.clause_index];
      require(Array.isArray(clause) && clause.length === 0, "Definition certificate does not name an empty clause.");
      work(reference.work);
      return "unsat";
    }
    require(reference.label === "sat", "Invalid definition label.");
    keys(reference, ["kind", "label", "model", "model_check"], "definition reference");
    require(input.length === 0, "Definition sat row on a formula with clauses.");
    equal(reference.model, Array.from({ length: variables }, (_, i) => -(i + 1)), "Definition model is not the all-false assignment.");
    keys(reference.model_check, ["valid", "work"], "model check");
    require(reference.model_check.valid === true, "Definition model check must be valid.");
    work(reference.model_check.work);
    return "sat";
  }
  require(reference.kind === "oracle", "Unknown reference kind.");
  require(variables > TRUTH_TABLE_MAX_VARIABLES, "Oracle reference at or below the truth-table bound.");
  const { label } = reference;
  require(["sat", "unsat", "unknown"].includes(label), "Invalid oracle label.");
  const fields = ["kind", "label", ...(label === "unknown" ? ["reason"] : []), "dimacs_sha256", "conflict_limit", "model", "model_check", "proof_check", "oracle"];
  keys(reference, fields, "oracle reference");
  const digest = sha256Hex(writeDimacs(input, variables));
  require(reference.dimacs_sha256 === digest, "Oracle digest does not identify the case's DIMACS text.");
  require(reference.conflict_limit === limits.conflicts, "Oracle conflict limit differs from the protocol's.");
  keys(reference.oracle, ["name", "version", "sha256"], "oracle identity");
  for (const field of ["name", "version", "sha256"]) require(reference.oracle[field] === pins[field], "Oracle " + field + " differs from the pin.");
  const entry = answerIndex(answers).get(digest);
  require(entry !== undefined && entry.label === label, "No committed answer with this digest and label.");
  if (label === "sat") {
    require(ascendingCompleteModel(reference.model, variables), "Oracle model is not ascending and complete.");
    require(satisfies(input, reference.model), "Oracle model leaves a clause unsatisfied.");
    equal(reference.model, entry.model, "Oracle model differs from the committed answer.");
    keys(reference.model_check, ["valid", "work"], "model check");
    require(reference.model_check.valid === true, "Oracle model check must be valid.");
    work(reference.model_check.work);
    require(reference.proof_check === null, "A sat row carries no proof check.");
  } else if (label === "unsat") {
    require(reference.model === null && reference.model_check === null, "An unsat row carries no model.");
    keys(reference.proof_check, ["valid", "lrat_sha256", "lrat_bytes", "lemmas", "deletions", "work"], "proof check");
    require(reference.proof_check.valid === true, "Oracle proof check must be valid.");
    count(reference.proof_check.lemmas);
    count(reference.proof_check.deletions);
    require(reference.proof_check.lrat_sha256 === sha256Hex(entry.lrat) && reference.proof_check.lrat_bytes === Buffer.byteLength(entry.lrat), "Proof digest or length differs from the committed proof text.");
    work(reference.proof_check.work);
  } else {
    require(UNKNOWN_REASONS.includes(reference.reason) && reference.reason === entry.reason, "Unknown reason differs from the committed answer.");
    require(reference.model === null && reference.model_check === null && reference.proof_check === null, "An unknown row carries no certificate.");
  }
  return label;
}
