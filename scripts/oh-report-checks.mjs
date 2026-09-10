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
