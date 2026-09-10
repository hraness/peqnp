# Fragment interface protocol v1

Specified before held-out evaluation, 10 September 2026. Experiment ID: `fragment-interface-v1`. Preserve this version after its freeze commit. The artifact is `artifacts/fragment-interface.json`, produced by CLI command `fragment`.

## Question and distinct outputs

The three completed experiments measured negative interface deltas on formulas whose residual search was already near zero. This experiment asks the question they could not: when a formula's residual search is expensive, does reading the forced literals of its binary fragment through the standard implication graph repay the cost of building and reading that graph?

Inputs are CNFs over at most 12 declared variables with clauses of width at most three. The **binary fragment** of a formula is the sub-formula of its clauses of width at most two, with their original indices. Every literal entailed by the fragment is entailed by the whole formula, and an unsatisfiable fragment makes the whole formula unsatisfiable; both facts follow because a satisfying assignment of the whole satisfies the fragment. Adding an entailed unit preserves the model set.

Two arms decide each case under one budget of 1,000,000 declared events each:

1. **Baseline:** the existing deterministic unit-propagation/DPLL solver on the original formula.
2. **Fragment interface:** build the implication graph of the binary fragment exactly as in the implication-calibration protocol, compute strongly connected components, and decide the fragment. If the fragment is unsatisfiable, return UNSAT with the fragment's certificate and no residual solving. Otherwise extract every forced literal of the fragment by the same all-literal path queries, append those units to a copy of the original formula in the order found, and run the same DPLL solver on that formula with the remaining budget. Graph construction, component computation, fragment certification, extraction, copying, and unit appends are charged to the arm before the residual solver runs.

Empty fragments enable no inference and their construction cost is still charged. No truth table, reference label, or reference backbone reaches either arm. The fragment arm uses no learned library; the implication-graph procedure is fixed algorithmic knowledge and has no acquisition step.

The comparison is total online work per case when both arms complete, reported as better, tied, and worse counts together with completion differences. Phase costs of the fragment arm are reported separately: construction, components, certificate, extraction, unit append and copy, and residual. Report the number of fragment-forced literals, the residual solver's search nodes in both arms, and, as a descriptive ratio only, the independent reference's enumeration work divided by each arm's total work per family.

## Fixed 162-case corpus

All random clauses are drawn with the xorshift64 transition `x ^= x << 13; x ^= x >> 7; x ^= x << 17` on a 64-bit unsigned state with truncating shifts. For a case with base seed s, declared variables n, ternary density t, and binary count b, the initial state is `s XOR (n << 32) XOR (t << 40) XOR (b << 48)`. These fields occupy disjoint bits for the fixed domain. Draw a clause of width w by repeating: draw `next() % n + 1` as a variable, reject it if it already occurs in the clause, and once accepted draw its sign from a fresh draw's low bit, zero meaning positive; stop after w accepted literals. Preserve draw, literal, and clause order. Clauses may repeat.

- **144 random mixed cases:** the Cartesian product of base seeds `3001, 6007, 12007, 24001`, variable counts `8, 10, 12`, ternary densities `3, 4, 5`, and binary counts `0, n/2, n, 2n`. Emit `t*n` ternary clauses first, then `b` binary clauses, from one generator. Cases with `b = 0` form the family `random-ternary-only`, an overhead control with an empty fragment; the others form `random-mixed`. Family and per-parameter subtotals are reported.
- **12 chain-embedded controls:** for n in `8, 10, 12` and ternary density t in `3, 5`, with the sign of x positive or negative: the binary chain `F_k` with `k = n - 1` over `x = 1` and `y_i = i + 1`, exactly as in the implication-calibration protocol including its sign transformation, followed by `t*n` random ternary clauses over all n variables drawn with base seed `3001` and binary count `n` for the initial-state formula. The fragment forces exactly the selected sign of x whenever the fragment is satisfiable; whether the whole formula is satisfiable is determined by the reference.
- **6 contradictory-fragment controls:** for n in `8, 10, 12` with cycle length m in `3, 5, 7` respectively and ternary density t in `3, 5`: the parity cycle `(i or next(i)) and (not i or not next(i))` for i from 1 to m with `next(i) = i mod m + 1`, followed by `t*n` random ternary clauses over all n variables drawn with base seed `6007` and binary count `2m`. Odd cycles are unsatisfiable, so the fragment arm must return UNSAT from the certificate alone.

Enumerate families in the order above, and parameters in the displayed order with seeds outermost for the random family. Reject any exact `(declared_variables, ordered_CNF)` overlap between cases or with the three earlier corpora. The declared variable count is n for every case; report the used count separately. All sizes are calibration sizes chosen so the truth-table reference stays feasible; they are not scaling evidence.

## Reference, budgets, and accounting

An independent direct truth-table evaluator enumerates all `2^n` assignments after the corpus is fixed and records the model count, the SAT/UNSAT label, and the exact backbone when the model count is positive. Its work is reported outside both arms. Every completed decision must agree with the reference. On satisfiable cases the fragment arm's derived units must be a subset of the reference backbone. On chain-embedded controls with a satisfiable fragment the derived units must contain exactly the selected sign of x from the chain. On contradictory-fragment controls the fragment arm must decide UNSAT with zero residual work. On `random-ternary-only` cases the fragment arm must derive nothing. A raw-clause checker outside both arms validates every fragment certificate and every derived unit's path; its work is reported separately.

Reuse the checked event counters and unit increments of the earlier experiments, including the graph phases' charges from the implication-calibration protocol and the copy and unit-append charges from the clue-transfer protocol. Timeout means unknown, never a decision. If the fragment phases exhaust the budget, the arm is unknown. Compare totals only when both arms complete. Do not treat an unfinished arm's smaller work count as a speedup.

For a formula with n variables, m₂ binary clauses, and L literals, the fragment phases take O(n + m₂) events for construction, components, and certificate and O(n(n + m₂)) for extraction, plus O(L + n) for the copy and appends, by the arguments stated in the earlier protocols. The residual DPLL has no polynomial bound. Measured totals do not prove these statements.

## Acceptance and reporting

A mismatch with the reference or the checker invalidates the calibration; repair implementation bugs transparently before publication, preserving the frozen design. Algorithm or corpus changes prompted by observed performance require a new protocol identity. Keep every generated case, outcome, phase counter, derived unit, certificate, and comparison. Report per-family results before any aggregate, and report the binary-count subtotals within `random-mixed`, since the fragment's size is the treatment variable. No required result is a positive delta; a negative result is reported with the same completeness as a positive one.

Focused correctness tests include: fragment extraction on every subset of the nine canonical width-at-most-two clauses on two variables combined with each of the eight ternary clauses on three variables; agreement with the reference on all cases; a formula whose fragment is satisfiable but whose whole is not; a formula whose fragment forces a literal that the reference backbone contains; empty and malformed inputs; and budget exhaustion at each phase boundary.

## Primary references

- [Aspvall, Plass, and Tarjan (1979)](https://doi.org/10.1016/0020-0190(79)90002-4): the implication-graph decision used on the fragment.
- [Williams, Gomes, and Selman, Backdoors To Typical Case Complexity](https://www.cs.cornell.edu/selman/papers/pdf/03.ijcai.backdoors.pdf): tractable sub-structure exploited by a subsolver; the binary fragment is one such sub-structure, read rather than searched.
- [De Haan, Kanj, and Szeider, Local Backbones](https://arxiv.org/abs/1304.5479): forced literals with bounded explanations; the fragment's forced literals are backbones of a tractable sub-formula.
