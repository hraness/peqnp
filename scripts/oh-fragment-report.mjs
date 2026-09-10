import {
  BUDGET, COMPARISON_FIELDS, comparison, count, distinctLiterals, equal, keys, literal, require, sum, validateLimitations, work,
} from "./oh-report-checks.mjs";

export const FRAGMENT_PROTOCOL_SHA256 = "bce209aa982a9964cf235758e43c7b6f405d7735a3c52c11cc867fbe9e60ec36";
const SEEDS = [3001, 6007, 12007, 24001];
const VARIABLES = [8, 10, 12];
const DENSITIES = [3, 4, 5];
const CONTROL_DENSITIES = [3, 5];
const CHAIN_SEED = 3001;
const CONTRADICTORY_SEED = 6007;
const CYCLE_LENGTHS = { 8: 3, 10: 5, 12: 7 };
const MAX_VARIABLES = 12;
const MAX_WIDTH = 3;
const U64_MAX = (1n << 64n) - 1n;
const CORPUS = {
  cases: 162,
  families: { "random-ternary-only": 36, "random-mixed": 108, "chain-embedded": 12, "contradictory-fragment": 6 },
  seeds: SEEDS, variables: VARIABLES, ternary_densities: DENSITIES, binary_counts: ["0", "n/2", "n", "2n"], control_densities: CONTROL_DENSITIES,
  chain_seed: CHAIN_SEED, chain_length: "n - 1", contradictory_seed: CONTRADICTORY_SEED, cycle_lengths: { 8: 3, 10: 5, 12: 7 },
  generator: "xorshift64", effective_seed: "base_seed ^ (n << 32) ^ (t << 40) ^ (b << 48)",
  max_variables: MAX_VARIABLES, max_width: MAX_WIDTH, exact_earlier_overlap: 0,
};
const COUNTER_MODEL = "transfer-v1-events-with-graph-phases-and-unit-appends";
const COMPARISON = {
  left: "fragment interface: graph construction, components, fragment certificate, extraction, copy and unit appends, then the common DPLL",
  right: "baseline DPLL on the original formula",
  restricted_to: "cases where both arms completed; every completed decision agreed with the reference",
};
const ENCODING = {
  vertex_labels: "usize 2*(v-1) and 2*(v-1)+1, at most 24 vertices",
  clause_indices: "usize positions in the original input, ternary clauses included",
  literals: "i32 signed DIMACS-style",
  counters: "u64 checked increments; identical in debug and release",
  enumeration_ratio: "reference work units divided by arm work units, nearest thousandth",
};
const FRAGMENT_PHASES = ["construction", "components", "certificate", "extraction", "append"];
const PHASES = [...FRAGMENT_PHASES, "residual"];
const UNSAT_CERTIFICATES = ["empty-clause", "opposite-paths"];
const BASELINE_SUMMARY_FIELDS = ["work_units", "search_nodes", "complete", "sat", "unsat", "unknown"];
const PHASE_SUMMARY_FIELDS = FRAGMENT_PHASES.map(phase => phase + "_work_units");
const FRAGMENT_SUMMARY_FIELDS = ["phases", "fragment_work_units", "residual_work_units", "total_work_units", "search_nodes", "derived_units", "fragment_unsat", "complete", "sat", "unsat", "unknown"];
const RATIO_FIELDS = ["reference_over_baseline", "reference_over_fragment"];
const EMPTY_METADATA = { seed: null, effective_seed: null, density: null, binary_count: null, binary_label: null, chain_length: null, sign: null, cycle_length: null };

const signName = sign => sign > 0 ? "pos" : "neg";
// The Rust generator's u64 seed fields occupy disjoint bits; the report
// emits the effective seed as a decimal string because it exceeds 2^53.
const effectiveSeed = (base, n, t, b) => (BigInt(base) ^ (BigInt(n) << 32n) ^ (BigInt(t) << 40n) ^ (BigInt(b) << 48n)).toString();
function chain(k, sign) {
  const y = i => i + 1;
  const clauses = [[1, y(1)]];
  for (let i = 1; i < k; i++) clauses.push([-y(i), y(i + 1)]);
  clauses.push([-y(k), 1]);
  return clauses.map(clause => clause.map(lit => Math.abs(lit) === 1 ? lit * sign : lit));
}
function parityCycle(m) {
  const clauses = [];
  for (let i = 1; i <= m; i++) clauses.push([i, (i % m) + 1], [-i, -((i % m) + 1)]);
  return clauses;
}

// The fixed 162-case corpus in protocol enumeration order. Engineered
// prefixes are regenerated exactly; the pseudorandom ternary and binary
// suffixes are identified by their parameters and effective seed and checked
// for count, width, and distinct variables only.
function expectedCases() {
  const cases = new Map();
  const add = (id, family, variables, metadata, prefix, ternary, binary) =>
    cases.set(id, { metadata: { family, variables, ...EMPTY_METADATA, ...metadata }, prefix, ternary, binary });
  for (const seed of SEEDS) for (const n of VARIABLES) for (const t of DENSITIES) {
    for (const [label, b] of [["b=0", 0], ["b=n/2", n / 2], ["b=n", n], ["b=2n", 2 * n]]) {
      add(`random-n${n}-t${t}-b${b}-s${seed}`, b === 0 ? "random-ternary-only" : "random-mixed", n,
        { seed, effective_seed: effectiveSeed(seed, n, t, b), density: t, binary_count: b, binary_label: label }, [], t * n, b);
    }
  }
  for (const n of VARIABLES) for (const t of CONTROL_DENSITIES) for (const sign of [1, -1]) {
    const k = n - 1;
    add(`chain-n${n}-t${t}-${signName(sign)}`, "chain-embedded", n,
      { seed: CHAIN_SEED, effective_seed: effectiveSeed(CHAIN_SEED, n, t, n), density: t, binary_count: n, chain_length: k, sign }, chain(k, sign), t * n, 0);
  }
  for (const n of VARIABLES) {
    const m = CYCLE_LENGTHS[n];
    for (const t of CONTROL_DENSITIES) {
      add(`contradictory-n${n}-m${m}-t${t}`, "contradictory-fragment", n,
        { seed: CONTRADICTORY_SEED, effective_seed: effectiveSeed(CONTRADICTORY_SEED, n, t, 2 * m), density: t, binary_count: 2 * m, cycle_length: m }, parityCycle(m), t * n, 0);
    }
  }
  require(cases.size === CORPUS.cases, "Corpus enumeration must yield exactly 162 cases.");
  return cases;
}

function boundedCnf(value, variables, clauses) {
  return Array.isArray(value) && value.length === clauses && value.every(clause => Array.isArray(clause) &&
    clause.length <= MAX_WIDTH && clause.every(lit => literal(lit, variables)));
}
const drawn = (clause, width) => clause.length === width && new Set(clause.map(Math.abs)).size === width;

// Raw-clause path checking on the width-at-most-three formula: only a unit
// (x) supporting not x -> x, or a binary clause (x or y) supporting
// not x -> y and not y -> x, can witness an edge. Ternary clauses never do.
function validatePath(path, input, from, to, variables) {
  keys(path, ["nodes", "clauses"], "implication path");
  require(Array.isArray(path.nodes) && Array.isArray(path.clauses) && path.nodes.length === path.clauses.length + 1 &&
    path.nodes.every(node => literal(node, variables)) && new Set(path.nodes).size === path.nodes.length, "Implication path must be a simple node sequence with one clause per edge.");
  require(path.nodes[0] === from && path.nodes.at(-1) === to, "Implication path endpoints differ from the claimed implication.");
  path.clauses.forEach((index, position) => {
    require(Number.isSafeInteger(index) && index >= 0 && index < input.length, "Implication path cites a clause outside the input.");
    const clause = input[index];
    const [a, b] = [path.nodes[position], path.nodes[position + 1]];
    const supported = clause.length === 1 ? a === -clause[0] && b === clause[0]
      : clause.length === 2 ? (a === -clause[0] && b === clause[1]) || (a === -clause[1] && b === clause[0]) : false;
    require(supported, "Cited clause does not support its implication-path edge.");
  });
}

function validateCertificate(certificate, input, variables) {
  require(certificate && typeof certificate === "object" && !Array.isArray(certificate), "Completed fragment arm requires a certificate.");
  if (certificate.kind === "sat") {
    keys(certificate, ["kind", "values"], "SAT certificate");
    require(Array.isArray(certificate.values) && certificate.values.length === variables &&
      certificate.values.every(value => typeof value === "boolean"), "SAT certificate must assign one Boolean per declared variable.");
    require(input.every(clause => clause.length > 2 || clause.some(lit => certificate.values[Math.abs(lit) - 1] === (lit > 0))), "Fragment SAT certificate does not satisfy every width-at-most-two clause.");
    return;
  }
  if (certificate.kind === "empty-clause") {
    keys(certificate, ["kind", "clause"], "empty-clause certificate");
    require(Number.isSafeInteger(certificate.clause) && certificate.clause >= 0 && certificate.clause < input.length && input[certificate.clause].length === 0, "Empty-clause certificate must cite an empty raw clause.");
    return;
  }
  keys(certificate, ["kind", "variable", "positive_to_negative", "negative_to_positive"], "opposite-paths certificate");
  require(certificate.kind === "opposite-paths" && Number.isInteger(certificate.variable) && certificate.variable >= 1 && certificate.variable <= variables, "Invalid fragment certificate kind or variable.");
  validatePath(certificate.positive_to_negative, input, certificate.variable, -certificate.variable, variables);
  validatePath(certificate.negative_to_positive, input, -certificate.variable, certificate.variable, variables);
}

const idle = arm => ["extraction", "append", "residual"].every(phase => arm.phases[phase].work_units === 0);

function validateFragment(arm, row) {
  keys(arm, ["outcome", "certificate", "derived_units", "clues", "phases", "fragment_work_units", "residual_work_units", "search_nodes", "total_work_units"], "fragment arm");
  keys(arm.phases, PHASES, "fragment phases");
  for (const phase of PHASES) work(arm.phases[phase]);
  require(arm.fragment_work_units === sum(FRAGMENT_PHASES.map(phase => arm.phases[phase].work_units)) &&
    arm.residual_work_units === arm.phases.residual.work_units && arm.search_nodes === arm.phases.residual.search_nodes &&
    arm.total_work_units === sum([arm.fragment_work_units, arm.residual_work_units]) && arm.total_work_units <= BUDGET, "Fragment phase totals are inconsistent or exceed the shared budget.");
  require(["sat", "unsat", "unknown-budget"].includes(arm.outcome), "Invalid fragment outcome.");
  require(Array.isArray(arm.clues) && distinctLiterals(arm.derived_units, row.variables), "Clues must be a list and derived units distinct declared literals.");
  const reference = row.reference;
  if (arm.outcome === "unknown-budget") {
    require(arm.total_work_units === BUDGET, "Unknown-budget requires exhausted work budget.");
    require(arm.certificate === null && arm.clues.length === 0 && arm.derived_units.length === 0, "Unknown fragment arm cannot carry a certificate, clues, or units.");
    return;
  }
  require((arm.outcome === "sat") === reference.sat, "Fragment decision disagrees with the supplied reference label.");
  validateCertificate(arm.certificate, row.input, row.variables);
  if (arm.certificate.kind !== "sat") {
    require(arm.outcome === "unsat" && arm.clues.length === 0 && arm.derived_units.length === 0 && idle(arm), "Fragment UNSAT certificate must decide the case without extraction, appends, or residual work.");
    return;
  }
  require(arm.phases.residual.work_units > 0, "Fragment SAT certificate must be followed by the residual decision.");
  for (const clue of arm.clues) {
    keys(clue, ["literal", "path"], "clue");
    validatePath(clue.path, row.input, -clue.literal, clue.literal, row.variables);
  }
  equal(arm.derived_units, arm.clues.map(clue => clue.literal), "Derived units differ from the clue literals in order.");
  if (reference.sat) require(arm.derived_units.every(unit => reference.backbone.includes(unit)), "Derived units must be a subset of the reference backbone.");
}

function validateObservation(row, fixed) {
  keys(row, ["id", "family", "variables", "used_variables", "clauses", "fragment_clauses", "seed", "effective_seed", "density", "binary_count", "binary_label", "chain_length", "sign", "cycle_length", "input", "reference", "baseline", "fragment", "checks"], "observation");
  equal({ family: row.family, variables: row.variables, seed: row.seed, effective_seed: row.effective_seed, density: row.density, binary_count: row.binary_count, binary_label: row.binary_label, chain_length: row.chain_length, sign: row.sign, cycle_length: row.cycle_length },
    fixed.metadata, "Case metadata differs from the fixed corpus.");
  const clauses = fixed.prefix.length + fixed.ternary + fixed.binary;
  require(boundedCnf(row.input, row.variables, clauses), "Invalid bounded width-at-most-three CNF.");
  equal(row.input.slice(0, fixed.prefix.length), fixed.prefix, "Engineered control input differs from the protocol construction.");
  const random = row.input.slice(fixed.prefix.length);
  require(random.every((clause, index) => drawn(clause, index < fixed.ternary ? 3 : 2)), "Random clause shape differs from its parameters.");
  require(row.clauses === row.input.length && row.fragment_clauses === row.input.filter(clause => clause.length <= 2).length &&
    row.used_variables === new Set(row.input.flat().map(Math.abs)).size, "Clause or used-variable counts differ from the CNF.");
  const reference = row.reference;
  keys(reference, ["model_count", "sat", "backbone", "work"], "reference");
  count(reference.model_count);
  require(reference.model_count <= 2 ** row.variables && reference.sat === (reference.model_count > 0), "Reference model count or label is inconsistent.");
  work(reference.work);
  require(reference.work.assignments === 2 ** row.variables && reference.work.formula_checks >= 2 ** row.variables, "Incomplete reference coverage counters.");
  if (reference.sat) {
    require(distinctLiterals(reference.backbone, row.variables) && new Set(reference.backbone.map(Math.abs)).size === reference.backbone.length, "Reference backbone must list distinct forced literals.");
  } else require(reference.backbone === null, "UNSAT reference cannot define a backbone.");
  const baseline = row.baseline;
  keys(baseline, ["outcome", "work", "total_work_units", "search_nodes"], "baseline arm");
  work(baseline.work);
  require(baseline.total_work_units === baseline.work.work_units && baseline.search_nodes === baseline.work.search_nodes && baseline.total_work_units <= BUDGET, "Baseline totals are inconsistent or exceed the shared budget.");
  require(["sat", "unsat", "unknown-budget"].includes(baseline.outcome), "Invalid baseline outcome.");
  if (baseline.outcome === "unknown-budget") require(baseline.total_work_units === BUDGET, "Unknown-budget requires exhausted work budget.");
  else require((baseline.outcome === "sat") === reference.sat, "Baseline decision disagrees with the supplied reference label.");
  const fragment = row.fragment;
  validateFragment(fragment, row);
  const completed = fragment.outcome !== "unknown-budget";
  if (row.family === "chain-embedded" && completed) {
    require(fragment.certificate.kind === "sat", "Chain-embedded control must report a satisfiable fragment.");
    equal(fragment.derived_units, [row.sign], "Chain-embedded control must derive exactly the selected sign of x.");
  }
  if (row.family === "contradictory-fragment") {
    require(!reference.sat, "Contradictory-fragment control labeled satisfiable.");
    require(fragment.outcome === "unsat" && UNSAT_CERTIFICATES.includes(fragment.certificate.kind) && idle(fragment), "Contradictory-fragment control must be decided UNSAT by the fragment alone.");
  }
  if (row.family === "random-ternary-only" && completed) {
    require(fragment.certificate.kind === "sat" && fragment.derived_units.length === 0, "An empty fragment cannot derive units or refute the formula.");
    require(baseline.outcome !== "unknown-budget", "Completed residual on an empty fragment implies a completed baseline.");
    equal(fragment.phases.residual, baseline.work, "Residual counters on an empty fragment differ from the baseline's work.");
  }
  keys(row.checks, ["certificate_valid", "clues_checked", "clues_valid", "work"], "checks");
  work(row.checks.work);
  require(row.checks.certificate_valid === (fragment.certificate === null ? null : true) &&
    row.checks.clues_checked === fragment.clues.length && row.checks.clues_valid === fragment.clues.length, "Checker outcome disagrees with the reported certificate and clues.");
}

// Descriptive ratio as the Rust emits it: nearest thousandth by u64
// integer arithmetic, null on a zero denominator or u64 overflow.
function ratio(numerator, denominator) {
  if (denominator === 0) return null;
  const scaled = BigInt(numerator) * 1000n;
  if (scaled > U64_MAX) return null;
  const rounded = scaled + BigInt(denominator) / 2n;
  if (rounded > U64_MAX) return null;
  return Number(rounded / BigInt(denominator)) / 1000;
}

function summarize(rows) {
  const tally = (fields, entries) => {
    const result = Object.fromEntries(fields.map(key => [key, 0]));
    for (const [key, value] of entries) result[key] = sum([result[key], value]);
    return result;
  };
  const outcome = arm => arm.outcome === "sat" ? "sat" : arm.outcome === "unsat" ? "unsat" : "unknown";
  const baseline = tally(BASELINE_SUMMARY_FIELDS, rows.flatMap(row => [["work_units", row.baseline.total_work_units], ["search_nodes", row.baseline.search_nodes],
    [outcome(row.baseline), 1], ["complete", Number(row.baseline.outcome !== "unknown-budget")]]));
  const fragment = {
    phases: tally(PHASE_SUMMARY_FIELDS, rows.flatMap(row => FRAGMENT_PHASES.map(phase => [phase + "_work_units", row.fragment.phases[phase].work_units]))),
    ...tally(FRAGMENT_SUMMARY_FIELDS.slice(1), rows.flatMap(row => [
      ["fragment_work_units", row.fragment.fragment_work_units], ["residual_work_units", row.fragment.residual_work_units],
      ["total_work_units", row.fragment.total_work_units], ["search_nodes", row.fragment.search_nodes],
      ["derived_units", row.fragment.derived_units.length],
      ["fragment_unsat", Number(row.fragment.certificate !== null && UNSAT_CERTIFICATES.includes(row.fragment.certificate.kind))],
      ["complete", Number(row.fragment.outcome !== "unknown-budget")], [outcome(row.fragment), 1],
    ])),
  };
  const referenceWorkUnits = sum(rows.map(row => row.reference.work.work_units));
  return {
    cases: rows.length,
    reference_sat: rows.filter(row => row.reference.sat).length,
    reference_unsat: rows.filter(row => !row.reference.sat).length,
    reference_work_units: referenceWorkUnits,
    checker_work_units: sum(rows.map(row => row.checks.work.work_units)),
    certificates_checked: rows.filter(row => row.checks.certificate_valid !== null).length,
    clues_checked: sum(rows.map(row => row.checks.clues_checked)),
    arms: { baseline, fragment },
    comparisons: {
      fragment_vs_baseline: comparison(rows.map(row => [
        row.fragment.outcome === "unknown-budget" ? null : row.fragment.total_work_units,
        row.baseline.outcome === "unknown-budget" ? null : row.baseline.total_work_units,
      ])),
    },
    enumeration_ratio: {
      reference_over_baseline: ratio(referenceWorkUnits, baseline.work_units),
      reference_over_fragment: ratio(referenceWorkUnits, fragment.total_work_units),
    },
  };
}

function validateSummaryShape(summary) {
  keys(summary, ["cases", "reference_sat", "reference_unsat", "reference_work_units", "checker_work_units", "certificates_checked", "clues_checked", "arms", "comparisons", "enumeration_ratio"], "summary");
  keys(summary.arms, ["baseline", "fragment"], "summary arms");
  keys(summary.arms.baseline, BASELINE_SUMMARY_FIELDS, "baseline summary");
  keys(summary.arms.fragment, FRAGMENT_SUMMARY_FIELDS, "fragment summary");
  keys(summary.arms.fragment.phases, PHASE_SUMMARY_FIELDS, "fragment phase summary");
  keys(summary.comparisons, ["fragment_vs_baseline"], "summary comparisons");
  keys(summary.comparisons.fragment_vs_baseline, COMPARISON_FIELDS, "comparison");
  keys(summary.enumeration_ratio, RATIO_FIELDS, "enumeration ratio");
}

function validateGroups(groups, rows, label, name) {
  const labels = [...new Set(rows.map(label))].sort();
  keys(groups, labels, name);
  for (const value of labels) {
    validateSummaryShape(groups[value]);
    equal(groups[value], summarize(rows.filter(row => label(row) === value)), name.replace(/ summaries$/, "") + " summary differs from observations.");
  }
  return labels;
}

export function validateFragmentInterface(report) {
  keys(report, ["schema_version", "experiment", "status", "proof_status", "protocol_sha256", "arm_budget", "counter_model", "limitations", "corpus", "observations", "summary", "outside_arms", "comparison", "encoding", "families", "random_mixed_by_binary_count", "random_by_variables", "random_by_density"], "fragment-interface report");
  require(report.schema_version === 1 && report.experiment === "fragment-interface-v1" && report.status === "bounded-tested" &&
    report.proof_status === "no-formal-proof" && report.protocol_sha256 === FRAGMENT_PROTOCOL_SHA256,
  "Expected the fixed bounded fragment-interface-v1 protocol without a formal-proof claim.");
  require(report.arm_budget === BUDGET, "Unexpected shared arm budget.");
  require(report.counter_model === COUNTER_MODEL, "Unexpected counter model.");
  validateLimitations(report.limitations);
  equal(report.corpus, CORPUS, "Unexpected fragment-interface corpus.");
  equal(report.comparison, COMPARISON, "Unexpected comparison declaration.");
  equal(report.encoding, ENCODING, "Unexpected encoding declaration.");
  require(Array.isArray(report.observations) && report.observations.length === CORPUS.cases, "Expected every fixed fragment-interface case.");
  const expected = expectedCases();
  for (const row of report.observations) {
    require(row && typeof row === "object" && expected.has(row.id), "Unexpected or duplicate fragment-interface case identity.");
    const fixed = expected.get(row.id);
    expected.delete(row.id);
    validateObservation(row, fixed);
  }
  const identities = report.observations.map(row => JSON.stringify([row.variables, row.input]));
  require(new Set(identities).size === identities.length, "Corpus repeats an exact declared-variables and ordered-CNF pair.");
  validateSummaryShape(report.summary);
  equal(report.summary, summarize(report.observations), "Summary differs from per-case observations.");
  const families = validateGroups(report.families, report.observations, row => row.family, "family summaries");
  equal(Object.fromEntries(families.map(family => [family, report.families[family].cases])), CORPUS.families, "Family sizes differ from the fixed corpus.");
  const random = report.observations.filter(row => row.family.startsWith("random-"));
  validateGroups(report.random_mixed_by_binary_count, random.filter(row => row.family === "random-mixed"), row => row.binary_label, "binary-count summaries");
  validateGroups(report.random_by_variables, random, row => "n=" + row.variables, "random-variables summaries");
  validateGroups(report.random_by_density, random, row => "t=" + row.density, "random-density summaries");
  equal(report.outside_arms, {
    reference_work_units: report.summary.reference_work_units,
    checker_work_units: report.summary.checker_work_units,
  }, "Outside-arm totals are inconsistent.");
  return report;
}
