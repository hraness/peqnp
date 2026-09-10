# Security

The candidate evaluator runs the project's restricted language. It does not execute arbitrary Rust, shell commands, or model-generated host code. This research prototype is not a general sandbox for hostile native programs.

Keep `.oh/`, private datasets, and credentials out of Git. Oh's replay verification checks record integrity, not the truth of a mathematical claim. Do not publish raw private research records as part of a bug report.

Report vulnerabilities through [GitHub private vulnerability reporting](https://github.com/hraness/peqnp/security/advisories/new). Include a minimal reproducer without secrets. For ordinary correctness problems, open an issue with the failing input and expected behavior.
