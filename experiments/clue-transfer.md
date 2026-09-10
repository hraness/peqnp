# Clues transferred, but their lookup cost more than they saved

Completed finite experiment, 9 September 2026. On all 122 tested formulas, adding the learned-rule preprocessor increased total measured work. Both arms returned the correct decision on every case, with no exhausted budgets. The rules reduced residual search, but matching them was too expensive in this implementation.

The [protocol](clue-transfer-protocol.md) was fixed in commit `4931f7d` before execution. The [full artifact](../artifacts/clue-transfer.json) contains every training candidate, exact held-out CNF, outcome, phase counters, and family summary. Its SHA-256 is `dd3293c5466fb64eb7b0406e61afb42f3ae52d00ea86073ffa83a0dd184fd85f`.

## Reproduce the comparison

Use the repository's Rust 1.97.1 toolchain and Bun 1.3.14:

```sh
cargo run --locked --release -- transfer artifacts/clue-transfer.json
bun run check
```

The final check verifies the protocol hash and reproduces both experiment artifacts byte for byte. To record this observation in the local Oh ledger, after `bun run oh:init`, run `bun run oh:record:transfer` and `bun run oh:verify`. The ledger remains local and ignored by Git.

## What was learned

The miner evaluated 24 possible implications over six two-clause formulas, exhaustively checking four assignments per candidate. Four unit conclusions survived. They are instances of one known resolution schema:

\[
(a\lor b)\land(a\lor\neg b)\models a.
\]

Actual mined rule objects were frozen before the held-out corpus was generated or labeled. A generic matcher used those objects under signed, injective variable renaming. Their soundness in a surrounding formula follows from a short [renaming and entailment argument](../docs/clue-transfer-theory.md#why-transfer-is-sound). This is checked reuse of a familiar rule, not novel mathematical discovery.

The transfer arm made one preprocessing pass, added the resulting units, then called the same deterministic DPLL solver as the baseline. Both arms had one million total events per case; preprocessing consumed the transfer arm's allowance. The held-out truth-table oracle was outside both arms.

## Results

| Measured quantity | Without added library | With frozen library |
| --- | ---: | ---: |
| Correct completed decisions | 122 | 122 |
| Unknown due to budget | 0 | 0 |
| Search nodes | 738 | 620 |
| Residual solver work | 424,997 | 376,636 |
| Preprocessing work | 0 | 1,300,009 |
| Total online work | 424,997 | 1,676,645 |

The library produced 127 derived units across the corpus. Search nodes fell by about 16%, while online work increased to about 3.95 times baseline. Mining cost another 1,000 events, bringing acquisition plus transfer work to 1,677,645. Independent reference labeling cost 3,047,512 additional research events, excluded from both algorithm totals.

| Family | Cases | Derived units | Baseline work | Transfer total |
| --- | ---: | ---: | ---: | ---: |
| Random mixed 2/3-CNF | 48 | 91 | 132,895 | 1,180,559 |
| Random 3-CNF | 48 | 0 | 270,096 | 292,560 |
| Forced-true mechanism controls | 4 | 18 | 1,418 | 18,806 |
| Forced-false mechanism controls | 4 | 18 | 3,096 | 18,806 |
| Parity cycles | 8 | 0 | 9,356 | 136,528 |
| Pigeonhole | 2 | 0 | 6,016 | 26,642 |
| Ternary UNSAT cubes | 4 | 0 | 1,616 | 2,032 |
| Hidden-backbone controls | 4 | 0 | 504 | 712 |

Transfer was worse on every individual case, not just in aggregate. Reusing the library across more repetitions of this tested workload cannot repay acquisition: its online cost is already higher. There is no observed break-even point to report.

The hidden-backbone controls are satisfiable and force a literal, but contain no binary clauses. They expose a coverage limit: zero matches does not mean there are no implicit clues. The ternary cubes are UNSAT. Both sets use three active variables with larger declared labels/padding and are logical controls, not evidence about scaling. Generated formulas use fixed seeds 7, 41, 2026, and 65537; the artifact records all parameters. This small, constructed corpus is not a representative SAT workload.

## What the work counter means

Every work unit increments one checked counter: a formula check, clause read, literal/scalar read, clause write, literal/scalar write, assignment, pair check, rule attempt, or search node. Logical copies and validation traversals are included. The generic matcher scans binary-clause pairs and tries the four stored rule instances in both literal orders. Its repeated attempts and copying dominate this result.

These are declared operational events, not elapsed time or a proof about bit operations. Allocation internals, arithmetic/comparison instructions, loop control, corpus generation, and report construction are outside these counts. The two arms share the same residual solver and event definitions, so this is a controlled implementation comparison. It neither ranks competitive SAT solvers nor proves that useful preprocessing must be expensive.

Focused checks compare both arms with an independent reference on all 4,096 ordered pairs of canonical three-variable clauses, test empty and malformed inputs, check signed matching and an empty-library control, and exercise exhaustion at the shared-budget boundary. Independent review also checked the oracle boundary, protocol, scoring, and general rule argument.

## What this changes about the next experiment

The idea of reusable clues survives as a sound mechanism, while this particular application strategy loses. Keep this result unchanged. A new protocol can test an indexed, symmetry-deduplicated matcher against the same inference semantics, using fresh evaluation cases after implementation choices are made. Selection should optimize total cost, not count of clues or branches removed.

Coverage needs a separate investigation. The [theory note](../docs/clue-transfer-theory.md) gives a satisfiable 2-CNF family whose forced literal needs arbitrarily many source clauses to explain. Longer implication paths recover that clue even though fixed two-clause unit patterns cannot start. That motivates studying compositional inference and a standard 2-SAT baseline before widening general-SAT claims. None of these finite observations establishes P = NP or an evolutionary-search advantage.
