import {
  BUDGET, COMPARISON_FIELDS, comparison, count, equal, keys, literal, require, solverArm, sum, validateAcquisition,
  validateFrozenRules, validateLimitations, work,
} from "./oh-report-checks.mjs";

export const INDEXED_PROTOCOL_SHA256 = "8e3239abca6796e86dc4463b51bdbfb081a1f3c33e14d8bc632e31332910a043";
const SEEDS = [1201, 5179, 65539, 104729];
const VARIABLES = [6, 8, 10, 12];
const DENSITIES = [2, 4, 6];
const RANDOM_FAMILIES = [[1n, "random-2cnf", [2]], [2n, "random-3cnf", [3]], [3n, "random-mixed", [2, 3]]];
const MAX_CLAUSES = 2 * 11 * 8;
const ARM_SUMMARY_FIELDS = ["preprocessing_work_units", "residual_work_units", "total_work_units", "search_nodes", "derived_units", "complete", "unknown"];
const INDEX_FIELDS = ["entries", "key_comparisons", "group_lookups", "entry_swaps", "peak_stored_scalar_cells"];
const COMPARISONS = [["generic_vs_baseline", "generic", "baseline"], ["indexed_vs_baseline", "indexed", "baseline"], ["indexed_vs_generic", "indexed", "generic"]];

function cnf(value, variables) {
  return Array.isArray(value) && value.length <= MAX_CLAUSES && value.every(clause => Array.isArray(clause) &&
    clause.length >= 2 && clause.length <= 3 && clause.every(lit => literal(lit, variables)) &&
    new Set(clause.map(Math.abs)).size === clause.length);
}

// The fixed 160-case corpus: 144 seeded random cases identified by their
// protocol parameters and effective seed, and 16 exactly regenerated
// duplicate-pressure controls.
function expectedCases() {
  const cases = new Map();
  for (const seed of SEEDS) for (const variables of VARIABLES) for (const density of DENSITIES) for (const [tag, family, widths] of RANDOM_FAMILIES) {
    const effective = BigInt(seed) ^ (BigInt(variables) << 32n) ^ (BigInt(density) << 40n) ^ (tag << 48n);
    cases.set(`${family}-n${variables}-d${density}-s${seed}`, {
      metadata: { family, variables, seed, effective_seed: effective.toString(), density, repetitions: null, target_sign: null },
      clauses: variables * density, widths, input: null,
    });
  }
  for (const variables of VARIABLES) for (const sign of [1, -1]) for (const repetitions of [1, 8]) {
    const family = sign > 0 ? "duplicate-star-positive" : "duplicate-star-negative";
    const input = [];
    for (let repetition = 0; repetition < repetitions; repetition++) {
      for (let pivot = 1; pivot < variables; pivot++) input.push([variables * sign, pivot], [variables * sign, -pivot]);
    }
    cases.set(`${family}-n${variables}-r${repetitions}`, {
      metadata: { family, variables, seed: null, effective_seed: null, density: null, repetitions, target_sign: sign },
      clauses: input.length, widths: [2], input,
    });
  }
  return cases;
}

// An arm exhausted inside preprocessing reports unknown, no residual work and
// no derived units; any other signature means its preprocessing completed.
function preprocessingCompleted(arm) {
  return arm.outcome !== "unknown-budget" || arm.residual.work_units > 0 || arm.derived_units.length > 0;
}
function equivalence(generic, indexed) {
  if (generic.outcome !== "unknown-budget" && indexed.outcome !== "unknown-budget") return "complete-arms-exact";
  if (preprocessingCompleted(generic) && preprocessingCompleted(indexed)) return "preprocessing-exact";
  return "not-comparable-budget";
}

function validateIndex(index, arm, input) {
  keys(index, INDEX_FIELDS, "index counter");
  for (const key of INDEX_FIELDS) count(index[key]);
  require(index.key_comparisons === arm.preprocessing.pair_checks, "Index key comparisons differ from the charged pair checks.");
  // Every heapsort swap follows a key comparison, and no arm can store more
  // entries than twice its eligible binary clauses, whether or not it finished.
  require(index.entry_swaps <= index.key_comparisons, "Index swaps exceed the charged key comparisons.");
  const eligible = input.filter(clause => clause.length === 2).length;
  require(index.entries <= 2 * eligible, "Index entry count exceeds twice the eligible binary clauses.");
  if (!preprocessingCompleted(arm)) return;
  require(index.entries === 2 * eligible, "Index entry count differs from twice the eligible binary clauses.");
  require(index.group_lookups >= index.entries, "A completed scan reads every index entry at least once.");
  // Peak cells count the entry array plus one witness triple and one unit per derived unit.
  require(index.peak_stored_scalar_cells === sum([3 * index.entries, 4 * arm.derived_units.length]), "Peak stored cells differ from the entry, witness and unit arrays.");
}

function armSummary(arms) {
  const result = Object.fromEntries(ARM_SUMMARY_FIELDS.map(key => [key, 0]));
  const add = (key, value) => { result[key] = sum([result[key], value]); };
  for (const arm of arms) {
    add("preprocessing_work_units", arm.preprocessing.work_units);
    add("residual_work_units", arm.residual.work_units);
    add("total_work_units", arm.total_work_units);
    add("search_nodes", arm.residual.search_nodes);
    add("derived_units", arm.derived_units.length);
    add(arm.outcome === "unknown-budget" ? "unknown" : "complete", 1);
  }
  return result;
}
const arms = row => ({ baseline: row.baseline, generic: row.generic, indexed: row.indexed.arm });
const cost = arm => arm.outcome === "unknown-budget" ? null : arm.total_work_units;

function summarize(rows) {
  return {
    cases: rows.length,
    reference_work_units: sum(rows.map(row => row.reference_work.work_units)),
    arms: Object.fromEntries(["baseline", "generic", "indexed"].map(name => [name, armSummary(rows.map(row => arms(row)[name]))])),
    comparisons: Object.fromEntries(COMPARISONS.map(([name, left, right]) =>
      [name, comparison(rows.map(row => [cost(arms(row)[left]), cost(arms(row)[right])]))])),
  };
}

function validateSummaryShape(summary) {
  keys(summary, ["cases", "reference_work_units", "arms", "comparisons"], "summary");
  keys(summary.arms, ["baseline", "generic", "indexed"], "summary arms");
  for (const arm of Object.values(summary.arms)) keys(arm, ARM_SUMMARY_FIELDS, "arm summary");
  keys(summary.comparisons, COMPARISONS.map(([name]) => name), "summary comparisons");
  for (const value of Object.values(summary.comparisons)) keys(value, COMPARISON_FIELDS, "comparison");
}

export function validateIndexedTransfer(report) {
  keys(report, ["schema_version", "experiment", "status", "proof_status", "protocol_sha256", "arm_budget", "counter_model", "limitations", "frozen_library", "setup", "corpus", "observations", "summary", "setup_plus_online", "families"], "indexed-transfer report");
  require(report.schema_version === 1 && report.experiment === "indexed-transfer-v1" && report.status === "bounded-tested" &&
    report.proof_status === "no-formal-proof" && report.protocol_sha256 === INDEXED_PROTOCOL_SHA256,
  "Expected the fixed bounded indexed-transfer-v1 protocol without a formal-proof claim.");
  require(report.arm_budget === BUDGET, "Unexpected shared arm budget.");
  require(report.counter_model === "transfer-v1-events-with-metered-array-index", "Unexpected counter model.");
  validateLimitations(report.limitations);
  keys(report.frozen_library, ["source_rule_instances", "compiled_schemas", "rules"], "frozen library");
  require(report.frozen_library.source_rule_instances === 4 && report.frozen_library.compiled_schemas === 1, "Unexpected frozen-library contract.");
  validateFrozenRules(report.frozen_library.rules);
  keys(report.setup, ["acquisition", "compilation"], "setup");
  validateAcquisition(report.setup.acquisition);
  work(report.setup.compilation);
  require(report.setup.compilation.rule_attempts === 4 && report.setup.compilation.work_units <= BUDGET, "Compilation must check every source rule within the budget.");
  equal(report.corpus, { cases: 160, seeds: SEEDS, variables: VARIABLES, densities: DENSITIES, random_cases: 144, duplicate_controls: 16, exact_v1_overlap: 0 }, "Unexpected indexed-transfer corpus.");
  require(Array.isArray(report.observations) && report.observations.length === 160, "Expected every fixed held-out case.");
  const expected = expectedCases();
  for (const row of report.observations) {
    keys(row, ["id", "family", "variables", "used_variables", "seed", "effective_seed", "density", "repetitions", "target_sign", "input", "reference_sat", "reference_work", "baseline", "generic", "indexed", "generic_indexed_equivalence"], "observation");
    const fixed = expected.get(row.id);
    require(fixed !== undefined, "Unexpected or duplicate held-out case identity.");
    expected.delete(row.id);
    equal({ family: row.family, variables: row.variables, seed: row.seed, effective_seed: row.effective_seed, density: row.density, repetitions: row.repetitions, target_sign: row.target_sign },
      fixed.metadata, "Case metadata differs from the fixed corpus.");
    require(cnf(row.input, row.variables) && row.input.length === fixed.clauses && row.input.every(clause => fixed.widths.includes(clause.length)), "Invalid bounded held-out CNF.");
    if (fixed.input) equal(row.input, fixed.input, "Engineered control input differs from the protocol construction.");
    require(row.used_variables === new Set(row.input.flat().map(Math.abs)).size, "Used-variable count differs from the CNF.");
    require(typeof row.reference_sat === "boolean", "Reference label must be Boolean.");
    work(row.reference_work);
    require(row.reference_work.assignments === 2 ** row.variables && row.reference_work.formula_checks === 2 ** row.variables, "Incomplete reference coverage counters.");
    const context = { referenceSat: row.reference_sat, variables: row.variables };
    solverArm(row.baseline, { ...context, baseline: true });
    solverArm(row.generic, context);
    keys(row.indexed, ["arm", "index"], "indexed arm");
    solverArm(row.indexed.arm, context);
    validateIndex(row.indexed.index, row.indexed.arm, row.input);
    const label = equivalence(row.generic, row.indexed.arm);
    require(row.generic_indexed_equivalence === label, "Equivalence label disagrees with the arm outcomes.");
    if (label !== "not-comparable-budget") equal(row.indexed.arm.derived_units, row.generic.derived_units, "Generic and indexed derived units differ where the protocol requires equality.");
    if (label === "complete-arms-exact") {
      require(row.generic.outcome === row.indexed.arm.outcome, "Completed generic and indexed decisions differ.");
      equal(row.indexed.arm.residual, row.generic.residual, "Completed generic and indexed residual counters differ.");
    }
  }
  validateSummaryShape(report.summary);
  equal(report.summary, summarize(report.observations), "Summary differs from per-case observations.");
  const families = [...new Set(report.observations.map(row => row.family))].sort();
  keys(report.families, families, "family summaries");
  for (const family of families) {
    validateSummaryShape(report.families[family]);
    equal(report.families[family], summarize(report.observations.filter(row => row.family === family)), "Family summary differs from observations.");
  }
  keys(report.setup_plus_online, ["generic_work_units", "indexed_work_units"], "setup-plus-online");
  require(report.setup_plus_online.generic_work_units === sum([report.setup.acquisition.work_units, report.summary.arms.generic.total_work_units]) &&
    report.setup_plus_online.indexed_work_units === sum([report.setup.acquisition.work_units, report.setup.compilation.work_units, report.summary.arms.indexed.total_work_units]),
  "Setup-plus-online totals are inconsistent.");
  return report;
}
