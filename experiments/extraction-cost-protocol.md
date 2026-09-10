# Extraction cost protocol v1

Specified before held-out evaluation, 10 September 2026. Experiment ID: `extraction-cost-v1`. Preserve this version after its freeze commit. The artifact is `artifacts/extraction-cost.json`, produced by CLI command `extraction`.

## Question and distinct outputs

The fragment-interface experiment found that extracting every forced literal of the binary fragment by one depth-first search per literal, with two freshly initialized arrays per query, was the largest phase cost and the reason forced literals did not repay their read. This experiment asks whether a cheaper complete extraction procedure changes that outcome, holding everything else fixed.

Three arms decide each case under one budget of 1,000,000 declared events each:

1. **Baseline:** the existing deterministic unit-propagation/DPLL solver on the original formula.
2. **Per-literal extraction:** the unchanged fragment-interface-v1 arm: fragment graph, components, fragment certificate, per-literal depth-first extraction, copy and unit appends in the order `1, -1, 2, -2, ...`, then the residual solver.
3. **Settled extraction:** identical to arm 2 in every phase except extraction, which uses the procedure below. Its forced literals are appended in the same literal order, so when both arms complete their fragment phases the processed formulas must be identical and the residual counters must be identical; the difference between arms 2 and 3 is extraction cost alone. A disagreement invalidates the run.

Compare arm 3 against the baseline and against arm 2 on total online work when both complete, as better, tied, and worse counts with completion differences. Report the extraction phase of arms 2 and 3 separately, the number of forced literals, the number of searches each procedure ran, and, as a descriptive ratio only, the reference's enumeration work divided by each arm's total.

## Settled extraction

No algorithm is known that reads every forced literal of a 2-CNF in time linear in its size. [Buss, Kullmann, and Vassilevska Williams](https://mathweb.ucsd.edu/~sbuss/ResearchWeb/DualDFS/DualDFS_SAT.pdf) prove that a subquadratic algorithm for the complete 2-CNF backbone would improve the best known algorithms for detecting k-cycles in directed graphs, and that a near-linear one would resolve an open problem for Max-k-SAT. The procedure below therefore has the same worst-case bound as per-literal search. Its hypothesis is that three savings measured in this event model are large on formulas of this size: one initialization instead of one per query, queries answered by inheritance without a search, and queries settled by an earlier failed search.

Inputs are the fragment implication graph with forward and reverse adjacency lists carrying original clause indices, the component index of every vertex as computed by the shared components phase, and the declared variable count n. The components phase numbers components in the order the second depth-first pass discovers them, which is a topological order of the condensation with sources first. Vertices are `2*(v-1)` for positive literal v and `2*(v-1)+1` for its negation; the dual of vertex x is `x XOR 1`.

**State, allocated once.** Four arrays of `2n` cells, each initialized with one container write and `2n` scalar writes: `status` with values unknown, bad, or good; `stamp` with value 0; `predecessor` and `predecessor_clause` with a sentinel. A query counter starting at 0. A list of recorded paths indexed by vertex, one per bad vertex. A visited list reused across searches.

A vertex u is **bad** when a path from u to its dual exists, in which case the literal of the dual is forced true; it is **good** when no such path exists. Every vertex is unknown at the start.

**Order.** Visit vertices grouped by component from the highest component index to the lowest, and within a component in ascending vertex index. Each visit charges one scalar read of `status`; a vertex that is not unknown is skipped.

**Inheritance.** For an unknown vertex u, read its forward adjacency in insertion order, charging one container read for the list and one scalar read per entry's target and one per target's status. At the first successor v whose status is bad, stop: u is bad. Its path is u, then the recorded path of v from v to the dual of v, then the dual of u; the final edge from the dual of v to the dual of u is the dual of the edge u to v and cites the same clause index. Reduce the path to a simple path as described below, record it, set `status[u]` to bad and `status[dual(u)]` to good, charging one scalar write each, and continue with the next vertex. If no successor is bad, search.

**Search.** Increment the query counter and use it as the current stamp. Push u with `stamp[u]` set to the current stamp and `predecessor[u]` set to u; charge two scalar writes per push, one container write for the stack, and one scalar write for appending u to the visited list. Pop in the order of the existing depth-first path search: while the stack is nonempty, read the top entry's vertex and next-edge index, charging two scalar reads and one container read; if the edge index equals the adjacency length, pop, charging two scalar reads; otherwise advance the index, charging one scalar write, read the edge, charging two scalar reads, and read the target's stamp, charging one scalar read. A target whose stamp equals the current stamp is skipped. Otherwise charge one search-node event, set the target's stamp, predecessor, and predecessor clause, charging three scalar writes, and append it to the visited list, charging one scalar write. Read the target's status, charging one scalar read. If the target is the dual of u, stop: u is bad and its path is the tree path from u to the dual, reconstructed by following predecessors as the existing path search does and charged identically. If the target's status is bad, stop: u is bad and its path is the tree path from u to the target, then the recorded path of the target, then the duals of the tree path's edges in reverse order, each citing the clause of the edge it mirrors; reduce it to a simple path. Otherwise push the target with edge index 0, charging two scalar writes.

If the search exhausts the stack, u is good, and so is every vertex on the visited list: for each listed vertex w, if u reached w and w were bad, then u would reach the dual of w and, by the dual of the path from u to w, the dual of u, so u would be bad. Set `status[w]` to good for every listed vertex, charging one scalar read and one scalar write per entry, and clear the list, charging one container write.

When u is bad by either route, record its path, set `status[u]` to bad and `status[dual(u)]` to good, and clear the visited list. Correctness of setting the dual good: if both u and its dual were bad, u and its dual would lie in one component and the fragment certificate would have refuted the fragment before extraction ran.

**Simple paths.** A constructed path is reduced by one left-to-right scan with a position array of `2n` cells initialized once alongside the others: for each node, charge one scalar read of its recorded position; if the node has a position from this scan, truncate the path to that position, charging one container write; then record the current position, charging one scalar write. Clause indices are truncated with their nodes. The scan's own stamp distinguishes positions from earlier scans, charged like the search stamp.

**Output.** After every vertex is settled, emit clues in the order `1, -1, 2, -2, ...`: for each literal l, read `status[dual(l)]`, charging one scalar read; if bad, emit the clue with literal l and the recorded path from `dual(l)` to l, charging one container write and one scalar write. Append the units to the copied formula in that order, exactly as the per-literal arm does.

**Soundness and completeness.** A vertex is marked bad only with a path from it to its dual, which the raw checker validates; the literal of the dual is therefore entailed by the fragment. A forced literal l has a path from `dual(l)` to l; `dual(l)` is visited, and is either bad already, bad by inheritance, or searched, and the search from `dual(l)` reaches l or a bad vertex, so `dual(l)` is marked bad. Good marks are set only for vertices proved good by the argument above.

**Bound.** At most `2n` searches each visit at most `2n` vertices and `2m₂` edges, so extraction takes O(n(n + m₂)) events plus O(n) initialization, the same worst case as per-literal search; the reduction to simple paths is linear in the constructed path. The three hypothesized savings are measured, not proved.

## Fixed corpus

Primary evaluation uses fresh cases from the fragment-interface generator with new base seeds; the fragment-interface-v1 corpus is re-run as a secondary matched comparison and reported separately.

All random clauses use the xorshift64 transition `x ^= x << 13; x ^= x >> 7; x ^= x << 17` on a 64-bit unsigned state with truncating shifts. For base seed s, declared variables n, ternary density t, and binary count b, the initial state is `s XOR (n << 32) XOR (t << 40) XOR (b << 48)`. Draw a clause of width w by repeating: draw `next() % n + 1` as a variable, reject it if it already occurs in the clause, and once accepted draw its sign from a fresh draw's low bit, zero meaning positive; stop after w accepted literals. Emit `t*n` ternary clauses first, then `b` binary clauses, from one generator. Clauses may repeat.

- **Primary, 144 random mixed cases:** base seeds `4001, 8009, 16001, 32003`, variable counts `8, 10, 12`, ternary densities `3, 4, 5`, and binary counts `n, 2n, 3n, 4n`. Larger fragments than before, because forced literals rose with fragment size; no ternary-only family, because the empty-fragment overhead is already measured.
- **Primary, 24 fragment-only cases:** base seeds `4001, 8009, 16001, 32003`, variable counts `8, 10, 12`, ternary density `0`, binary counts `n, 2n`. The fragment is the whole formula, so the residual solver receives a 2-CNF with its forced literals appended; these isolate extraction cost where the certificate or the units decide most of the work.
- **Primary, 8 ladder controls:** for `(k, j)` in `(2, 6), (3, 9), (5, 7), (1, 11)` and the sign of the ladder positive or negative: variables `y_1..y_k` are `1..k` and `x_1..x_j` are `k+1..k+j`, so `n = k + j`; clauses are `F_k` on x_1 with the y chain, `(x_1 or y_1)`, `(not y_i or y_{i+1})` for i from 1 to k-1, and `(not y_k or x_1)`, followed by `(not x_i or x_{i+1})` for i from 1 to j-1. The negative variant negates every occurrence of every x variable. All j ladder literals are forced with explanations of growing length, and no y literal is forced. Ternary density is 0.
- **Secondary, the 162 fragment-interface-v1 cases:** regenerated by that experiment's frozen generator, identified by their v1 ids with the prefix `v1:`. Arm 2 on these cases must reproduce the fragment-interface-v1 artifact's per-case counters exactly.

Enumerate families in the order above, seeds outermost, then n, t, b; ladders by the listed pairs, positive before negative. Reject exact `(declared_variables, ordered_CNF)` overlap between primary cases or with the four earlier corpora; the secondary cases are the v1 corpus by construction. All sizes are calibration sizes chosen so the truth-table reference stays feasible; they are not scaling evidence.

## Reference, budgets, and accounting

An independent direct truth-table evaluator enumerates all `2^n` assignments after the corpus is fixed and records the model count, the SAT/UNSAT label, and the exact backbone when the model count is positive. Its work is outside all arms. Every completed decision must agree with the reference. On satisfiable cases each fragment arm's derived units must be a subset of the reference backbone, and arms 2 and 3 must derive the same set. A raw-clause checker outside all arms validates every fragment certificate and every clue path of both fragment arms; its work is reported separately.

Reuse the checked event counters and unit increments of the earlier experiments. Arm 3's extraction charges every array initialization, stamp read and write, stack operation, adjacency read, and path copy exactly as the procedure states. Timeout means unknown, never a decision. Compare totals only when both arms complete. Do not treat an unfinished arm's smaller work count as a speedup.

For a formula with n variables and m₂ binary clauses, both extraction procedures have worst-case cost O(n(n + m₂)) events; the settled procedure adds O(n) initialization and O(n) output work and may perform fewer than `2n` searches. No linear bound is claimed for either. The residual DPLL has no polynomial bound. Measured totals do not prove these statements.

## Acceptance and reporting

A mismatch with the reference or the checker, or a disagreement between arms 2 and 3 on derived units, processed formula, or residual counters when both complete, invalidates the calibration; repair implementation bugs transparently before publication, preserving the frozen design. Algorithm or corpus changes prompted by observed performance require a new protocol identity. Keep every generated case, outcome, phase counter, derived unit, certificate, path, search count, and comparison. Report primary families before the secondary matched comparison and before any aggregate; report the binary-count subtotals within the mixed family. No required result is a positive delta.

Focused correctness tests include: both extraction procedures on every subset of the nine canonical width-at-most-two clauses on two variables, and on every such subset combined with each of the eight ternary clauses on three variables, requiring identical derived-unit sets and checker-valid paths; the F_k chains for k from 2 to 11; the odd and even parity cycles; formulas with several forced literals sharing one explanation; empty and malformed inputs; and budget exhaustion at each phase boundary of arm 3.

## Primary references

- [Aspvall, Plass, and Tarjan (1979)](https://doi.org/10.1016/0020-0190(79)90002-4): the implication-graph decision and component structure used by both extraction procedures.
- [Buss, Kullmann, and Vassilevska Williams, Dual Depth First Search for Binary Clause Reasoning (2024)](https://mathweb.ucsd.edu/~sbuss/ResearchWeb/DualDFS/DualDFS_SAT.pdf): the complete 2-CNF backbone problem, its reduction from k-cycle detection, and the failed-literal characterization of forced literals; the settled procedure here is a simpler scheme than their DualDFS and inherits none of its guarantees.
- [Heule, Järvisalo, and Biere, Efficient CNF Simplification Based on Binary Implication Graphs (2011)](https://www.cs.utexas.edu/~marijn/publications/unhiding.pdf): failed literals on the binary implication graph and the conjecture that their fixpoint computation is at least quadratic.
