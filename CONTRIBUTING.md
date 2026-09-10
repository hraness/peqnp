# Contributing

Start with the [research proposal](docs/research-proposal.md) and [agent operating rules](AGENTS.md). Propose a precise question, a candidate rule, a counterexample, or a focused implementation improvement.

Keep experimental observations distinct from general theorems. Include exact inputs, grammar and resource limits, reproduction commands, and an independent check. Do not submit private corpora or local Oh databases. A useful negative result explains the failed claim and preserves its smallest known counterexample.

Record research claims, evidence, and reviewed document editions in the repository's canonical [Oh ledger](docs/oh.md). Submit additive, reviewed operation-history changes with their readable documents and artifacts; preserve the existing history. Do not maintain a parallel research record in Jungle KB.

Use the pinned Rust toolchain and Bun 1.3.14. Install dependencies with `bun install --frozen-lockfile --ignore-scripts`, run focused tests, then `bun run check`. Obtain independent review for changes to the language, evaluator, reference oracle, or scoring. The final continuous integration check must pass for the delivered commit.

Follow the [Hraness documentation guidelines](https://github.com/hraness/.github/blob/main/DOCUMENTATION_GUIDELINES.md) and keep README commands synchronized with working examples. Open a pull request for external contributions. Maintainers may integrate reviewed changes through the repository's permitted non-force workflow after checks pass.
