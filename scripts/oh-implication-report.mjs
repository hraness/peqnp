import {
  BUDGET, COMPARISON_FIELDS, comparison, count, distinctLiterals, equal, keys, literal, require, sum, validateAcquisition,
  validateFrozenRules, validateLimitations, work,
} from "./oh-report-checks.mjs";

export const IMPLICATION_PROTOCOL_SHA256 = "5b957bb778acd98fa22cd40e8c4bd0f46c87ec516bea42b68736c70f44352ec2";
const CHAIN_LENGTHS = [2, 3, 5, 7, 9, 11];
const IMMEDIATE_SIZES = [2, 4, 6, 8, 10, 12];
const PARITY_SIZES = [3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
const SEEDS = [2027, 8191, 131071, 999983];
const RANDOM_VARIABLES = [6, 8, 10, 12];
const DENSITIES = [1, 2, 4];
const MAX_VARIABLES = 12;
const CORPUS = {
  cases: 120,
  families: { "long-explanation": 24, "broken-explanation": 24, "immediate-pattern": 12, "parity-cycle": 10, "empty-control": 2, "random-2cnf": 48 },
  chain_lengths: CHAIN_LENGTHS, immediate_sizes: IMMEDIATE_SIZES, parity_sizes: PARITY_SIZES, seeds: SEEDS,
  random_variables: RANDOM_VARIABLES, densities: DENSITIES, generator: "xorshift64",
  effective_seed: "base_seed ^ (n << 32) ^ (d << 16)", max_variables: MAX_VARIABLES,
};
const DECISION_COMPARISON = {
  left: "graph decision including construction, SCC and explicit certificate output",
  right: "DPLL Boolean decision without certificate",
  restricted_to: "cases where both decisions completed; every completed decision agreed with the reference",
};
const ENCODING = {
  vertex_labels: "usize 2*(v-1) and 2*(v-1)+1, at most 24 vertices",
  clause_indices: "usize positions in the retained input",
  literals: "i32 signed DIMACS-style",
  counters: "u64 checked increments; identical in debug and release",
};
const PHASES = ["construction", "scc", "decision_certificate", "backbone"];
const LOCAL_SUMMARY_FIELDS = ["work_units", "complete", "unknown", "derived_units"];
const DPLL_SUMMARY_FIELDS = ["work_units", "search_nodes", "complete", "sat", "unsat", "unknown"];
const GRAPH_SUMMARY_FIELDS = ["construction_work_units", "scc_work_units", "decision_certificate_work_units", "decision_work_units", "decision_search_nodes", "backbone_work_units", "backbone_search_nodes", "total_work_units", "decision_complete", "sat", "unsat", "decision_unknown", "backbone_complete", "backbone_not_defined_unsat", "backbone_unknown", "clues"];
const COVERAGE_FIELDS = ["cases", "reference_literals", "found_literals", "missed_literals", "full_coverage_cases"];
const EMPTY_METADATA = { seed: null, effective_seed: null, density: null, chain_length: null, sign: null, order: null };

const signName = sign => sign > 0 ? "pos" : "neg";
function chain(k, broken) {
  const y = i => i + 1;
  const clauses = [[1, y(1)]];
  for (let i = 1; i < k; i++) if (!(broken && i === Math.floor(k / 2))) clauses.push([-y(i), y(i + 1)]);
  clauses.push([-y(k), 1]);
  return clauses;
}
function transformed(clauses, sign, order) {
  const result = clauses.map(clause => clause.map(lit => Math.abs(lit) === 1 ? lit * sign : lit));
  return order === "reversed" ? result.reverse() : result;
}

// The fixed 120-case corpus. Engineered families are regenerated exactly with
// their stated control expectations; random cases are identified by their
// parameters and effective seed and checked for shape only.
function expectedCases() {
  const cases = new Map();
  const add = (id, family, variables, metadata, input, expectation) =>
    cases.set(id, { metadata: { family, variables, ...EMPTY_METADATA, ...metadata }, input, expectation });
  for (const [family, broken] of [["long-explanation", false], ["broken-explanation", true]]) for (const k of CHAIN_LENGTHS) {
    for (const sign of [1, -1]) for (const order of ["forward", "reversed"]) {
      add(`${family}-k${k}-${signName(sign)}-${order}`, family, k + 1, { chain_length: k, sign, order },
        transformed(chain(k, broken), sign, order), { backbone: broken ? [] : [sign] });
    }
  }
  for (const n of IMMEDIATE_SIZES) for (const sign of [1, -1]) {
    const input = [];
    const backbone = [];
    for (let i = 1; i <= n / 2; i++) {
      input.push([sign * (2 * i - 1), 2 * i], [sign * (2 * i - 1), -(2 * i)]);
      backbone.push(sign * (2 * i - 1));
    }
    add(`immediate-pattern-n${n}-${signName(sign)}`, "immediate-pattern", n, { sign }, input, { backbone });
  }
  for (const n of PARITY_SIZES) {
    const input = [];
    for (let i = 1; i <= n; i++) input.push([i, (i % n) + 1], [-i, -((i % n) + 1)]);
    add(`parity-cycle-n${n}`, "parity-cycle", n, {}, input, n % 2 === 0 ? { backbone: [] } : { unsat: true });
  }
  add("empty-formula", "empty-control", 0, {}, [], { backbone: [] });
  add("empty-clause", "empty-control", 0, {}, [[]], { unsat: true });
  for (const seed of SEEDS) for (const n of RANDOM_VARIABLES) for (const d of DENSITIES) {
    const effective = Number(BigInt(seed) ^ (BigInt(n) << 32n) ^ (BigInt(d) << 16n));
    add(`random-n${n}-d${d}-s${seed}`, "random-2cnf", n, { seed, effective_seed: effective, density: d }, null, { clauses: n * d });
  }
  return cases;
}

function binaryCnf(value, variables) {
  return Array.isArray(value) && value.length <= MAX_VARIABLES * 4 && value.every(clause => Array.isArray(clause) &&
    clause.length <= 2 && clause.every(lit => literal(lit, variables)));
}

// Raw-clause path checking: clause (a or b) supports the edge u -> w exactly
// when it contains both not u and w; a unit (a) supports not a -> a.
function validatePath(path, input, from, to, variables) {
  keys(path, ["nodes", "clauses"], "implication path");
  require(Array.isArray(path.nodes) && Array.isArray(path.clauses) && path.nodes.length === path.clauses.length + 1 &&
    path.nodes.every(node => literal(node, variables)) && new Set(path.nodes).size === path.nodes.length, "Implication path must be a simple node sequence with one clause per edge.");
  require(path.nodes[0] === from && path.nodes.at(-1) === to, "Implication path endpoints differ from the claimed implication.");
  path.clauses.forEach((index, position) => {
    require(Number.isSafeInteger(index) && index >= 0 && index < input.length, "Implication path cites a clause outside the input.");
    require(input[index].includes(-path.nodes[position]) && input[index].includes(path.nodes[position + 1]), "Cited clause does not support its implication-path edge.");
  });
}

function validateCertificate(certificate, decision, input, variables) {
  if (decision === "sat") {
    keys(certificate, ["kind", "values"], "SAT certificate");
    require(certificate.kind === "sat" && Array.isArray(certificate.values) && certificate.values.length === variables &&
      certificate.values.every(value => typeof value === "boolean"), "SAT certificate must assign one Boolean per declared variable.");
    require(input.every(clause => clause.some(lit => certificate.values[Math.abs(lit) - 1] === (lit > 0))), "SAT certificate does not satisfy every raw clause.");
    return;
  }
  require(certificate && typeof certificate === "object" && !Array.isArray(certificate), "UNSAT decision requires a certificate.");
  if (certificate.kind === "empty-clause") {
    keys(certificate, ["kind", "clause"], "empty-clause certificate");
    require(Number.isSafeInteger(certificate.clause) && certificate.clause >= 0 && certificate.clause < input.length && input[certificate.clause].length === 0, "Empty-clause certificate must cite an empty raw clause.");
    return;
  }
  keys(certificate, ["kind", "variable", "positive_to_negative", "negative_to_positive"], "opposite-paths certificate");
  require(certificate.kind === "opposite-paths" && Number.isInteger(certificate.variable) && certificate.variable >= 1 && certificate.variable <= variables, "Invalid UNSAT certificate kind or variable.");
  validatePath(certificate.positive_to_negative, input, certificate.variable, -certificate.variable, variables);
  validatePath(certificate.negative_to_positive, input, -certificate.variable, certificate.variable, variables);
}

function validateGraph(graph, row) {
  keys(graph, ["decision", "certificate", "backbone_status", "clues", "phases", "decision_work_units", "backbone_work_units", "total_work_units"], "graph arm");
  keys(graph.phases, PHASES, "graph phases");
  for (const phase of PHASES) work(graph.phases[phase]);
  const decision = sum(["construction", "scc", "decision_certificate"].map(phase => graph.phases[phase].work_units));
  require(graph.decision_work_units === decision && graph.backbone_work_units === graph.phases.backbone.work_units &&
    graph.total_work_units === sum([decision, graph.backbone_work_units]) && graph.total_work_units <= BUDGET, "Graph phase totals are inconsistent or exceed the shared budget.");
  require(["sat", "unsat", "unknown-budget"].includes(graph.decision), "Invalid graph decision.");
  require(Array.isArray(graph.clues), "Clues must be a list.");
  const label = row.reference.model_count > 0;
  if (graph.decision === "unknown-budget") {
    require(graph.total_work_units === BUDGET, "Unknown-budget requires exhausted work budget.");
    require(graph.certificate === null && graph.backbone_status === "unknown-budget" && graph.clues.length === 0, "Unknown decision cannot carry a certificate, backbone status, or clues.");
    return;
  }
  require((graph.decision === "sat") === label, "Graph decision disagrees with the supplied reference label.");
  validateCertificate(graph.certificate, graph.decision, row.input, row.variables);
  if (graph.decision === "unsat") {
    require(graph.backbone_status === "not-defined-unsat" && graph.clues.length === 0 && graph.backbone_work_units === 0, "UNSAT rows must report no clues and an undefined backbone.");
    return;
  }
  require(["complete", "unknown-budget"].includes(graph.backbone_status), "SAT rows must report a complete or unknown backbone status.");
  if (graph.backbone_status === "unknown-budget") {
    require(graph.total_work_units === BUDGET && graph.clues.length === 0, "Unknown backbone requires an exhausted budget and no partial clue set.");
    return;
  }
  const literals = graph.clues.map(clue => clue.literal);
  require(distinctLiterals(literals, row.variables), "Clue literals must be distinct declared literals.");
  for (const clue of graph.clues) {
    keys(clue, ["literal", "path"], "clue");
    validatePath(clue.path, row.input, -clue.literal, clue.literal, row.variables);
  }
  equal(literals, row.reference.backbone, "Complete graph backbone differs from the reference backbone.");
}

function validateObservation(row, fixed) {
  keys(row, ["id", "family", "variables", "used_variables", "seed", "effective_seed", "density", "chain_length", "sign", "order", "input", "reference", "local", "dpll", "graph", "checks"], "observation");
  equal({ family: row.family, variables: row.variables, seed: row.seed, effective_seed: row.effective_seed, density: row.density, chain_length: row.chain_length, sign: row.sign, order: row.order },
    fixed.metadata, "Case metadata differs from the fixed corpus.");
  require(binaryCnf(row.input, row.variables), "Invalid bounded binary CNF.");
  if (fixed.input) equal(row.input, fixed.input, "Engineered control input differs from the protocol construction.");
  else require(row.input.length === fixed.expectation.clauses && row.input.every(clause => clause.length === 2 && Math.abs(clause[0]) !== Math.abs(clause[1])), "Random case shape differs from its parameters.");
  require(row.used_variables === new Set(row.input.flat().map(Math.abs)).size, "Used-variable count differs from the CNF.");
  const reference = row.reference;
  keys(reference, ["model_count", "sat", "backbone", "work"], "reference");
  count(reference.model_count);
  require(reference.model_count <= 2 ** row.variables && reference.sat === (reference.model_count > 0), "Reference model count or label is inconsistent.");
  work(reference.work);
  require(reference.work.assignments === 2 ** row.variables && reference.work.formula_checks >= 2 ** row.variables, "Incomplete reference coverage counters.");
  if (reference.sat) {
    require(distinctLiterals(reference.backbone, row.variables) && new Set(reference.backbone.map(Math.abs)).size === reference.backbone.length, "Reference backbone must list distinct forced literals.");
  } else require(reference.backbone === null, "UNSAT reference cannot define a backbone.");
  if (fixed.expectation.unsat) require(!reference.sat, "Engineered UNSAT control labeled satisfiable.");
  else if (fixed.expectation.backbone) equal(reference.backbone, fixed.expectation.backbone, "Engineered control backbone differs from its stated expectation.");
  keys(row.local, ["complete", "units", "work"], "local arm");
  work(row.local.work);
  require(typeof row.local.complete === "boolean" && row.local.work.work_units <= BUDGET && distinctLiterals(row.local.units, row.variables), "Invalid local arm.");
  if (!row.local.complete) require(row.local.work.work_units === BUDGET && row.local.units.length === 0, "Unknown local arm requires an exhausted budget and no units.");
  else if (reference.sat) {
    require(row.local.units.every(unit => reference.backbone.includes(unit)), "Local units must be a subset of the reference backbone.");
    if (row.family === "long-explanation") require(row.local.units.length === 0, "Two-clause subsets of a long explanation cannot entail a unit.");
    if (row.family === "immediate-pattern") equal([...row.local.units].sort((a, b) => a - b), [...reference.backbone].sort((a, b) => a - b), "Immediate-pattern control must validate library reuse.");
  }
  keys(row.dpll, ["outcome", "work", "total_work_units", "search_nodes"], "dpll arm");
  work(row.dpll.work);
  require(row.dpll.total_work_units === row.dpll.work.work_units && row.dpll.search_nodes === row.dpll.work.search_nodes && row.dpll.total_work_units <= BUDGET, "DPLL totals are inconsistent or exceed the shared budget.");
  require(["sat", "unsat", "unknown-budget"].includes(row.dpll.outcome), "Invalid DPLL outcome.");
  if (row.dpll.outcome === "unknown-budget") require(row.dpll.total_work_units === BUDGET, "Unknown-budget requires exhausted work budget.");
  else require((row.dpll.outcome === "sat") === reference.sat, "DPLL decision disagrees with the supplied reference label.");
  validateGraph(row.graph, row);
  keys(row.checks, ["certificate_valid", "clues_checked", "clues_valid", "work"], "checks");
  work(row.checks.work);
  require(row.checks.certificate_valid === (row.graph.certificate === null ? null : true) &&
    row.checks.clues_checked === row.graph.clues.length && row.checks.clues_valid === row.graph.clues.length, "Checker outcome disagrees with the reported certificate and clues.");
}

function coverage(pairs) {
  const result = Object.fromEntries(COVERAGE_FIELDS.map(key => [key, 0]));
  const add = (key, value) => { result[key] = sum([result[key], value]); };
  for (const [reference, found] of pairs) {
    require(found <= reference, "Coverage cannot exceed the reference backbone.");
    add("cases", 1);
    add("reference_literals", reference);
    add("found_literals", found);
    add("missed_literals", reference - found);
    if (reference === found) add("full_coverage_cases", 1);
  }
  return result;
}

function summarize(rows) {
  const tally = (fields, entries) => {
    const result = Object.fromEntries(fields.map(key => [key, 0]));
    for (const [key, value] of entries) result[key] = sum([result[key], value]);
    return result;
  };
  const local = tally(LOCAL_SUMMARY_FIELDS, rows.flatMap(row => [["work_units", row.local.work.work_units], [row.local.complete ? "complete" : "unknown", 1], ["derived_units", row.local.units.length]]));
  const dpll = tally(DPLL_SUMMARY_FIELDS, rows.flatMap(row => [["work_units", row.dpll.total_work_units], ["search_nodes", row.dpll.search_nodes],
    [row.dpll.outcome === "sat" ? "sat" : row.dpll.outcome === "unsat" ? "unsat" : "unknown", 1], ["complete", Number(row.dpll.outcome !== "unknown-budget")]]));
  const graph = tally(GRAPH_SUMMARY_FIELDS, rows.flatMap(row => {
    const arm = row.graph;
    return [
      ["construction_work_units", arm.phases.construction.work_units], ["scc_work_units", arm.phases.scc.work_units],
      ["decision_certificate_work_units", arm.phases.decision_certificate.work_units], ["decision_work_units", arm.decision_work_units],
      ["decision_search_nodes", sum(["construction", "scc", "decision_certificate"].map(phase => arm.phases[phase].search_nodes))],
      ["backbone_work_units", arm.backbone_work_units], ["backbone_search_nodes", arm.phases.backbone.search_nodes],
      ["total_work_units", arm.total_work_units], ["decision_complete", Number(arm.decision !== "unknown-budget")],
      [arm.decision === "sat" ? "sat" : arm.decision === "unsat" ? "unsat" : "decision_unknown", 1],
      [arm.backbone_status === "complete" ? "backbone_complete" : arm.backbone_status === "not-defined-unsat" ? "backbone_not_defined_unsat" : "backbone_unknown", 1],
      ["clues", arm.clues.length],
    ];
  }));
  const satRows = rows.filter(row => row.reference.sat);
  return {
    cases: rows.length,
    reference_sat: satRows.length,
    reference_unsat: rows.length - satRows.length,
    reference_work_units: sum(rows.map(row => row.reference.work.work_units)),
    checker_work_units: sum(rows.map(row => row.checks.work.work_units)),
    certificates_checked: rows.filter(row => row.checks.certificate_valid !== null).length,
    clues_checked: sum(rows.map(row => row.checks.clues_checked)),
    arms: { local, dpll, graph },
    coverage: {
      local: coverage(satRows.filter(row => row.local.complete).map(row => [row.reference.backbone.length, row.local.units.length])),
      graph: coverage(satRows.filter(row => row.graph.backbone_status === "complete").map(row => [row.reference.backbone.length, row.graph.clues.length])),
    },
    comparisons: {
      graph_decision_vs_dpll: comparison(rows.map(row => [
        row.graph.decision === "unknown-budget" ? null : row.graph.decision_work_units,
        row.dpll.outcome === "unknown-budget" ? null : row.dpll.total_work_units,
      ])),
    },
  };
}

function validateSummaryShape(summary) {
  keys(summary, ["cases", "reference_sat", "reference_unsat", "reference_work_units", "checker_work_units", "certificates_checked", "clues_checked", "arms", "coverage", "comparisons"], "summary");
  keys(summary.arms, ["local", "dpll", "graph"], "summary arms");
  keys(summary.arms.local, LOCAL_SUMMARY_FIELDS, "local summary");
  keys(summary.arms.dpll, DPLL_SUMMARY_FIELDS, "dpll summary");
  keys(summary.arms.graph, GRAPH_SUMMARY_FIELDS, "graph summary");
  keys(summary.coverage, ["local", "graph"], "coverage");
  for (const value of Object.values(summary.coverage)) keys(value, COVERAGE_FIELDS, "coverage counters");
  keys(summary.comparisons, ["graph_decision_vs_dpll"], "summary comparisons");
  keys(summary.comparisons.graph_decision_vs_dpll, COMPARISON_FIELDS, "comparison");
}

export function validateImplicationCalibration(report) {
  keys(report, ["schema_version", "experiment", "status", "proof_status", "protocol_sha256", "arm_budget", "counter_model", "limitations", "frozen_library", "setup", "corpus", "observations", "summary", "outside_arms", "decision_comparison", "encoding", "families"], "implication-calibration report");
  require(report.schema_version === 1 && report.experiment === "implication-calibration-v1" && report.status === "bounded-tested" &&
    report.proof_status === "no-formal-proof" && report.protocol_sha256 === IMPLICATION_PROTOCOL_SHA256,
  "Expected the fixed bounded implication-calibration-v1 protocol without a formal-proof claim.");
  require(report.arm_budget === BUDGET, "Unexpected shared arm budget.");
  require(report.counter_model === "transfer-v1-events-with-graph-phases", "Unexpected counter model.");
  validateLimitations(report.limitations);
  keys(report.frozen_library, ["source_rule_instances", "training_pairs", "frozen_before_heldout", "rules"], "frozen library");
  require(report.frozen_library.source_rule_instances === 4 && report.frozen_library.training_pairs === 6 && report.frozen_library.frozen_before_heldout === true, "Unexpected frozen-library contract.");
  validateFrozenRules(report.frozen_library.rules);
  keys(report.setup, ["acquisition"], "setup");
  validateAcquisition(report.setup.acquisition);
  equal(report.corpus, CORPUS, "Unexpected implication-calibration corpus.");
  equal(report.decision_comparison, DECISION_COMPARISON, "Unexpected decision-comparison declaration.");
  equal(report.encoding, ENCODING, "Unexpected encoding declaration.");
  require(Array.isArray(report.observations) && report.observations.length === 120, "Expected every fixed calibration case.");
  const expected = expectedCases();
  for (const row of report.observations) {
    require(row && typeof row === "object" && expected.has(row.id), "Unexpected or duplicate calibration case identity.");
    const fixed = expected.get(row.id);
    expected.delete(row.id);
    validateObservation(row, fixed);
  }
  validateSummaryShape(report.summary);
  equal(report.summary, summarize(report.observations), "Summary differs from per-case observations.");
  const families = [...new Set(report.observations.map(row => row.family))].sort();
  keys(report.families, families, "family summaries");
  for (const family of families) {
    validateSummaryShape(report.families[family]);
    equal(report.families[family], summarize(report.observations.filter(row => row.family === family)), "Family summary differs from observations.");
  }
  equal(report.outside_arms, {
    acquisition_work_units: report.setup.acquisition.work_units,
    reference_work_units: report.summary.reference_work_units,
    checker_work_units: report.summary.checker_work_units,
  }, "Outside-arm totals are inconsistent.");
  return report;
}
