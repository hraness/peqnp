# Implication calibration protocol v1

Specified before held-out evaluation, 9 September 2026. Experiment ID: `implication-calibration-v1`. Preserve this version after its freeze commit. The artifact is `artifacts/implication-calibration.json`, produced by CLI command `implication`.

## Questions and distinct outputs

Can composing binary implications recover forced literals missed by the frozen two-clause unit library? What does complete clue discovery cost beyond deciding satisfiability?

Keep two comparisons separate:

1. **Clue coverage:** the original frozen-library root preprocessor versus implication-path extraction of every backbone literal of a satisfiable 2-CNF. The local library may be incomplete. Coverage and costs are reported together; these procedures do not promise identical output sets.
2. **Decision:** the existing deterministic DPLL baseline versus a standard strongly-connected-component (SCC) 2-SAT procedure. SCC also returns a checkable decision certificate, whose construction is included in its cost. DPLL retains its existing Boolean output. Do not describe extra backbone enumeration as necessary for the SCC decision.

This is a calibration of known 2-SAT reasoning. It measures no evolutionary advantage, novelty of the inference rules, or result on general SAT complexity.

## Algorithms and proof obligations

Inputs are finite CNFs over at most 12 declared variables, with clauses of width at most two. Empty clauses, units, duplicate literals, repeated clauses, and tautologies have their usual Boolean meaning. Literal zero, the minimum signed integer, out-of-range variables, and wider clauses are invalid inputs. Empty formulas are satisfiable. Exact input clauses and declared/used variable counts are retained.

Mine the same four rule objects from the six two-variable training pairs before constructing the evaluation corpus. Record the actual library and acquisition counter. Use the original generic root preprocessor and its measured output without changing its semantics or removing its copying costs. Its failure to return a unit is not evidence that the unit is unforced. On UNSAT instances, do not report a solution-set backbone or a backbone coverage percentage.

Construct a graph with vertices `2*(v-1)` for positive variable v and `2*(v-1)+1` for its negation. Preserve input order. Clause `(a or b)` contributes edges `not a -> b` and `not b -> a`, including duplicate edges. A unit `(a)` contributes `not a -> a`. Keep the source clause index on each edge and build the reverse adjacency lists for SCC computation. Record the first empty-clause index if present.

Use two-pass depth-first SCC computation with explicit stacks, ascending initial vertex order, adjacency insertion order, and reverse finishing order for the second pass. Without an empty clause, the formula is UNSAT exactly when some variable and its negation occupy the same component. Otherwise component topological order gives a satisfying assignment. A SAT certificate records one Boolean value per declared variable. An UNSAT certificate records either an empty-clause index or the smallest contradictory variable and two implication paths connecting its opposite literals in both directions. Charge all certificate searches and output construction to decision cost.

Only after a certified SAT decision, query literals in order `1,-1,2,-2,...`. A literal l is forced exactly when the graph contains a path `not l -> l`. Use depth-first reachability in adjacency insertion order, stopping when its target is discovered; keep predecessor vertices and clause indices. Return a simple path for every discovered literal. A separate checker verifies each path directly against its cited raw clauses and verifies decision certificates without rebuilding the candidate graph or calling the SCC implementation.

The general argument must establish: edge soundness; SCC decision soundness and completeness; component-order assignment correctness; forced-literal equivalence for satisfiable 2-CNF; certificate validity; termination; and construction, traversal, copying, and output-size bounds. For the forced-literal converse, adding unit `not l` adds edge `l -> not l`; without an existing return path it cannot merge SCCs and cannot create a contradictory component. Informal reviewed arguments remain distinct from kernel proof receipts.

## Fixed 120-case corpus

- **24 long explanations:** for k in `{2,3,5,7,9,11}`, use distinct variables x=1 and y_i=i+1 and `F_k=(x or y_1) and AND_{i=1}^{k-1}(not y_i or y_{i+1}) and (not y_k or x)`. Cross two signs for x with forward/reversed clause order. Negate every occurrence of x for the negative variant; keep each clause's literal order. These formulas are SAT and force exactly the selected sign of x. Their two-clause subsets cannot entail a unit.
- **24 broken explanations:** from each corresponding long-explanation case, remove the middle link with i=`floor(k/2)` before applying its sign and order variant. These formulas are SAT with no backbone literals.
- **12 immediate-pattern controls:** for n in `{2,4,6,8,10,12}` and sign s in `{+1,-1}`, use disjoint pairs `(s*(2i-1) or 2i)` and `(s*(2i-1) or not 2i)` for i=1 through n/2. These deliberately favor the local library and force exactly the signed odd variables.
- **10 parity cycles:** n=3 through 12, with `(i or next(i)) and (not i or not next(i))`, where `next(i)=i mod n+1`. Odd cycles are UNSAT; even cycles are SAT without backbones.
- **Two empty controls:** the empty formula and a formula containing one empty clause, both declaring zero variables.
- **48 unconditioned pseudorandom 2-CNFs:** cross base seeds `{2027,8191,131071,999983}`, n in `{6,8,10,12}`, and densities d in `{1,2,4}`. Use n*d binary clauses, no rejection based on outcomes or helpfulness. Effective seed is `base_seed XOR (n << 32) XOR (d << 16)` in u64. For each clause, draw a variable as `next() mod n+1`, then a sign from a fresh draw's low bit (zero means positive); repeat for the second variable until it differs from the first, drawing its sign only after accepting the distinct variable. Repeated clauses are allowed. `next()` uses xorshift64: `x ^= x << 13; x ^= x >> 7; x ^= x << 17`, with ordinary fixed-width unsigned bit shifts and XOR. Start a fresh generator for every parameter tuple. Record base/effective seeds and parameters. These are reproducible samples, not independent or representative population estimates.

Enumerate families in the order above, parameters in the displayed order, positive sign before negative, and forward order before reversed. All sizes are calibration cases, not scaling evidence. Training-overlap immediate controls are explicitly mechanism checks.

## Reference, budgets, and accounting

An independent direct truth-table evaluator enumerates all assignments after the library and inputs are fixed. It records the model count, SAT/UNSAT label, and the exact backbone set when the model count is positive. The reference and raw certificate checker are outside all compared algorithms; report their declared work separately. No labels, model sets, or reference backbones enter preprocessing, SCC, or reachability.

Each local-library, DPLL, and graph arm receives a one-million-event budget per case. Graph construction, SCC analysis, decision-certificate generation, and subsequent backbone extraction share that graph budget in order. Record the decision cost at the end of its completed certificate, then separately report the additional backbone work and total extraction cost. If a later backbone phase exhausts its budget, retain a completed decision but report backbone status unknown. If decision certification exhausts the budget, report decision unknown. Never report an incomplete clue set as complete or treat an exhausted solver as UNSAT.

Reuse checked event counters with unit increments: formula checks, clause/container reads and writes, literal/scalar reads and writes, assignments, pair checks, rule attempts, and search-node visits. Include input validation, graph/reverse-graph storage, initialized arrays, stack/predecessor/component storage, traversals, and copied/serialized certificate values. Report the event vectors and phase totals, including one-time library acquisition. These count declared logical operations; allocator internals, loop-control instructions, arithmetic/comparison instructions, input generation, JSON formatting, and report construction are excluded. Certificate data construction belongs inside the arm; formatting that data as JSON does not.

For compact integer labels and adjacency lists, the standard graph decision and its certificate take O(n+m) word operations and storage. The chosen all-literal DFS implementation takes O(n(n+m)) additional word operations and may retain O(n^2) path entries. Explain label widths and encoding costs separately. The bounded Rust implementation and measured event totals do not prove those general asymptotic statements by testing.

## Acceptance and reporting

Every completed decision must agree with the independent reference. Every completed SAT certificate and reported implication path must pass the raw-clause checker. On completed SAT cases, graph backbones must equal the reference set and local units must be a subset. The F_k and broken-chain controls must exhibit the stated distinction; immediate-pattern controls must validate reuse. Report UNSAT backbone status as not defined, not as an empty solution-set claim.

Keep every generated case, outcome, timeout, certificate, clue, and cost. Compare completed decision costs only when both decisions are correct; graph's certificate output must remain explicit in that comparison. Report clue coverage and cost without treating the incomplete local task as output-equivalent to complete extraction. No required result is a speedup. A mismatch invalidates the calibration; repair implementation bugs transparently before publication, preserving the frozen design. Algorithm or corpus changes prompted by observed performance require a new protocol identity.

Focused correctness tests include all 512 subsets of the nine canonical non-tautological clauses of width at most two on two variables (empty clause, four units, four binary clauses), explicit duplicate/tautological clauses, raw-certificate tampering, signed/order transformations, and budget exhaustion at phase boundaries. These are implementation checks, distinct from the 120 reported cost cases.

## Primary references

- [Aspvall, Plass, and Tarjan (1979)](https://doi.org/10.1016/0020-0190(79)90002-4): the established SCC characterization and linear-time 2-SAT algorithm. The [authors' university publication record](https://collaborate.princeton.edu/en/publications/a-linear-time-algorithm-for-testing-the-truth-of-certain-quantifi/) identifies the original work.
- [Kolen (2002)](https://cdn.aaai.org/FLAIRS/2002/FLAIRS02-038.pdf): a primary 2-SAT algorithm paper describing the implication graph, SCC baseline, and forcing a literal through a path from its negation. Its incremental-search analysis also illustrates why repeated traversal needs separate accounting.
- [De Haan, Kanj, and Szeider, Local Backbones](https://arxiv.org/abs/1304.5479): bounded-clause explanations and their computational detection. The F_k family in this project illustrates the chosen local grammar's coverage limit; no new 2-SAT theorem is claimed.
