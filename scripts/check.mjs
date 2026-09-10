import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { ANSWER_FILE_CAP_BYTES, ORACLE_PINS, validateAnswerFile } from "./oh-report-checks.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));

function run(command, args, capture = false) {
  console.log(`> ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    stdio: capture ? "pipe" : "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} failed (${result.status ?? result.signal}): ${result.stderr ?? ""}`);
  }
  return result.stdout?.trim();
}

const channel = readFileSync(resolve(root, "rust-toolchain.toml"), "utf8")
  .match(/^channel = "([^"]+)"$/m)?.[1];
if (!channel) throw new Error("Missing pinned Rust channel");
for (const tool of ["rustc", "cargo"]) {
  const version = run(tool, ["--version"], true);
  if (!version.startsWith(`${tool} ${channel} `)) {
    throw new Error(`Expected ${tool} ${channel}, received ${version}`);
  }
}
if (run("bun", ["--version"], true) !== "1.3.14") {
  throw new Error("Bun 1.3.14 is required");
}

// Oracle experiments: [experiment, committed oracle-answer file]. Replay of
// each experiment's artifact needs no solver; regeneration of its answer file
// runs only where the pinned binary is present and matching. Empty until the
// first protocol above the truth-table bound commits its answer file.
const ORACLE_ANSWERS = [];
const REPLAY_WALL_SECONDS_BOUND = 120;

// Present and matching, absent, or present with another build's digest.
function oraclePresent() {
  let resolved;
  let digest;
  try {
    resolved = realpathSync(ORACLE_PINS.path);
    digest = createHash("sha256").update(readFileSync(resolved)).digest("hex");
  } catch {
    return { state: "missing" };
  }
  return digest === ORACLE_PINS.sha256 ? { state: "present", resolved } : { state: "mismatch", resolved, digest };
}

// The skip is reported once per gate run, never per answer file, and
// PEQNP_REQUIRE_ORACLE=1 fails the gate even when no answer file is listed.
function oracleSkipMessage(oracle) {
  if (oracle.state === "missing") return `oracle regeneration skipped: ${ORACLE_PINS.path} missing; committed oracle-answer files verified by replay only`;
  if (oracle.state === "mismatch") return `oracle regeneration skipped: digest mismatch at ${oracle.resolved} (found ${oracle.digest})`;
  return null;
}

function firstDifferingEntry(committed, regenerated) {
  const left = new Map(committed.entries.map(entry => [entry.dimacs_sha256, entry.label]));
  const right = new Map(regenerated.entries.map(entry => [entry.dimacs_sha256, entry.label]));
  for (const digest of [...new Set([...left.keys(), ...right.keys()])].sort()) {
    if (left.get(digest) !== right.get(digest)) {
      return `entry ${digest}: committed ${left.get(digest) ?? "absent"}, regenerated ${right.get(digest) ?? "absent"}`;
    }
  }
  return "identical labels; the difference is inside a model or proof text";
}

run("cargo", ["fmt", "--all", "--", "--check"]);
run("cargo", ["clippy", "--locked", "--all-targets", "--", "-D", "warnings"]);
run("cargo", ["test", "--locked"]);
const oracle = oraclePresent();
const oracleSkip = oracleSkipMessage(oracle);
if (oracleSkip && process.env.PEQNP_REQUIRE_ORACLE === "1") throw new Error(oracleSkip);
if (oracleSkip) console.log(oracleSkip);
if (oracle.state === "present") run("cargo", ["test", "--locked", "--", "--ignored"]);
run("bun", ["run", "check:oh"]);
run("bun", ["run", "check:ledger"]);

const PROTOCOLS = [
  ["artifacts/clue-transfer.json", "experiments/clue-transfer-protocol.md"],
  ["artifacts/indexed-transfer.json", "experiments/indexed-transfer-protocol.md"],
  ["artifacts/implication-calibration.json", "experiments/implication-protocol.md"],
  ["artifacts/fragment-interface.json", "experiments/fragment-interface-protocol.md"],
  ["artifacts/extraction-cost.json", "experiments/extraction-cost-protocol.md"],
];
for (const [artifact, protocol] of PROTOCOLS) {
  const report = JSON.parse(readFileSync(resolve(root, artifact), "utf8"));
  const protocolHash = createHash("sha256").update(readFileSync(resolve(root, protocol))).digest("hex");
  if (report.protocol_sha256 !== protocolHash) {
    throw new Error(`${artifact} does not identify the checked-in ${protocol}`);
  }
}

// Every committed oracle-answer file is structurally checked on every host
// before replay; replay itself re-verifies every model and proof in Rust.
for (const [experiment, answers] of ORACLE_ANSWERS) {
  const bytes = readFileSync(resolve(root, answers));
  if (bytes.length > ANSWER_FILE_CAP_BYTES) throw new Error(`${answers} exceeds ${ANSWER_FILE_CAP_BYTES} bytes`);
  const file = JSON.parse(bytes.toString("utf8"));
  if (file.experiment !== experiment) throw new Error(`${answers} names experiment ${file.experiment}, expected ${experiment}`);
  validateAnswerFile(file, bytes.length);
}

const scratchRoot = resolve(root, ".work");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(resolve(scratchRoot, "replay-"));
const regenerated = [];
const skipped = [];
try {
  const replays = [
    ["experiment", "calibration.json"],
    ["transfer", "clue-transfer.json"],
    ["indexed", "indexed-transfer.json"],
    ["implication", "implication-calibration.json"],
    ["fragment", "fragment-interface.json"],
    ["extraction", "extraction-cost.json"],
    ...ORACLE_ANSWERS.map(([experiment, answers]) => [experiment, `${experiment}-reference.json`, answers]),
  ];
  for (const [command, filename, answers] of replays) {
    const report = resolve(scratch, filename);
    const started = performance.now();
    run("cargo", ["run", "--locked", "--release", "--", command, report, ...(answers ? ["--oracle-answers", resolve(root, answers)] : [])]);
    const seconds = (performance.now() - started) / 1000;
    console.log(`${command} replay took ${seconds.toFixed(1)} s`);
    if (seconds > REPLAY_WALL_SECONDS_BOUND) console.warn(`warning: ${command} replay exceeded the ${REPLAY_WALL_SECONDS_BOUND} s review bound`);
    if (!readFileSync(report).equals(readFileSync(resolve(root, "artifacts", filename)))) {
      throw new Error(`${filename} replay differs from the reviewed artifact`);
    }
  }
  for (const [experiment, answers] of ORACLE_ANSWERS) {
    if (oracleSkip) {
      skipped.push(experiment);
      continue;
    }
    const fresh = resolve(scratch, `${experiment}-oracle.json`);
    run("cargo", ["run", "--locked", "--release", "--", "oracle-answers", experiment, fresh]);
    const committed = readFileSync(resolve(root, answers));
    const produced = readFileSync(fresh);
    if (!produced.equals(committed)) {
      const detail = firstDifferingEntry(JSON.parse(committed.toString("utf8")), JSON.parse(produced.toString("utf8")));
      throw new Error(`${answers} regeneration differs from the committed file at ${detail}`);
    }
    regenerated.push(experiment);
  }
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
run("git", ["diff", "--check"]);
run("git", ["diff", "--cached", "--check"]);
const experiments = 6 + ORACLE_ANSWERS.length;
const oracleLine = skipped.length > 0 || oracle.state !== "present"
  ? `oracle regeneration skipped for [${skipped.join(", ")}]`
  : `${regenerated.length} oracle-answer files regenerated identically`;
console.log(`All checks passed; the canonical ledger replayed, five protocol digests matched, ${experiments} experiments reproduced byte for byte; ${oracleLine}.`);
