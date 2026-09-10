import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

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

run("cargo", ["fmt", "--all", "--", "--check"]);
run("cargo", ["clippy", "--locked", "--all-targets", "--", "-D", "warnings"]);
run("cargo", ["test", "--locked"]);
run("bun", ["run", "check:oh"]);
run("bun", ["run", "check:ledger"]);

const transfer = JSON.parse(readFileSync(resolve(root, "artifacts/clue-transfer.json"), "utf8"));
const protocolHash = createHash("sha256")
  .update(readFileSync(resolve(root, "experiments/clue-transfer-protocol.md")))
  .digest("hex");
if (transfer.protocol_sha256 !== protocolHash) {
  throw new Error("Clue-transfer report does not identify the checked-in protocol");
}

const scratchRoot = resolve(root, ".work");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(resolve(scratchRoot, "replay-"));
try {
  for (const [command, filename] of [["experiment", "calibration.json"], ["transfer", "clue-transfer.json"]]) {
    const report = resolve(scratch, filename);
    run("cargo", ["run", "--locked", "--release", "--", command, report]);
    if (!readFileSync(report).equals(readFileSync(resolve(root, "artifacts", filename)))) {
      throw new Error(`${filename} replay differs from the reviewed artifact`);
    }
  }
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
run("git", ["diff", "--check"]);
run("git", ["diff", "--cached", "--check"]);
console.log("All checks passed; the canonical ledger replayed and both experiments reproduced byte for byte.");
