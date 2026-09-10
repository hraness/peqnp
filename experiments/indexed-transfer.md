# Indexing made clue lookup cheaper, but the clues still cost more than they saved

Completed finite experiment, 10 September 2026. On all 160 fresh formulas, the indexed preprocessor cost more total online work than running the solver with no library, so the negative result of the [first clue-transfer experiment](clue-transfer.md) stands. The index did what it was built to do: it derived exactly the same units in the same order as the v1 generic matcher, completed every case the generic matcher could not, and spent about one tenth of the generic matcher's preprocessing work. That saving was not enough. The residual search it removed was worth 116,853 events; the index cost 1,048,871 events to find it.

The [protocol](indexed-transfer-protocol.md) was fixed in commit `205f774` before execution. Its SHA-256, `8e3239abca6796e86dc4463b51bdbfb081a1f3c33e14d8bc632e31332910a043`, is embedded in the experiment code and in the artifact. The [full artifact](../artifacts/indexed-transfer.json) contains the frozen rules, setup costs, every input CNF with its generation parameters, the reference label and labeling work, all three arms' phase counters and derived units, the index counters, and the summary and family tables. Its SHA-256 is `a95c7144abe2d65beb1fa69c288034b3be4aa1492bcdfbc12d33624e5936ac62`.

## Reproduce the comparison

Use the repository's Rust 1.97.1 toolchain and Bun 1.3.14:

```sh
cargo run --locked --release -- indexed artifacts/indexed-transfer.json
bun run check
```

The first command rewrites the artifact deterministically; two consecutive runs produced byte-identical files, and `shasum -a 256 artifacts/indexed-transfer.json` must print the digest above. The second command runs the repository gate: formatting, Clippy, the test suite, the ledger checks, a check that each artifact's embedded protocol digest matches the committed protocol file, and byte-for-byte replay of all four experiment artifacts, including the two v1 artifacts this experiment leaves unchanged.

## What was tested

The same miner as v1 evaluated the same 24 candidate implications over six two-clause formulas and froze the same four unit rules, at a cost of 1,000 events. A compiler then checked each rule object against one structural schema: two binary premises share the conclusion literal, their other literals are complementary, and the shared and pivot variables are distinct. All four rules are signed variants of that schema, so they compiled to one enabled schema for 89 events (4 rule attempts, 8 clause reads, 56 literal reads, 12 pair checks, 9 scalar writes). The compiler rejects malformed or unsupported rules and enables nothing for an empty library. Neither compiler, matcher, nor solver calls a truth table.

Three arms ran on each case with one million total events each, preprocessing included:

- **baseline**: the deterministic unit-propagation/DPLL solver alone;
- **generic**: the unchanged v1 root matcher, which tries the four stored rules in both literal orders on every pair of eligible binary clauses, then the same solver;
- **indexed**: two oriented entries per eligible binary clause, an explicit metered heapsort by shared literal, pivot variable, and clause index, one linear scan for groups with both pivot signs, one witness per shared literal reduced to its earliest clause pair, a second heapsort of the witnesses by those pairs, then the same solver.

The corpus has 144 random cases from the Cartesian product of base seeds 1201, 5179, 65539, and 104729, declared variable counts 6, 8, 10, and 12, clause densities 2, 4, and 6, and pure binary, pure ternary, and mixed families, each with its own nonzero xorshift64 initial state derived from those parameters. The other 16 cases are engineered duplicate-pressure controls: one shared literal on the highest variable, paired with every other variable in both signs, repeated once or eight times. No case coincides exactly with the 122-case v1 corpus or with another new case. An independent truth-table enumeration labeled every case after the library was frozen: 83 satisfiable, 77 unsatisfiable, for 5,533,377 events outside all three arms.

Every completed answer agreed with the reference. On all 156 cases where both library arms completed, the generic and indexed arms produced identical derived-unit lists, identical processed formulas, and identical residual solver counters, as the protocol requires. The four remaining cases are not comparable because the generic arm exhausted its budget inside preprocessing.

## Results

| Measured quantity | Baseline | Generic (v1 matcher) | Indexed |
| --- | ---: | ---: | ---: |
| Correct completed decisions | 160 | 156 | 160 |
| Unknown due to budget | 0 | 4 | 0 |
| Derived units | 0 | 377 | 381 |
| Search nodes | 786 | 590 | 594 |
| Preprocessing work | 0 | 11,235,212 | 1,048,871 |
| Residual solver work | 520,161 | 395,572 | 403,308 |
| Total online work | 520,161 | 11,630,784 | 1,452,179 |

The generic arm's four unknowns are the eight-repetition duplicate controls at 10 and 12 variables; each spent its whole million-event budget inside pair matching and derived nothing. The indexed arm's four extra units and four extra search nodes come from finishing those cases. The indexed arm's total online work is about 2.8 times the baseline's. Its preprocessing is about 9 times the residual work it saved.

| Comparison, both arms complete | Cases | First better | Tied | First worse | Completed by one arm only |
| --- | ---: | ---: | ---: | ---: | --- |
| Generic versus baseline | 156 | 0 | 0 | 156 | baseline only: 4 |
| Indexed versus baseline | 160 | 0 | 0 | 160 | none |
| Indexed versus generic | 156 | 106 | 0 | 50 | indexed only: 4 |

The 50 cases where indexing was more expensive than generic matching are the 48 pure ternary cases, which contain no binary clauses and cost the indexed arm three events more each (one library flag read and two empty sort passes), and two six-variable mixed cases with only three binary clauses each, where the index cost 68 and 42 events more. The smallest saving on the other 106 cases was 117 events; the largest were the duplicate controls, where the generic matcher's quadratic pair enumeration exhausted its budget.

| Cost | Baseline | Generic | Indexed |
| --- | ---: | ---: | ---: |
| One-time mining | 0 | 1,000 | 1,000 |
| One-time compilation | 0 | 0 | 89 |
| Online total | 520,161 | 11,630,784 | 1,452,179 |
| Setup plus online | 520,161 | 11,631,784 | 1,453,268 |

No amortization against the baseline can be computed: no completed case had positive online savings over it, so no batch size repays the setup. Against the generic matcher, the 89-event compilation is smaller than the saving on any one of the 106 cases where indexing won, but that comparison only ranks two ways of applying a library that both lose to applying none.

### Results by family

| Family | Cases | Units | Baseline work | Generic work | Indexed work | Indexed versus baseline | Indexed versus generic |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| Random 2-CNF | 48 | 280 | 106,174 | 4,118,035 | 511,510 | 0 better, 48 worse | 48 better, 0 worse |
| Random 3-CNF | 48 | 0 | 251,371 | 273,835 | 273,979 | 0 better, 48 worse | 0 better, 48 worse |
| Random mixed 2/3-CNF | 48 | 85 | 135,488 | 1,088,978 | 287,347 | 0 better, 48 worse | 46 better, 2 worse |
| Duplicate-pressure, positive target | 8 | 8 | 13,564 | 3,074,968 (2 unknown) | 186,721 | 0 better, 8 worse | 6 better, 0 worse, 2 indexed-only |
| Duplicate-pressure, negative target | 8 | 8 | 13,564 | 3,074,968 (2 unknown) | 192,622 | 0 better, 8 worse | 6 better, 0 worse, 2 indexed-only |

Units are the indexed arm's derived units; the generic arm derived the same units on every case it completed. Over the 144 random cases alone, baseline work was 493,033, generic work 5,480,848, and indexed work 1,072,836, with search nodes falling from 754 to 578 in both library arms. The indexed arm derived units on 79 random cases and was cheaper than the baseline on none of them. Pure 2-CNF cases produced the most units and the largest relative penalty: the indexed arm spent 459,023 events preprocessing them to save 53,687 events of search.

The duplicate-pressure controls are mechanism and overhead controls, not representative instances, and are reported separately for that reason. Each is trivially satisfiable and forces its shared literal; every arm that completed found it. The eight-repetition controls show the intended contrast between the two matchers: at 176 clauses, the generic matcher exhausted a million events while the index spent 59,438 and 61,475 events. The baseline solved the same formulas for 4,160 events each.

## What the index did

Across the corpus the index stored 7,442 oriented entries, two for each of the 3,721 eligible binary clauses. Sorting and scanning them made 94,276 key comparisons, 11,814 group lookups, and 43,409 element swaps across both heapsorts. The largest explicit auxiliary storage on any case was 1,060 scalar cells (352 entries, one witness, one unit) on the 12-variable, eight-repetition controls; among random cases the peak was 492 cells on a 2-CNF case and 244 on a mixed case. Pure ternary cases stored nothing.

| Family | Entries | Key comparisons | Group lookups | Swaps | Peak stored cells on one case |
| --- | ---: | ---: | ---: | ---: | ---: |
| Random 2-CNF | 3,456 | 43,743 | 6,044 | 19,654 | 492 |
| Random 3-CNF | 0 | 0 | 0 | 0 | 0 |
| Random mixed 2/3-CNF | 1,682 | 18,308 | 3,098 | 7,789 | 244 |
| Duplicate-pressure, positive target | 1,152 | 16,107 | 1,336 | 8,032 | 1,060 |
| Duplicate-pressure, negative target | 1,152 | 16,118 | 1,336 | 7,934 | 1,060 |

These counters describe operations already charged in the event totals and are not added to them. The swap counter covers both the entry sort and the witness sort, since both use the same metered heapsort on three-scalar elements. Peak stored cells count the entry array, witness array, and derived-unit list only, not the copied output formula, the frozen library, scalar locals, or allocator capacity.

## What the work counter means

Every work unit increments one checked counter: a formula check, clause read, literal or scalar read, clause write, literal or scalar write, assignment, pair or key comparison, rule attempt, or search node. The indexed arm charges reading each input clause, writing each entry field, every field inspected by a key comparison plus the comparison itself, every scalar moved by a sort swap, the group and sign reads of the scan, retained witness state, witness copies, and the common v1 charges for copying the formula and appending units. The generic arm's charges are unchanged from v1, and both library arms hand the same processed formula to the same metered solver with whatever budget remains.

These are declared operational events, not elapsed time and not a proved bound on bit operations. Vector capacity and allocator internals, loop-control arithmetic, diagnostics, corpus generation, reference labeling, and JSON serialization are outside the counts, as in v1. Scalar values fit bounded machine integers here; the protocol's asymptotic statement about the preprocessor, linear plus sort cost in the number of eligible binary clauses, concerns that preprocessor alone and says nothing about the residual search.

Focused acceptance checks compare the indexed and generic arms with each other and with the independent reference on all 4,096 ordered pairs of canonical three-variable clauses and all 1,728 ordered triples of signed binary clauses on three variables, including duplicates, tautologies, and reversed literal order; on rotated and reversed formulas with several pivots per shared literal; on empty, unit, contradictory, and malformed inputs; on an empty library and on rejected rule shapes; on the charged index arrays and sort swaps; and on exact budget exhaustion at each phase boundary.

## What changed since v1 and what did not

Only the matching algorithm changed. The four rule objects, their mining cost of 1,000 events, the signed injective renaming semantics, the derived units and their order, the root-only application, the residual DPLL solver, the event categories, and the per-arm budget are all the same as in [clue-transfer-v1](clue-transfer.md), whose artifact still reproduces byte for byte. The corpus is new and was generated from parameter-specific seeds after the protocol was frozen; its numbers are therefore not directly comparable with v1's 122-case totals, where the generic arm cost 1,676,645 events against a 424,997-event baseline. On this corpus the generic arm cost 11,630,784 events against 520,161, with four exhausted budgets, mostly because the duplicate controls were built to expose its quadratic pair enumeration.

## Limitations

The corpus is small, constructed, and not a representative SAT workload; the duplicate controls in particular were engineered to favor the index over the generic matcher. Event counts are declared operational measurements under one implementation and one cost model, not elapsed time, not memory, and not a proved bit-cost bound. The residual solver is a transparent toy DPLL, so neither the absolute costs nor the ratios rank competitive solvers. The library expresses one known resolution schema, and the index changes only how that schema is looked up; nothing here bears on general SAT complexity, on P versus NP, or on any evolutionary-search advantage. A more efficient index, a stronger library, or application inside search would each be a new experiment with a fresh protocol and a fresh corpus, not a revision of this one.

## What this changes about the next experiment

Matching cost was the v1 explanation for the loss, and this experiment removes most of it without changing the outcome. At about 2.8 times baseline work on this corpus, the two-clause library does not find clues whose search cost is large enough to be worth an extra pass: each derived unit is a two-clause resolution that the solver's own branching and unit propagation also recover cheaply, which the [theory note](../docs/clue-transfer-theory.md) relates to a failed-literal test. The next question is therefore coverage rather than lookup: whether inference that a fixed two-clause pattern cannot start, such as binary implication reachability, exposes forced literals whose search cost is large enough to repay a linear-time pass. The [implication-calibration experiment](implication-calibration.md), run under its own protocol frozen in the same commit, answers the coverage half: the implication graph reads every forced literal the library misses, and its decision still costs more than the toy solver on most cases. This artifact and both v1 artifacts stay unchanged as the record of what cheap lookup alone does not buy.
