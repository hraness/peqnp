# Reading the binary fragment paid off only when it decided the whole formula

Completed finite experiment, 10 September 2026. On 162 mixed 2/3-CNF formulas, building the implication graph of the binary fragment, certifying it, extracting its forced literals, and handing them to the solver cost more total declared events than the solver alone on 140 cases and less on 22. All 22 wins except one are cases where the fragment was itself unsatisfiable, so its certificate decided the whole formula with no search at all. On the ternary-only overhead controls the fragment arm's residual matched the baseline exactly, so every extra event there was the price of building and reading an empty interface. Every decision agreed with the independent reference, every certificate and clue path passed a raw-clause checker, and neither arm exhausted its budget.

The [protocol](fragment-interface-protocol.md) was fixed in commit `779b715` before execution. Its SHA-256, `bce209aa982a9964cf235758e43c7b6f405d7735a3c52c11cc867fbe9e60ec36`, is embedded in `src/fragment.rs` and in the artifact. The [full artifact](../artifacts/fragment-interface.json) contains every input CNF with its generation parameters, the reference model count and backbone, both arms' counters with the fragment arm's six phases, every certificate and clue path, the checker's verdicts and work, and the summary, family, and subtotal tables. Its SHA-256 is `ee4d85dbfd4ee3c981f401d482c61bc05fa68f035de6d54b853b03c30d3a6375`.

## Reproduce the comparison

Use the repository's Rust 1.97.1 toolchain and Bun 1.3.14:

```sh
cargo run --locked --release -- fragment artifacts/fragment-interface.json
bun run check
```

The first command rewrites the artifact deterministically; `shasum -a 256 artifacts/fragment-interface.json` must print the digest above. The second command runs the repository gate: formatting, Clippy, the test suite, the ledger checks, a check that each artifact's embedded protocol digest matches its committed protocol file, and byte-for-byte replay of all five experiment artifacts.

## What was tested

The three earlier experiments measured negative interface deltas on formulas whose residual search was already near zero. This one adds ternary clauses so that search is expensive, and treats the binary clauses as a fragment that a known interface can read. The **binary fragment** of a formula is its clauses of width at most two, kept at their original indices. Any literal forced by the fragment is forced by the whole formula, and an unsatisfiable fragment makes the whole formula unsatisfiable, because a satisfying assignment of the whole satisfies the fragment.

Two arms decided every case with one million declared events each:

- **baseline**: the deterministic unit-propagation/DPLL solver on the original formula;
- **fragment interface**: implication-graph construction over the fragment with original clause indices on every edge, two-pass strongly-connected-component computation, and a certificate for the fragment; if the fragment is unsatisfiable, UNSAT is returned with that certificate and no residual solving; otherwise every forced literal of the fragment is extracted by one reachability query per literal in the order `1, -1, 2, -2, ...`, the original formula is copied with those units appended in the order found, and the same DPLL solver runs on the result with the remaining budget.

One meter covers the fragment arm's six phases in order, so the budget cannot be spent twice. The fragment arm uses no learned library and has no acquisition step; the implication-graph procedure is fixed algorithmic knowledge. No truth table, reference label, or reference backbone reaches either arm.

A fragment SAT certificate is one Boolean per declared variable satisfying every width-two clause; it does not claim to satisfy the whole formula. A fragment UNSAT certificate is a variable with implication paths between its two literals in both directions, each path citing the clauses that justify its steps. A clue is a forced literal with such a path. A separate checker validated all 162 certificates and all 205 clue paths directly against the raw clauses, without rebuilding the graph, for 112,689 events. An independent truth-table enumeration labeled every case after the corpus was fixed: 56 satisfiable and 106 unsatisfiable, for 6,482,442 events. Reference and checker work are outside both arms.

The corpus is the protocol's fixed 162 cases in its enumeration order: 144 random cases from base seeds 3001, 6007, 12007, and 24001 crossed with 8, 10, or 12 variables, ternary densities 3, 4, and 5, and binary counts 0, n/2, n, and 2n, where the 36 cases with no binary clauses form the `random-ternary-only` overhead family and the other 108 form `random-mixed`; 12 chain-embedded controls, each an F_k chain with k = n−1 forcing one sign of x, followed by random ternary clauses; and 6 contradictory-fragment controls, each an odd parity cycle of length 3, 5, or 7 followed by random ternary clauses. The corpus holds 7,920 clauses, of which 1,440 are binary. No case coincides exactly with the three earlier corpora.

## Results

| Measured quantity | Baseline | Fragment interface |
| --- | ---: | ---: |
| Completed cases | 162 | 162 |
| Unknown due to budget | 0 | 0 |
| Decided by the fragment certificate alone | none | 21 |
| Derived units | 0 | 205 |
| Residual search nodes | 1,123 | 931 |
| Fragment phases | 0 | 495,221 |
| Residual solver work | 961,854 | 750,655 |
| Total online work | 961,854 | 1,245,876 |

The fragment phases divide into 79,128 events for construction, 91,170 for components, 16,422 for the certificate, 256,126 for extraction, and 52,375 for copying the formula and appending units. Extraction alone is more than half of all phase work. The 22 wins saved 58,537 events in total; the 140 losses cost 342,559 more. The largest single win was `random-n12-t3-b24-s12007`, 2,498 against 8,282; the largest single loss was `random-n12-t3-b24-s6007`, 10,583 against 5,676.

### Results by family

| Family | Cases | SAT | Units | Fragment UNSAT | Baseline work | Fragment phases | Fragment residual | Fragment total | Better | Worse |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Random ternary-only | 36 | 25 | 0 | 0 | 253,831 | 75,720 | 253,831 | 329,551 | 0 | 36 |
| Random mixed | 108 | 29 | 193 | 15 | 600,598 | 358,251 | 437,594 | 795,845 | 16 | 92 |
| Chain-embedded | 12 | 2 | 12 | 0 | 84,399 | 52,484 | 59,230 | 111,714 | 0 | 12 |
| Contradictory fragment | 6 | 0 | 0 | 6 | 23,026 | 8,766 | 0 | 8,766 | 6 | 0 |
| All | 162 | 56 | 205 | 21 | 961,854 | 495,221 | 750,655 | 1,245,876 | 22 | 140 |

Both arms completed every case, so all 162 comparisons count. On the ternary-only family the fragment arm's residual counters equal the baseline's on every case: an empty fragment derives nothing, and the 75,720 events of phases, between 1,428 and 2,856 per case, are pure overhead that raised total work by a factor between 1.07 and 2.11.

### Results by binary count, within the random mixed family

The number of binary clauses is the treatment variable.

| Binary clauses | Cases | SAT | Units | Fragment UNSAT | Baseline work (nodes) | Fragment phases | Fragment residual (nodes) | Fragment total | Better | Worse |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| n/2 | 36 | 21 | 6 | 0 | 219,880 (280) | 92,738 | 212,482 (272) | 305,220 | 0 | 36 |
| n | 36 | 8 | 45 | 0 | 198,528 (197) | 119,997 | 164,271 (155) | 284,268 | 1 | 35 |
| 2n | 36 | 0 | 142 | 15 | 182,190 (124) | 145,516 | 60,841 (23) | 206,357 | 15 | 21 |

At n/2 binary clauses the fragment forced a literal on only 5 of 36 cases. At n it forced literals on 21 cases, and one of them, `random-n12-t3-b12-s3001`, is the single win that was not a fragment certificate: three units cut the residual from 11,131 events and 11 search nodes to 2,905 events and one node, for 3,912 events of phases. At 2n the fragment decided 15 cases outright; on the other 21 it forced 142 units and cut their residual from 112,990 events and 75 nodes to 60,841 events and 23 nodes, but spent 116,279 events of phases to do so and lost all 21.

By variable count over the 144 random cases, the fragment arm won 5 of 48 at n = 8, 7 of 48 at n = 10, and 4 of 48 at n = 12. By ternary density it won 6 of 48 at density 3 and 5 of 48 at densities 4 and 5. The artifact holds those subtotals.

### Enumeration-to-interface ratios

The [interface framing](../docs/knowledge-and-complexity.md#interfaces-free-lunches-and-the-cost-of-reading-a-clue) asks that each experiment also report how much cheaper an interface is than enumerating assignments. The ratio below divides the independent reference's enumeration work by each arm's work. It is descriptive: the reference is a research oracle outside the comparison, and the ratio is not a speedup claim.

| Family | Reference work | Reference / baseline | Reference / fragment |
| --- | ---: | ---: | ---: |
| Random ternary-only | 1,566,291 | 6.171 | 4.753 |
| Random mixed | 4,578,419 | 7.623 | 5.753 |
| Chain-embedded | 219,802 | 2.604 | 1.968 |
| Contradictory fragment | 117,930 | 5.122 | 13.453 |
| All | 6,482,442 | 6.740 | 5.203 |

On the contradictory controls the fragment arm decided each formula for about one thirteenth of enumeration and about two fifths of the baseline. Everywhere else the baseline's ratio exceeded the fragment arm's, since the baseline did less work.

### Chain-embedded controls

The forced sign of x was found on all 12 cases: the derived unit was exactly the selected sign, and the clue paths ran up to 13 vertices. Only 2 of the 12 whole formulas were satisfiable. The unit reduced residual search from 83 nodes to 60 across the family, and residual work from 84,399 to 59,230 events, but the phases cost 52,484, so the family lost on every case by between 1,248 and 4,026 events. One forced literal among eight to twelve variables removes little of a ternary search.

## What the work counter means

Every work unit increments one checked counter: a formula check, clause or container read, literal or scalar read, clause or container write, literal or scalar write, assignment, pair check, rule attempt, or search-node visit. The fragment arm's construction charges validation of the whole formula, one clause read per clause including ternary ones, and the adjacency-list writes of the implication-calibration protocol for binary clauses only. Components and certificate are charged as in that protocol. Extraction charges one rule attempt per literal query, two freshly initialized arrays per query, the depth-first search, and the returned path copies. The append phase charges the copy of the original formula and one literal read, one clause write, and one literal write per unit, as in the clue-transfer protocol. The residual is the identical validate, copy, and DPLL routine the baseline runs.

Vertex labels are `usize` values `2*(v-1)` and `2*(v-1)+1`, at most 24 per case; clause indices are positions in the original input, ternary clauses included; literals are signed 32-bit integers; counters are `u64` checked increments identical in debug and release builds. Allocator internals, loop control, arithmetic and comparison instructions, corpus generation, JSON formatting, and report construction are outside the counts.

For a formula with n variables and m₂ binary clauses, construction, components, and certificate take O(n + m₂) events, extraction O(n(n + m₂)), and the copy and appends O(L + n) in the literal count L, by the arguments in the earlier protocols. The residual DPLL has no polynomial bound. These are design statements; the measured totals on at most 12 variables neither prove nor test them. The extraction phase's per-query array initialization is visible in its cost: 69,937 events on the 56 satisfiable cases and 186,189 on the 85 unsatisfiable cases that still needed a search, where every query ran and most found nothing.

Focused correctness tests in `tests/fragment.rs` are implementation checks distinct from the 162 reported cases: all 4,096 combinations of a subset of the nine canonical width-at-most-two clauses on two variables with one of the eight ternary clauses on three variables, in both clause orders, compared with the truth-table reference; a formula whose fragment is satisfiable but whose whole is not; a formula whose fragment forces a literal the reference backbone contains; empty and malformed inputs; and budget exhaustion at each of the six phase boundaries.

## What was learned

**The delta turns positive exactly when the read is the decision.** Twenty-one of the 22 wins are fragment-unsatisfiable cases: the certificate decided the whole formula for between 985 and 2,498 events, against baselines between 1,716 and 8,282. This is the 2-SAT free lunch of the implication-calibration experiment appearing inside a harder formula. There it lost to DPLL because DPLL also finished immediately; here the ternary clauses make the baseline search, and a contradiction that lives entirely in the binary fragment is read in linear time instead. No clue was extracted on those cases, so the interface's value came from certification, not from clues.

**Forced literals cut the search but their extraction cost more than the cut.** On the 21 searched cases with 2n binary clauses the fragment's 142 units removed two thirds of the residual work and 52 of 75 search nodes, and still lost every case, because the phases cost 116,279 events against a residual saving of 52,149. Extraction is the phase to make cheaper: it is 256,126 of the 495,221 phase events, and it runs 2n depth-first searches with fresh arrays each time, most of which find no path. A literal l is forced exactly when the component of ¬l reaches the component of l, which the condensation of the strongly connected components answers for every literal in O(n + m₂) total; that replacement, not the graph itself, is where the interface's read cost lies.

**Sparse fragments rarely force anything.** At n/2 binary clauses the fragment forced literals on 5 cases and 6 literals in all; at n on 21 cases and 45 literals. A fragment too small to force literals is a pure cost, and the corpus shows the interface's yield rising with fragment size faster than its cost: phases grew from 92,738 to 145,516 events across the three binary counts while residual savings grew from 7,398 to 121,349.

**One clue among many variables does little.** The chain controls found the planted forced literal on every case and reduced search nodes by about a quarter, yet lost on every case. Clue coverage is not the quantity that decides a delta; the residual search the clue removes is.

**The framing held.** The [interface delta](../docs/knowledge-and-complexity.md#interfaces-free-lunches-and-the-cost-of-reading-a-clue) is baseline work minus build, read, and residual work. The first three experiments had negative deltas because the residual was near zero; this one had an expensive residual and still a negative delta on 140 cases, because the read cost exceeded the residual it removed. The 22 positive cases are the first in the project, and they share one feature: the interface's answer made the residual zero. The enumeration ratios record the accounted free lunch of each arm separately from that delta, as the framing asks.

## Limitations

This is a calibration of a known interface, the Aspvall, Plass, and Tarjan decision and path-based forced literals of the binary fragment, feeding a toy DPLL; it measures no novel inference rule, no evolutionary advantage, and nothing about general SAT complexity or P versus NP. The corpus is finite, constructed, and limited to 12 declared variables and clause width three so that the truth-table reference stays feasible; its pseudorandom cases are reproducible samples, not independent or representative population estimates, and no size in it is scaling evidence. The contradictory-fragment controls were built to be decided by the certificate and are reported separately for that reason. Event counts are declared operational measurements under one implementation and one cost model, not elapsed time, not memory, and not a proved bit-cost bound; the stated phase bounds are design arguments the measurements do not prove. The residual solver is a transparent toy DPLL, so neither the absolute costs nor the ratios rank competitive solvers. A fragment SAT certificate is an assignment of the fragment only. Unsatisfiable cases have no defined backbone, so the subset check on derived units applies to satisfiable cases only. Budget exhaustion would have meant unknown; none occurred.

## What this changes about the next experiment

Two levers follow, and each needs a fresh frozen protocol rather than a revision of this one. First, cheaper extraction: read every forced literal of the fragment from the component condensation in one linear pass instead of 2n searches, and measure whether the 2n-binary-clause cases, where the units already removed two thirds of the search, then repay their read. Second, an interface the solver already pays for: a solver that maintains its own implication or watch structure has charged construction already, so the marginal read cost of forced literals from that structure is the quantity to measure. Both keep this artifact and the three earlier ones unchanged as the record of where the interface delta first turned positive and why.

## Primary references

- [Aspvall, Plass, and Tarjan (1979)](https://doi.org/10.1016/0020-0190(79)90002-4): the implication-graph decision used on the fragment.
- [Williams, Gomes, and Selman, Backdoors To Typical Case Complexity](https://www.cs.cornell.edu/selman/papers/pdf/03.ijcai.backdoors.pdf): tractable sub-structure exploited by a subsolver; the binary fragment is one such sub-structure, read rather than searched.
- [De Haan, Kanj, and Szeider, Local Backbones](https://arxiv.org/abs/1304.5479): forced literals with bounded explanations; the fragment's forced literals are backbones of a tractable sub-formula.
