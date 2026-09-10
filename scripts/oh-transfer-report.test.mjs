import { afterEach, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { canonicalSha256 } from "@hraness/oh";
import { initializeLedger, openLedger, recordClueTransfer } from "./oh-ledger.mjs";
import { validateClueTransfer } from "./oh-transfer-report.mjs";

const roots = [];
const reportBytes = readFileSync(new URL("../artifacts/clue-transfer.json", import.meta.url));
const protocolBytes = readFileSync(new URL("../experiments/clue-transfer-protocol.md", import.meta.url));
const report = () => JSON.parse(reportBytes);
function root() {
  const path = mkdtempSync(join(tmpdir(), "peqnp-transfer-ledger-test-"));
  roots.push(path);
  for (const directory of ["artifacts", "experiments", "src"]) mkdirSync(join(path, directory));
  for (const file of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]) writeFileSync(join(path, file), "# synthetic source fixture\n");
  writeFileSync(join(path, "src/lib.rs"), "// Synthetic source fixture, not experimental provenance.\n");
  writeFileSync(join(path, "artifacts/clue-transfer.json"), reportBytes);
  writeFileSync(join(path, "experiments/clue-transfer-protocol.md"), protocolBytes);
  return path;
}
afterEach(() => { for (const path of roots.splice(0)) rmSync(path, { recursive: true, force: true }); });

test("transfer report preserves the complete finite comparison and rejects proof or coverage escalation", () => {
  const actual = report();
  expect(validateClueTransfer(actual)).toBe(actual);
  const proof = report();
  proof.proof_status = "proved-generally";
  expect(() => validateClueTransfer(proof)).toThrow("without a formal-proof");
  const missing = report();
  missing.observations.pop();
  expect(() => validateClueTransfer(missing)).toThrow("every fixed held-out");
  const duplicate = report();
  duplicate.observations[1] = duplicate.observations[0];
  expect(() => validateClueTransfer(duplicate)).toThrow("duplicate");
  const changed = report();
  changed.training.candidates[0].satisfying_assignments = 1;
  expect(() => validateClueTransfer(changed)).toThrow("complete tiny domain");
  const rule = report();
  rule.frozen_library.rules[0].conclusion *= -1;
  expect(() => validateClueTransfer(rule)).toThrow("accepted mining rules");
});

test("numeric consistency rejects invented gains, omitted work, wrong answers and unexhausted unknowns", () => {
  const gain = report();
  gain.summary.transfer_better++;
  expect(() => validateClueTransfer(gain)).toThrow("Summary differs");
  const work = report();
  work.observations[0].transfer.preprocessing.work_units--;
  expect(() => validateClueTransfer(work)).toThrow("categories do not sum");
  const answer = report();
  answer.observations[0].baseline.outcome = answer.observations[0].reference_sat ? "unsat" : "sat";
  expect(() => validateClueTransfer(answer)).toThrow("disagrees");
  const unknown = report();
  unknown.observations[0].transfer.outcome = "unknown-budget";
  expect(() => validateClueTransfer(unknown)).toThrow("exhausted work budget");
  const cold = report();
  cold.acquisition_plus_transfer_work_units--;
  expect(() => validateClueTransfer(cold)).toThrow("Acquisition-plus-online");
  const overflow = report();
  overflow.training.acquisition.work_units = Number.MAX_SAFE_INTEGER + 1;
  expect(() => validateClueTransfer(overflow)).toThrow("safe event count");
});

test("local ingestion is additive, repeatable, and records costs without a speedup proposition", () => {
  const path = root();
  initializeLedger(path);
  const first = recordClueTransfer(path, "artifacts/clue-transfer.json");
  expect(first.inserted).toBe(5);
  expect(first.verification.records).toBe(11);
  const second = recordClueTransfer(path, "artifacts/clue-transfer.json");
  expect(second.inserted).toBe(0);
  expect(second.verification).toEqual(first.verification);
  const oh = openLedger(path);
  try {
    const observation = oh.list({ kind: "assertion", limit: 20 }).find(record => record.key.startsWith("assertion:clue-transfer-"));
    expect(observation.value.stance).toBe("bounded-observation");
    expect(observation.value.formalProofAccepted).toBe(false);
    const evidence = oh.list({ kind: "evidence", limit: 20 })[0];
    expect(evidence.value.measuredSummary).toEqual(report().summary);
    expect(evidence.value.frozenLibrarySha256).toBe(canonicalSha256(report().frozen_library.rules));
    expect(evidence.value.acquisitionPlusTransferWorkUnits).toBe(report().acquisition_plus_transfer_work_units);
    expect(oh.get("assertion:p-equals-np-hypothesis").value.stance).toBe("research-hypothesis");
  } finally { oh.store.close(); }
});

test("invalid report and changed protocol fail before initializing or changing research state", () => {
  const path = root();
  const wrong = report();
  wrong.summary.transfer_worse--;
  writeFileSync(join(path, "artifacts/clue-transfer.json"), JSON.stringify(wrong));
  expect(() => recordClueTransfer(path, "artifacts/clue-transfer.json")).toThrow("Summary differs");
  expect(existsSync(join(path, ".oh"))).toBe(false);
  writeFileSync(join(path, "artifacts/clue-transfer.json"), reportBytes);
  const initial = initializeLedger(path);
  writeFileSync(join(path, "experiments/clue-transfer-protocol.md"), "changed protocol\n");
  expect(() => recordClueTransfer(path, "artifacts/clue-transfer.json")).toThrow("fixed protocol identity");
  const oh = openLedger(path);
  expect(oh.verify()).toEqual(initial.verification);
  oh.store.close();
});
