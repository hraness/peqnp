import { spawnSync } from "node:child_process";
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

const scratchRoot = resolve(root, ".work");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(resolve(scratchRoot, "replay-"));
try {
  const report = resolve(scratch, "calibration.json");
  run("cargo", ["run", "--locked", "--release", "--", "experiment", report]);
  if (!readFileSync(report).equals(readFileSync(resolve(root, "artifacts/calibration.json")))) {
    throw new Error("Calibration replay differs from the reviewed artifact");
  }
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
run("git", ["diff", "--check"]);
run("git", ["diff", "--cached", "--check"]);
console.log("All checks passed; calibration reproduced byte for byte.");
