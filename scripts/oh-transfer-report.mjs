import { canonicalJson } from "@hraness/oh";

export const TRANSFER_PROTOCOL_SHA256 = "5a4e14e2f637e6bc6ee3cb691f76d84e556d14d4eb126e55ecd387d59e89d561";
const BUDGET = 1_000_000;
const SEEDS = [7, 41, 2026, 65537];
const WORK_EVENTS = ["formula_checks", "clause_reads", "literal_reads", "clause_writes", "literal_writes", "assignments", "pair_checks", "rule_attempts", "search_nodes"];
const SUMMARY_FIELDS = ["cases", "baseline_work_units", "transfer_preprocessing_work_units", "transfer_residual_work_units", "transfer_work_units", "reference_work_units", "derived_units", "baseline_search_nodes", "transfer_search_nodes", "baseline_unknown", "transfer_unknown", "both_complete", "transfer_better", "tied", "transfer_worse"];

function require(condition, message) { if (!condition) throw new Error(message); }
function keys(value, fields, name) {
  require(value && typeof value === "object" && !Array.isArray(value) &&
    canonicalJson(Object.keys(value).sort()) === canonicalJson([...fields].sort()), "Unexpected " + name + " fields.");
}
function count(value) { require(Number.isSafeInteger(value) && value >= 0, "Expected a nonnegative safe event count."); }
function sum(values) {
  const result = values.reduce((total, value) => { count(value); return total + value; }, 0);
  count(result);
  return result;
}
function work(value) {
  keys(value, ["work_units", ...WORK_EVENTS], "work counter");
  count(value.work_units);
  require(value.work_units === sum(WORK_EVENTS.map(key => value[key])), "Event categories do not sum to work_units.");
}
function literal(value, variables) { return Number.isInteger(value) && value !== 0 && Math.abs(value) <= variables; }
function cnf(value, variables) {
  return Array.isArray(value) && value.length <= 100 && value.every(clause => Array.isArray(clause) &&
    clause.length <= variables && clause.every(lit => literal(lit, variables)) &&
    new Set(clause.map(Math.abs)).size === clause.length);
}
function equal(actual, expected, message) { require(canonicalJson(actual) === canonicalJson(expected), message); }

function expectedCases() {
  const cases = new Map();
  const add = (id, family, variables, seed = null, density = null) => cases.set(id, { family, variables, seed, density });
  for (const seed of SEEDS) for (const variables of [6, 8, 10, 12]) for (const density of [2, 4, 6]) {
    for (const family of ["random-3cnf", "random-mixed"]) add(`${family}-n${variables}-d${density}-s${seed}`, family, variables, seed, density);
  }
  for (const variables of [6, 8, 10, 12]) for (const family of ["forced-true", "forced-false", "ternary-cube", "hidden-backbone"]) {
    add(`${family}-n${variables}`, family, variables);
  }
  for (let variables = 5; variables <= 12; variables++) add(`parity-cycle-n${variables}`, "parity-cycle", variables);
  for (const [pigeons, holes] of [[3, 2], [4, 3]]) add(`pigeonhole-${pigeons}-${holes}`, "pigeonhole", pigeons * holes);
  return cases;
}

function validateTraining(training, library) {
  keys(training, ["variables", "premise_pairs", "conclusions_per_pair", "candidate_implications", "assignments_per_candidate", "accepted_rules", "acquisition", "candidates"], "training");
  for (const [key, value] of Object.entries({ variables: 2, premise_pairs: 6, conclusions_per_pair: 4, candidate_implications: 24, assignments_per_candidate: 4, accepted_rules: 4 })) {
    require(training[key] === value, "Unexpected fixed training scope.");
  }
  work(training.acquisition);
  require(training.acquisition.assignments === 96 && training.acquisition.rule_attempts === 24, "Incomplete training coverage counters.");
  require(Array.isArray(training.candidates) && training.candidates.length === 24, "Expected all 24 mining candidates.");
  const clauses = [[1, 2], [1, -2], [-1, 2], [-1, -2]];
  const accepted = [];
  const litValue = (lit, assignment) => Boolean(assignment & (1 << (Math.abs(lit) - 1))) === (lit > 0);
  let index = 0;
  for (let i = 0; i < clauses.length; i++) for (let j = i + 1; j < clauses.length; j++) for (const conclusion of [1, -1, 2, -2]) {
    const candidate = training.candidates[index++];
    keys(candidate, ["rule", "entailed", "satisfying_assignments", "first_counterexample_assignment"], "mining candidate");
    const rule = { premises: [clauses[i], clauses[j]], conclusion };
    equal(candidate.rule, rule, "Mining candidates differ from the fixed grammar or order.");
    const models = [0, 1, 2, 3].filter(assignment => rule.premises.every(clause => clause.some(lit => litValue(lit, assignment))));
    const counterexample = models.find(assignment => !litValue(conclusion, assignment)) ?? null;
    require(candidate.satisfying_assignments === models.length && candidate.entailed === (counterexample === null) &&
      candidate.first_counterexample_assignment === counterexample, "Mining conclusion or counterexample disagrees with its complete tiny domain.");
    if (counterexample === null) accepted.push(rule);
  }
  keys(library, ["rules", "rule_instances", "schemas_up_to_signed_renaming", "schema", "frozen_before_heldout"], "frozen library");
  require(library.rule_instances === 4 && library.schemas_up_to_signed_renaming === 1 && library.schema === "known-binary-resolution-to-unit" &&
    library.frozen_before_heldout === true, "Unexpected frozen-library contract.");
  equal(library.rules, accepted, "Frozen library differs from the accepted mining rules.");
}

function validateArm(arm, row, baseline) {
  keys(arm, ["outcome", "preprocessing", "residual", "total_work_units", "derived_units"], "solver arm");
  work(arm.preprocessing);
  work(arm.residual);
  require(arm.total_work_units === sum([arm.preprocessing.work_units, arm.residual.work_units]) && arm.total_work_units <= BUDGET, "Arm total is inconsistent or exceeds the shared budget.");
  require(["sat", "unsat", "unknown-budget"].includes(arm.outcome), "Invalid solver outcome.");
  if (arm.outcome === "unknown-budget") require(arm.total_work_units === BUDGET, "Unknown-budget requires exhausted work budget.");
  else require((arm.outcome === "sat") === row.reference_sat, "Solver decision disagrees with the supplied reference label.");
  require(Array.isArray(arm.derived_units) && arm.derived_units.length <= 2 * row.variables &&
    arm.derived_units.every(value => literal(value, row.variables)) && new Set(arm.derived_units).size === arm.derived_units.length,
  "Invalid derived-unit list.");
  if (baseline) require(arm.preprocessing.work_units === 0 && arm.derived_units.length === 0, "Baseline cannot include learned preprocessing.");
}

function summarize(rows) {
  const result = Object.fromEntries(SUMMARY_FIELDS.map(key => [key, 0]));
  const add = (key, value) => { result[key] = sum([result[key], value]); };
  for (const row of rows) {
    add("cases", 1);
    add("baseline_work_units", row.baseline.total_work_units);
    add("transfer_preprocessing_work_units", row.transfer.preprocessing.work_units);
    add("transfer_residual_work_units", row.transfer.residual.work_units);
    add("transfer_work_units", row.transfer.total_work_units);
    add("reference_work_units", row.reference_work.work_units);
    add("derived_units", row.transfer.derived_units.length);
    add("baseline_search_nodes", row.baseline.residual.search_nodes);
    add("transfer_search_nodes", row.transfer.residual.search_nodes);
    const baselineUnknown = row.baseline.outcome === "unknown-budget";
    const transferUnknown = row.transfer.outcome === "unknown-budget";
    add("baseline_unknown", Number(baselineUnknown));
    add("transfer_unknown", Number(transferUnknown));
    if (!baselineUnknown && !transferUnknown) {
      add("both_complete", 1);
      add(row.transfer.total_work_units < row.baseline.total_work_units ? "transfer_better" :
        row.transfer.total_work_units > row.baseline.total_work_units ? "transfer_worse" : "tied", 1);
    }
  }
  return result;
}

export function validateClueTransfer(report) {
  keys(report, ["schema_version", "experiment", "status", "protocol_sha256", "proof_status", "training", "frozen_library", "corpus", "arm_budget_work_units", "observations", "summary", "acquisition_plus_transfer_work_units", "families", "limitations"], "clue-transfer report");
  require(report.schema_version === 1 && report.experiment === "clue-transfer-v1" && report.status === "bounded-tested" &&
    report.proof_status === "informal-general-argument-not-kernel-checked" && report.protocol_sha256 === TRANSFER_PROTOCOL_SHA256,
  "Expected the fixed bounded clue-transfer-v1 protocol without a formal-proof claim.");
  validateTraining(report.training, report.frozen_library);
  equal(report.corpus, { generator: "xorshift64-v1", seeds: SEEDS, cases: 122, max_variables: 12 }, "Unexpected clue-transfer corpus.");
  require(report.arm_budget_work_units === BUDGET, "Unexpected shared arm budget.");
  require(Array.isArray(report.observations) && report.observations.length === 122, "Expected every fixed held-out case.");
  const expected = expectedCases();
  for (const row of report.observations) {
    keys(row, ["id", "family", "variables", "used_variables", "seed", "density", "input", "reference_sat", "reference_work", "baseline", "transfer"], "observation");
    const metadata = expected.get(row.id);
    require(metadata !== undefined, "Unexpected or duplicate held-out case identity.");
    equal({ family: row.family, variables: row.variables, seed: row.seed, density: row.density }, metadata, "Case metadata differs from the fixed corpus.");
    expected.delete(row.id);
    require(cnf(row.input, row.variables), "Invalid bounded held-out CNF.");
    require(row.used_variables === new Set(row.input.flat().map(Math.abs)).size, "Used-variable count differs from the CNF.");
    require(typeof row.reference_sat === "boolean", "Reference label must be Boolean.");
    work(row.reference_work);
    require(row.reference_work.assignments === 2 ** row.variables && row.reference_work.formula_checks === 2 ** row.variables, "Incomplete reference coverage counters.");
    validateArm(row.baseline, row, true);
    validateArm(row.transfer, row, false);
  }
  equal(report.summary, summarize(report.observations), "Summary differs from per-case observations.");
  const families = [...new Set(report.observations.map(row => row.family))].sort();
  keys(report.families, families, "family summaries");
  for (const family of families) equal(report.families[family], summarize(report.observations.filter(row => row.family === family)), "Family summary differs from observations.");
  require(report.acquisition_plus_transfer_work_units === sum([report.training.acquisition.work_units, report.summary.transfer_work_units]), "Acquisition-plus-online total is inconsistent.");
  require(Array.isArray(report.limitations) && report.limitations.length > 0 && report.limitations.every(value => typeof value === "string" && value.length > 0), "Report must state its limitations.");
  return report;
}
