import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createKnowledgeGraphRecordV1 } from "@hraness/oh";
import {
  CALIBRATION_PROGRAMS, commitAdditive, initializeLedger, openLedger, recordCalibration, validateCalibration,
} from "./oh-ledger.mjs";

const roots = [];
function root() {
  const path = mkdtempSync(join(tmpdir(), "peqnp-oh-test-"));
  roots.push(path);
  return path;
}
afterEach(() => { for (const path of roots.splice(0)) rmSync(path, { recursive: true, force: true }); });

function report() {
  return {
    schema_version: 1,
    experiment: "unit-propagation-calibration-v1",
    status: "bounded-tested",
    grammar_size: 8,
    corpus: { variables: 2, canonical_clauses: 16, formulas: 65536, assignments_per_formula: 4 },
    candidates: CALIBRATION_PROGRAMS.map((program, index) => ({
      program,
      preservation_passes: index === 3 ? 65536 : 100,
      progress_passes: index === 3 ? 65536 : 100,
      checked_formulas: 65536,
      first_counterexample: index === 3 ? null : { input: [[1], [-1]], output: [[1]], input_sat: false, output_sat: true },
      work_units: 123,
    })),
    selected_program: CALIBRATION_PROGRAMS[3],
    false_completeness_claim: { input: [[1, 2], [1, -2], [-1, 2], [-1, -2]], satisfiable: false, has_unit: false, unchanged: true, rejected: true },
    proof_status: "informal-general-argument-not-kernel-checked",
    limitations: ["Synthetic test fixture, not mathematical evidence."],
  };
}

test("read verification cannot initialize absent state, and init is replay-verifiable and idempotent", () => {
  const path = root();
  expect(() => openLedger(path)).toThrow("No ledger exists");
  const first = initializeLedger(path);
  expect(first.inserted).toBe(6);
  expect(first.verification.records).toBe(6);
  expect(first.verification.operations).toBe(1);
  const second = initializeLedger(path);
  expect(second.inserted).toBe(0);
  expect(second.verification).toEqual(first.verification);
});

test("existing conflicting records abort an additive batch without partial writes", () => {
  const path = root();
  initializeLedger(path);
  const oh = openLedger(path);
  try {
    const before = oh.verify();
    const original = oh.get("inquiry:p-equals-np");
    const altered = createKnowledgeGraphRecordV1({ v: 1, key: original.key, kind: original.kind, dependencies: original.dependencies, value: { changed: true } });
    const unrelated = createKnowledgeGraphRecordV1({ v: 1, key: "entity:should-not-exist", kind: "entity", dependencies: [], value: {} });
    expect(() => commitAdditive(oh, [unrelated, altered], "test")).toThrow("conflicts");
    expect(oh.get(unrelated.key)).toBeNull();
    expect(oh.verify()).toEqual(before);
  } finally { oh.store.close(); }
});

test("contract/domain validation rejects proof escalation and incomplete experimental coverage", () => {
  const forged = report();
  forged.status = "proved-generally";
  expect(() => validateCalibration(forged)).toThrow("bounded calibration");
  const incomplete = report();
  incomplete.candidates[0].checked_formulas = 100;
  expect(() => validateCalibration(incomplete)).toThrow("coverage");
  const noControl = report();
  noControl.false_completeness_claim.rejected = false;
  expect(() => validateCalibration(noControl)).toThrow("false claim");
  const ambiguous = report();
  ambiguous.candidates[1].preservation_passes = 65536;
  ambiguous.candidates[1].progress_passes = 65536;
  ambiguous.candidates[1].first_counterexample = null;
  expect(() => validateCalibration(ambiguous)).toThrow("one survivor");
  const wrongGrammar = report();
  wrongGrammar.candidates[0].program = "(different-rule)";
  expect(() => validateCalibration(wrongGrammar)).toThrow("exact distinct");
  const wrongControl = report();
  wrongControl.false_completeness_claim.input = [];
  expect(() => validateCalibration(wrongControl)).toThrow("false claim");
  const missingCounterexample = report();
  missingCounterexample.candidates[0].first_counterexample = null;
  expect(() => validateCalibration(missingCounterexample)).toThrow("Counterexample");
});

test("recording preserves raw-result identity and remains idempotent without promoting a proof", () => {
  const path = root();
  mkdirSync(join(path, "artifacts"));
  mkdirSync(join(path, "src"));
  writeFileSync(join(path, "Cargo.toml"), "# synthetic source fixture\n");
  writeFileSync(join(path, "Cargo.lock"), "# synthetic lock fixture\n");
  writeFileSync(join(path, "rust-toolchain.toml"), "# synthetic toolchain fixture\n");
  writeFileSync(join(path, "src/lib.rs"), "// synthetic fixture\n");
  writeFileSync(join(path, "artifacts/result.json"), JSON.stringify(report()));
  initializeLedger(path);
  const first = recordCalibration(path, "artifacts/result.json");
  expect(first.inserted).toBe(4);
  expect(first.verification.records).toBe(10);
  const second = recordCalibration(path, "artifacts/result.json");
  expect(second.inserted).toBe(0);
  expect(second.verification).toEqual(first.verification);
  const oh = openLedger(path);
  try {
    const evidence = oh.list({ kind: "evidence", limit: 10 })[0];
    expect(evidence.value.formalProofAccepted).toBe(false);
    expect(evidence.value.reportSha256).toBe(first.reportSha256);
    expect(oh.list({ kind: "edition", limit: 10 })[0].value.report.status).toBe("bounded-tested");
  } finally { oh.store.close(); }
});

test("missing reports and linked database paths fail before modifying ledger state", () => {
  const path = root();
  initializeLedger(path);
  expect(() => recordCalibration(path, "../escape.json")).toThrow("escapes");
  const oh = openLedger(path);
  const before = oh.verify();
  oh.store.close();
  expect(() => recordCalibration(path, "missing.json")).toThrow();
  const reopened = openLedger(path);
  expect(reopened.verify()).toEqual(before);
  reopened.store.close();
  const linked = root();
  symlinkSync(join(path, ".oh"), join(linked, ".oh"));
  expect(() => initializeLedger(linked)).toThrow("real directory");
});
