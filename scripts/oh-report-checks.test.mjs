import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
  ANSWER_FILE_CAP_BYTES, ORACLE_PINS, PROOF_TEXT_CAP_BYTES, REFERENCE_MODEL, TRUTH_TABLE_MAX_VARIABLES,
  integerThousandths, oracleReference, ratioPair, sha256Hex, validateAnswerFile, writeDimacs,
} from "./oh-report-checks.mjs";

const read = path => readFileSync(new URL(path, import.meta.url), "utf8");
const answersText = read("../tests/oracle/demo-oracle.json");
const answers = () => JSON.parse(answersText);
const artifact = () => JSON.parse(read("../tests/oracle/demo-reference.json"));
const LIMITS = { conflicts: 100 };

function parseDimacs(text) {
  const lines = text.split("\n").filter(line => line.length > 0);
  const [, , variables] = lines[0].split(" ");
  const input = lines.slice(1).map(line => line.split(" ").map(Number).filter(lit => lit !== 0));
  return { input, variables: Number(variables) };
}
const fixtures = {
  r16: parseDimacs(read("../tests/lrat/r16.cnf")),
  r30: parseDimacs(read("../tests/lrat/r30.cnf")),
  s60: parseDimacs(read("../tests/oracle/s60.cnf")),
};
const row = id => fixtures[id];
const reference = id => artifact().cases.find(c => c.id === id).reference;

test("the JavaScript pins equal the Rust constants in src/oracle.rs", () => {
  const source = read("../src/oracle.rs");
  expect(source).toContain(`name: "${ORACLE_PINS.name}",`);
  expect(source).toContain(`path: "${ORACLE_PINS.path}",`);
  expect(source).toContain(`version: "${ORACLE_PINS.version}",`);
  expect(source).toContain(`sha256: "${ORACLE_PINS.sha256}",`);
  expect(source).toContain(`FIXED_ARGUMENTS: [&str; 3] = [${ORACLE_PINS.arguments.map(a => JSON.stringify(a)).join(", ")}];`);
  expect(source).toContain(`TRUTH_TABLE_MAX_VARIABLES: u32 = ${TRUTH_TABLE_MAX_VARIABLES};`);
  expect(source).toContain(`PROOF_TEXT_CAP_BYTES: u64 = ${PROOF_TEXT_CAP_BYTES.toLocaleString("en-US").replaceAll(",", "_")};`);
  expect(source).toContain(`ANSWER_FILE_CAP_BYTES: u64 = ${ANSWER_FILE_CAP_BYTES.toLocaleString("en-US").replaceAll(",", "_")};`);
  expect(source).toContain(`"${REFERENCE_MODEL}"`);
});

test("integer thousandths round half up in BigInt and reject non-integers", () => {
  expect(integerThousandths(1, 2)).toBe("0.500");
  expect(integerThousandths(1, 3)).toBe("0.333");
  expect(integerThousandths(2, 3)).toBe("0.667");
  expect(integerThousandths(1, 16)).toBe("0.063");
  expect(integerThousandths(3, 16)).toBe("0.188");
  expect(integerThousandths(0, 7)).toBe("0.000");
  expect(integerThousandths(1_518_173, 124_045)).toBe("12.239");
  expect(integerThousandths(Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER)).toBe("1.000");
  expect(integerThousandths(Number.MAX_SAFE_INTEGER, 1)).toBe("9007199254740991.000");
  expect(integerThousandths(5, 0)).toBe(null);
  expect(() => integerThousandths(1.5, 2)).toThrow("nonnegative safe");
  expect(ratioPair(null)).toBe(null);
  expect(ratioPair({ numerator: 1, denominator: 16, thousandths: "0.063" }).thousandths).toBe("0.063");
  expect(() => ratioPair({ numerator: 1, denominator: 16, thousandths: "0.062" })).toThrow("thousandths");
  expect(() => ratioPair({ numerator: 1, denominator: 0, thousandths: "0.000" })).toThrow("zero denominator");
  expect(() => ratioPair({ numerator: 1, denominator: 2 })).toThrow("Unexpected ratio fields");
});

test("writeDimacs reproduces the fixtures byte for byte and the edge layouts", () => {
  for (const [id, path] of [["r16", "../tests/lrat/r16.cnf"], ["r30", "../tests/lrat/r30.cnf"], ["s60", "../tests/oracle/s60.cnf"]]) {
    expect(writeDimacs(row(id).input, row(id).variables)).toBe(read(path));
  }
  expect(writeDimacs([], 3)).toBe("p cnf 3 0\n");
  expect(writeDimacs([[]], 2)).toBe("p cnf 2 1\n0\n");
  expect(writeDimacs([[1, -4]], 9)).toBe("p cnf 9 1\n1 -4 0\n");
  expect(writeDimacs([], 0)).toBe("p cnf 0 0\n");
  expect(sha256Hex(writeDimacs(row("r16").input, 16))).toBe("ac7a9aebb4a81f2d4dfe51bee07e37cab97da434df560c20d0f1b472036f109e");
});

test("the demo answer file passes the structural check and rejects damage", () => {
  const { conflictLimit, entries } = validateAnswerFile(answers(), Buffer.byteLength(answersText));
  expect(conflictLimit).toBe(100);
  expect([...entries.values()].map(e => e.label)).toEqual(["unsat", "sat", "unknown"]);
  const mutate = (edit, message) => {
    const file = answers();
    edit(file);
    expect(() => validateAnswerFile(file)).toThrow(message);
  };
  mutate(file => { file.oracle.version = "3.0.0"; }, "version differs");
  mutate(file => { file.oracle.arguments[2] = "--binary=true"; }, "pinned invocation");
  mutate(file => { file.proof_text_cap_bytes = 1; }, "cap differs");
  mutate(file => { file.entries.reverse(); }, "sorted and unique");
  mutate(file => { file.entries.push(file.entries[2]); }, "sorted and unique");
  mutate(file => { file.entries[1].model.reverse(); }, "ascending and complete");
  mutate(file => { file.entries[0].lrat_bytes += 1; }, "lrat_bytes");
  mutate(file => { file.entries[0].lrat_sha256 = "0".repeat(64); }, "lrat_sha256");
  mutate(file => { file.entries[0].lrat = "x" + file.entries[0].lrat; }, "character outside");
  mutate(file => { file.entries[2].reason = "timeout"; }, "unknown reason");
  mutate(file => { file.entries[2].conflict_limit = 99; }, "conflict limit differs");
  mutate(file => { file.entries[1].variables = 30.5; }, "Non-integer");
  mutate(file => { file.extra = 1; }, "Unexpected oracle-answer file fields");
  expect(() => validateAnswerFile(answers(), ANSWER_FILE_CAP_BYTES + 1)).toThrow("exceeds");
});

test("oracleReference accepts the three demo rows and rejects every mismatch", () => {
  const file = answers();
  expect(oracleReference(reference("r16"), row("r16"), ORACLE_PINS, LIMITS, file)).toBe("unsat");
  expect(oracleReference(reference("r30"), row("r30"), ORACLE_PINS, LIMITS, file)).toBe("sat");
  expect(oracleReference(reference("s60"), row("s60"), ORACLE_PINS, LIMITS, file)).toBe("unknown");
  const reject = (id, edit, message, caseRow = row(id)) => {
    const ref = reference(id);
    edit(ref);
    expect(() => oracleReference(ref, caseRow, ORACLE_PINS, LIMITS, file)).toThrow(message);
  };
  reject("r16", ref => { ref.label = "sat"; }, "No committed answer");
  reject("r16", ref => { ref.label = "unknown"; }, "Unexpected oracle reference fields");
  reject("s60", ref => { delete ref.reason; ref.label = "unsat"; }, "No committed answer");
  reject("r16", ref => { ref.dimacs_sha256 = "0".repeat(64); }, "does not identify");
  reject("r16", () => {}, "does not identify", row("r30"));
  reject("r16", ref => { ref.conflict_limit = 200; }, "conflict limit differs");
  reject("r16", ref => { ref.oracle.sha256 = "0".repeat(64); }, "sha256 differs");
  reject("r16", ref => { ref.proof_check.valid = false; }, "must be valid");
  reject("r16", ref => { ref.proof_check.lrat_bytes = 388; }, "Proof digest or length");
  reject("r16", ref => { ref.proof_check.work.work_units += 1; }, "sum to work_units");
  reject("r30", ref => { ref.model[0] = -ref.model[0]; }, "leaves a clause unsatisfied");
  reject("r30", ref => { ref.model = [...ref.model].reverse(); }, "ascending and complete");
  reject("r30", ref => { ref.model_check.valid = false; }, "must be valid");
  reject("r30", ref => { ref.proof_check = {}; }, "no proof check");
  reject("s60", ref => { ref.reason = "proof-over-cap"; }, "differs from the committed answer");
  reject("s60", ref => { ref.model = []; }, "no certificate");
  // A committed answer with another label is not accepted.
  const swapped = answers();
  swapped.entries[2].label = "sat";
  swapped.entries[2].model = Array.from({ length: 60 }, (_, i) => i + 1);
  delete swapped.entries[2].reason;
  expect(() => oracleReference(reference("s60"), row("s60"), ORACLE_PINS, LIMITS, swapped)).toThrow("No committed answer");
  // Oracle rows at or below the bound are refused.
  expect(() => oracleReference(reference("r16"), { input: [[1]], variables: 12 }, ORACLE_PINS, LIMITS, file)).toThrow("truth-table bound");
});

test("truth-table and definition rows keep their own shapes", () => {
  const work = (assignments, extra = {}) => ({
    work_units: 3 + assignments, formula_checks: 1, clause_reads: 1, literal_reads: 1, clause_writes: 0,
    literal_writes: 0, assignments, pair_checks: 0, rule_attempts: 0, search_nodes: 0, ...extra,
  });
  const table = { kind: "truth-table", model_count: 2, sat: true, backbone: [2], work: work(4) };
  expect(oracleReference(table, { input: [[1, 2], [-1, 2]], variables: 2 }, ORACLE_PINS, LIMITS, new Map())).toBe("sat");
  expect(() => oracleReference({ ...table, work: work(3) }, { input: [[1, 2]], variables: 2 }, ORACLE_PINS, LIMITS, new Map())).toThrow("coverage");
  expect(() => oracleReference({ ...table, sat: false }, { input: [[1, 2]], variables: 2 }, ORACLE_PINS, LIMITS, new Map())).toThrow("sat flag");
  expect(() => oracleReference({ ...table, backbone: [2, 1] }, { input: [[1, 2]], variables: 2 }, ORACLE_PINS, LIMITS, new Map())).toThrow("variable order");
  expect(() => oracleReference(table, { input: [[1, 2]], variables: 13 }, ORACLE_PINS, LIMITS, new Map())).toThrow("above the truth-table bound");
  const unsat = { kind: "definition", label: "unsat", certificate: { kind: "empty-clause", clause_index: 1 }, work: work(0) };
  expect(oracleReference(unsat, { input: [[1], []], variables: 13 }, ORACLE_PINS, LIMITS, new Map())).toBe("unsat");
  expect(() => oracleReference(unsat, { input: [[1], [2]], variables: 13 }, ORACLE_PINS, LIMITS, new Map())).toThrow("empty clause");
  expect(() => oracleReference(unsat, { input: [[1], []], variables: 12 }, ORACLE_PINS, LIMITS, new Map())).toThrow("below the truth-table bound");
  const model = Array.from({ length: 13 }, (_, i) => -(i + 1));
  const sat = { kind: "definition", label: "sat", model, model_check: { valid: true, work: work(0) } };
  expect(oracleReference(sat, { input: [], variables: 13 }, ORACLE_PINS, LIMITS, new Map())).toBe("sat");
  expect(() => oracleReference(sat, { input: [[1]], variables: 13 }, ORACLE_PINS, LIMITS, new Map())).toThrow("with clauses");
  expect(() => oracleReference({ ...sat, model: model.map(l => -l) }, { input: [], variables: 13 }, ORACLE_PINS, LIMITS, new Map())).toThrow("all-false");
  expect(() => oracleReference({ ...sat, model_count: 1 }, { input: [], variables: 13 }, ORACLE_PINS, LIMITS, new Map())).toThrow("Unexpected definition reference fields");
});
