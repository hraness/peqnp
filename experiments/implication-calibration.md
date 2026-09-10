# The implication graph read every forced literal, and its linear-time decision still cost more than the toy solver

Completed finite experiment, 10 September 2026. On 120 two-variable-clause formulas, the standard 2-SAT implication graph recovered all 173 backbone literals of the 94 satisfiable cases, including the 24 F_k chains whose forced literal the frozen two-clause library cannot start to derive; the library found 63. The same graph's strongly-connected-component decision, with an explicit certificate for every case, cost more declared events than the deterministic DPLL baseline on 102 of the 120 cases and less on 18. On this corpus the interface delta for deciding was negative while the coverage delta was total. Every decision agreed with the independent reference, every certificate and every clue path passed a raw-clause checker, and no arm exhausted its budget.

The [protocol](implication-protocol.md) was fixed in commit `205f774` before execution. Its SHA-256, `5b957bb778acd98fa22cd40e8c4bd0f46c87ec516bea42b68736c70f44352ec2`, is embedded in `src/implication.rs` and in the artifact. The [full artifact](../artifacts/implication-calibration.json) contains the frozen rules and their acquisition cost, every input CNF with its generation parameters, the reference model count and backbone, all four arms' phase counters, every certificate and clue path, the checker's verdicts and work, and the summary and family tables. Its SHA-256 is `41d7c2f40a13bdab812bd7ae1ecbf1150791b1649b5af46bbdf4c2fea84107f7`.

## Reproduce the comparison

Use the repository's Rust 1.97.1 toolchain and Bun 1.3.14:

```sh
cargo run --locked --release -- implication artifacts/implication-calibration.json
bun run check
```

The first command rewrites the artifact deterministically; `shasum -a 256 artifacts/implication-calibration.json` must print the digest above. The second command runs the repository gate: formatting, Clippy, the test suite, the ledger checks, a check that each artifact's embedded protocol digest matches the committed protocol file, and byte-for-byte replay of all four experiment artifacts, this one included.

## What was tested

The same miner as the two clue-transfer experiments evaluated 24 candidate implications over six two-clause formulas and froze the same four unit rules before the corpus existed, for 1,000 events. No reference label, model set, or reference backbone reached any arm.

Four metered arms ran on each case, each local-library, DPLL, and graph run with a one-million-event budget:

- **local library**: the unchanged v1 root preprocessor, which copies the formula and tries the four frozen rules in both literal orders on every pair of eligible binary clauses, reporting the units it derives;
- **DPLL**: the deterministic unit-propagation/DPLL solver, returning a Boolean;
- **graph decision**: implication-graph construction with forward and reverse adjacency lists and source clause indices on every edge, two-pass depth-first strongly-connected-component computation, and an explicit certificate;
- **backbone extraction**: only after a certified SAT decision, one depth-first reachability query per literal in the order `1, -1, 2, -2, ...`, returning a simple path from `not l` to `l` for every forced literal.

The graph decision and backbone extraction share one budget in that order, so the decision cost is recorded when its certificate is complete and the extraction cost is reported separately on top of it. The protocol keeps two comparisons apart, and this report does the same. The coverage comparison sets the local library's unit list against the graph's complete clue list on the 94 completed satisfiable cases; the two procedures do not promise the same output and are not scored as if they did. The decision comparison sets the graph decision, certificate included, against DPLL's Boolean on the 120 cases where both completed. Backbone extraction is not part of the decision comparison, because the SCC decision does not need it.

A certificate is the evidence the graph arm hands out with its answer. For a satisfiable case it is one Boolean per declared variable; the artifact holds 94 of these. For an unsatisfiable case it is either the index of an empty clause (one case) or the smallest contradictory variable with two implication paths connecting its opposite literals in both directions (25 cases). A clue is a forced literal with its path and the clause indices that justify each step; the artifact holds 173. A separate checker validated all 120 certificates and all 173 clue paths directly against the raw input clauses, without rebuilding the graph or calling the SCC code, for 28,864 events. An independent truth-table enumeration labeled every case after the library was frozen: 94 satisfiable, 26 unsatisfiable, for 1,518,173 events. Acquisition, reference, and checker work are outside all four arms.

The corpus is the protocol's fixed 120 cases in its enumeration order: 24 long explanations (F_k for k in 2, 3, 5, 7, 9, 11, crossed with the sign of x and forward or reversed clause order), 24 broken explanations with the middle link removed, 12 immediate-pattern controls, 10 parity cycles for n from 3 to 12, two empty controls, and 48 unconditioned pseudorandom 2-CNFs from base seeds 2027, 8191, 131071, and 999983 crossed with 6, 8, 10, or 12 variables and densities 1, 2, and 4. All 1,563 clauses in the corpus are binary except the one empty clause. The largest work any arm spent on one case was 121,550 events (the local library on `random-n12-d4-s8191`), against 4,017 for DPLL and 9,720 for the graph arm including extraction; every case completed.

## Results

| Measured quantity | Local library | DPLL | Graph decision | Backbone extraction | Graph total |
| --- | ---: | ---: | ---: | ---: | ---: |
| Completed cases | 120 | 120 | 120 | 94 complete, 26 not defined (UNSAT) | 120 |
| Unknown due to budget | 0 | 0 | 0 | 0 | 0 |
| Derived units or clues | 156 | none | none | 173 | 173 |
| Search-node events | 0 | 429 | 2,276 | 4,964 | 7,240 |
| Work | 1,804,641 | 100,305 | 124,045 | 167,369 | 291,414 |

The graph decision's 124,045 events divide into 40,151 for construction, 67,370 for the two SCC passes, and 16,524 for certificate output. Its search-node events are depth-first vertex visits, not DPLL branches. Of the local library's 156 derived units, 63 fall on satisfiable cases and count toward coverage; the other 93 are sound entailments on unsatisfiable formulas, where the protocol defines no backbone, so they are reported here and nowhere else.

### Clue coverage on completed satisfiable cases

| Family | SAT cases | Reference backbone literals | Local library found | Graph found | Cases fully covered, local | Cases fully covered, graph |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Long explanations (F_k) | 24 | 24 | 0 | 24 | 0 | 24 |
| Broken explanations | 24 | 0 | 0 | 0 | 24 | 24 |
| Immediate-pattern controls | 12 | 42 | 42 | 42 | 12 | 12 |
| Parity cycles (even n) | 5 | 0 | 0 | 0 | 5 | 5 |
| Empty formula | 1 | 0 | 0 | 0 | 1 | 1 |
| Random 2-CNF | 28 | 107 | 21 | 107 | 9 | 28 |
| All | 94 | 173 | 63 | 173 | 51 | 94 |

The graph's clue list equaled the reference backbone on every satisfiable case, and the local library's units were a subset of it on every case, as the protocol's acceptance checks require. The library's 51 fully covered cases are the 37 with nothing to find (24 broken chains, 5 even cycles, the empty formula, and 7 random cases with an empty backbone), the 12 immediate-pattern controls, whose disjoint two-clause pairs are the library's own training grammar, and 2 random cases. Of the 21 satisfiable random cases with a nonempty backbone, the library found every literal on those 2, some but not all on 14, and none on 5. Its random coverage falls with density: 6 of 21 literals at density 1 and 15 of 86 at density 2; the 16 density-4 cases are all unsatisfiable.

### Decision cost, graph with certificate against DPLL

| Family | Both complete | Graph better | Tied | Graph worse | Graph decision work | DPLL work |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Long explanations (F_k) | 24 | 6 | 0 | 18 | 15,744 | 12,862 |
| Broken explanations | 24 | 0 | 0 | 24 | 14,712 | 8,123 |
| Immediate-pattern controls | 12 | 1 | 0 | 11 | 7,692 | 4,889 |
| Parity cycles | 10 | 1 | 0 | 9 | 12,130 | 9,815 |
| Empty controls | 2 | 0 | 0 | 2 | 27 | 8 |
| Random 2-CNF | 48 | 10 | 0 | 38 | 73,740 | 64,608 |
| All | 120 | 18 | 0 | 102 | 124,045 | 100,305 |

In aggregate the graph decision cost about 1.24 times DPLL. The 18 wins saved 7,116 events in total; the 102 losses cost 30,856 more. The largest single win was `random-n12-d2-s8191`, 1,607 against 2,763; the largest single loss was `random-n6-d4-s999983`, 1,629 against 912. Split by label, the graph decision cost 71,964 against 53,839 on the 94 satisfiable cases with 12 wins, and 52,081 against 46,466 on the 26 unsatisfiable cases with 6 wins.

### Cost by family

| Family | Cases | Local library | DPLL | Graph decision | Backbone extraction | Graph total | Reference | Checker |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Long explanations (F_k) | 24 | 72,000 | 12,862 | 15,744 | 41,616 | 57,360 | 219,800 | 3,412 |
| Broken explanations | 24 | 55,852 | 8,123 | 14,712 | 28,472 | 43,184 | 233,766 | 1,188 |
| Immediate-pattern controls | 12 | 36,414 | 4,889 | 7,692 | 11,044 | 18,736 | 123,552 | 2,866 |
| Parity cycles | 10 | 131,955 | 9,815 | 12,130 | 20,325 | 32,455 | 91,119 | 1,465 |
| Empty controls | 2 | 4 | 8 | 27 | 1 | 28 | 14 | 6 |
| Random 2-CNF | 48 | 1,508,416 | 64,608 | 73,740 | 65,911 | 139,651 | 849,922 | 19,927 |
| All | 120 | 1,804,641 | 100,305 | 124,045 | 167,369 | 291,414 | 1,518,173 | 28,864 |

The reference and checker columns are research costs outside the arms. The local library's 1,804,641 events are about 18 times DPLL's, consistent with the two earlier transfer experiments; 1,508,416 of them are on the random family, where pair enumeration meets the most binary clauses.

## What the work counter means

Every work unit increments one checked counter: a formula check, clause or container read, literal or scalar read, clause or container write, literal or scalar write, assignment, pair check, rule attempt, or search-node visit. The graph arm reports four phase snapshots of one meter. Construction charges input validation, two container writes per adjacency bucket for all 2n vertices, and for each edge two bucket reads, two edge-record writes, and four field writes; across the corpus it made 120 formula checks, 10,937 clause reads, 6,249 literal reads, 10,228 clause writes, and 12,617 literal writes. The SCC phase charges the initialized visited and component arrays, every explicit stack push and pop, every adjacency read, and one search-node event per vertex visited in the second pass: 6,864 clause reads, 39,952 literal reads, 600 clause writes, 18,084 literal writes, and 1,870 search nodes. The certificate phase charges the two component reads per variable, the copied assignment values, and on unsatisfiable cases the two path searches and the path copies: 820 clause reads, 8,532 literal reads, 444 clause writes, 6,322 literal writes, and 406 search nodes. Backbone extraction charges one rule attempt per literal query (1,452), two freshly initialized arrays of 2n cells per query, the depth-first search with its stack and predecessor writes (4,964 search nodes), and the reversed and forward copies of every returned path: 11,814 clause reads, 57,210 literal reads, 5,315 clause writes, and 86,614 literal writes. The local library's 1,804,641 events are dominated by 1,344,036 literal reads and 133,096 rule attempts; DPLL's 100,305 by 45,160 literal reads, 24,985 clause reads, and 17,566 literal writes around 429 search nodes and 1,071 assignments.

Vertex labels are `usize` values `2*(v-1)` and `2*(v-1)+1`, at most 24 per case; clause indices are `usize` positions in the retained input; literals are signed 32-bit integers; counters are `u64` checked increments identical in debug and release builds. Allocator internals, loop control, arithmetic and comparison instructions, corpus generation, JSON formatting, and report construction are outside the counts, as in the earlier experiments. Certificate data construction is inside the graph arm; formatting it as JSON is not.

For compact integer labels and adjacency lists, the standard decision and its certificate take O(n+m) word operations and storage, and the chosen all-literal depth-first extraction takes O(n(n+m)) additional word operations and may retain O(n^2) path entries. Those are stated bounds on the algorithm design. The measured totals on at most 12 variables neither prove nor test them; the extraction's per-query array initialization is visible in the 86,614 literal writes, but a finite corpus cannot establish an asymptotic statement.

Focused correctness tests in `tests/implication.rs` are implementation checks distinct from the 120 reported cost cases: all 512 subsets of the nine canonical non-tautological clauses of width at most two on two variables, compared with the truth-table reference; explicit duplicate and tautological clauses; tampered certificates and paths rejected by the raw checker; signed and order transformations; and budget exhaustion at each phase boundary.

## What was learned

**The local library is incomplete by design, and F_k is exactly the predicted gap.** The [theory note](../docs/clue-transfer-theory.md#a-future-control-with-arbitrarily-long-binary-explanations) proves that every clause of F_k is needed to entail x and that no two-clause subset entails any unit. The artifact shows that outcome on all 24 chain cases: the library derived nothing, the graph returned the single forced literal with a path from `not x` through y_1 to y_k to x, up to 13 vertices long at k = 11, and the checker validated each path against the cited clauses. The 24 broken chains, with one link removed, have no backbone, and both procedures correctly found none. The immediate-pattern controls confirmed that reuse of the library still works where its grammar applies. On the random family the library's 21 of 107 literals show that its gap is not confined to a constructed family.

**The decision loss comes from building and certifying on formulas where propagation is already almost free.** DPLL spent 429 search nodes on the whole corpus, at most 13 on one case, and at most 6 on all but 6 cases. Its unit propagation reaches the answer on these formulas with almost no branching, so there is little residual search for a linear-time decision to remove. The graph arm must first write two adjacency lists and then run two full passes before it can answer; on the three-clause F_2, that is 281 events against DPLL's 77. The 18 wins are where DPLL's fixed variable and polarity order first commits to the wrong value and must propagate before backtracking. Six are the negative-sign F_k chains at k = 7, 9, and 11 in both clause orders: there DPLL took 4 search nodes and 734, 1,100, and 1,538 events, against 3 nodes and 402, 595, and 824 on the positive-sign twins, while the graph decision cost 731, 911, and 1,091 for either sign, because a graph built from the same clauses does not depend on which value the solver tries first. Over the 24 chain cases DPLL spent 4,534 events on the positive sign and 8,328 on the negative, against 7,872 each for the graph. One win is `immediate-pattern-n12-neg`, DPLL's most branched case at 13 nodes; one is the unsatisfiable 11-cycle, 2,111 against 2,354, where the certificate alone cost 694. The 10 random wins are all at density 2 or 4 with 8 or more variables, five satisfiable and five unsatisfiable; the 16 density-1 cases, all satisfiable, went to DPLL every time, 13,136 against 7,138. At density 2 the family is close to even, 20,724 against 21,047 with 6 wins in 16.

**Complete clue extraction cost more than the decision itself and bought nothing for deciding.** Backbone extraction spent 167,369 events, more than the 124,045 of all 120 decisions and 2.3 times the 71,964 spent deciding the 94 satisfiable cases where it ran. It exceeded the decision on 87 of those 94 cases. Its all-literal design pays for every query whether or not a path exists: the 24 broken chains and the five even cycles, which have no backbone, cost 28,472 and 20,325 events of extraction for zero clues, and the 12-cycle cost 8,113 to find nothing after a 1,607-event decision. Most of the phase is the 86,614 literal writes that initialize two 2n-cell arrays for each of the 2n queries, the O(n(n+m)) term in the protocol's design statement. None of this work changes an answer: the SCC decision has already decided each formula and certified it before extraction begins, so a consumer that needs only satisfiability of a 2-CNF gains nothing from reading its full backbone.

**What this says about the interface delta.** The [interface delta](../docs/knowledge-and-complexity.md#interfaces-free-lunches-and-the-cost-of-reading-a-clue) is baseline work minus build, read, and residual work under one cost model. For deciding these cases, the graph's build cost is construction, its read cost is the SCC passes and certificate, and its residual is zero because the read is the decision. That sum exceeded the DPLL baseline on 102 of 120 cases, so the delta was negative in the same sense as the two clue-transfer experiments, and for the same structural reason: a build-and-read cost has to be repaid by residual search it removes, and on this corpus the residual was already near zero. The difference from the earlier experiments is on the coverage axis. The graph exposed 110 forced literals the library could not, every one of them checkable, including a family whose clue the library provably cannot reach. Coverage and cost stay on separate axes, as the protocol requires, and this experiment moved one all the way while leaving the other negative.

## Limitations

This is a calibration of known 2-SAT reasoning, the SCC decision of Aspvall, Plass, and Tarjan and path-based forced literals; it measures no evolutionary advantage, no novel inference rule, and nothing about general SAT complexity or P versus NP. The corpus is finite, constructed, and limited to 12 declared variables; its pseudorandom cases are reproducible samples, not independent or representative population estimates, and no size in it is scaling evidence. Event counts are declared operational measurements under one implementation and one cost model, not elapsed time, not memory, and not a proved bit-cost bound; the O(n+m) and O(n(n+m)) statements are design bounds the measurements do not prove. The residual solver is a transparent toy DPLL, so neither the absolute costs nor the ratios rank competitive solvers, and DPLL returns only a Boolean where the graph arm also constructs and outputs a certificate. The immediate-pattern controls overlap the library's training grammar and deliberately favor it; they are mechanism checks. The library's missing units are not evidence that a literal is unforced, and its output is not equivalent to complete extraction. Unsatisfiable cases have no defined backbone, so no coverage claim is made for them and the library's 93 entailed units there are not scored. Budget exhaustion would have meant unknown; none occurred.

## What this changes about the next experiment

The three completed experiments now separate three costs. Acquiring the rule library is cheap and one-time. Reading its clues through pair enumeration cost about four times the baseline; through a sorted index, about 2.8 times; and through the implication graph, about 1.24 times for a decision that also produces a certificate and, for a further 1.35 times the decision cost, every forced literal. Each interface paid its build and read cost up front and found little residual search to remove, because the formulas tested decide almost immediately under plain propagation.

Two questions follow, and both need a fresh frozen protocol rather than a revision of this one. First, whether an interface the consumer already pays for exposes clues at marginal read cost: a solver that maintains its own implication or propagation structure has already charged T_build, so the quantity to measure is the additional read cost of forced literals from that structure, not the cost of building a second graph beside it. Second, whether any interface yields a positive delta on formulas whose residual search is actually expensive. This corpus never had that property; a 3-SAT corpus with binary substructure, where the 2-CNF fragment can be read by the graph while the ternary remainder still forces branching, is the natural place to look. The artifact and both clue-transfer artifacts stay unchanged as the record that on pure 2-CNF the graph reads everything and the read still costs more than the answer.

## Primary references

- [Aspvall, Plass, and Tarjan (1979)](https://doi.org/10.1016/0020-0190(79)90002-4): the established SCC characterization and linear-time 2-SAT algorithm. The [authors' university publication record](https://collaborate.princeton.edu/en/publications/a-linear-time-algorithm-for-testing-the-truth-of-certain-quantifi/) identifies the original work.
- [Kolen (2002)](https://cdn.aaai.org/FLAIRS/2002/FLAIRS02-038.pdf): a primary 2-SAT algorithm paper describing the implication graph, SCC baseline, and forcing a literal through a path from its negation. Its incremental-search analysis also illustrates why repeated traversal needs separate accounting.
- [De Haan, Kanj, and Szeider, Local Backbones](https://arxiv.org/abs/1304.5479): bounded-clause explanations and their computational detection. The F_k family in this project illustrates the chosen local grammar's coverage limit; no new 2-SAT theorem is claimed.
