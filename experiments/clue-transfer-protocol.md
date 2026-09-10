# Clue transfer protocol v1

Specified before the first transfer run, 9 September 2026. This is a finite experiment protocol, not a claim about general SAT complexity. Preserve this version; a change to the experimental design needs a new version and an explicit comparison.

## Question

Can a small library of sound clues, extracted from solved examples, reduce the total work of the same deterministic solver on unseen formulas? Separate reduced branching from reduced computation. Count acquiring the library once and its application on every query.

## Learn, then freeze

The training corpus consists of the six unordered pairs of distinct clauses chosen from `(x or y)`, `(x or not y)`, `(not x or y)`, and `(not x or not y)`. Each pair is satisfiable. Enumerate its four assignments and test each of the four possible signed unit conclusions against all its solutions. Record all 24 candidate implications and the training work. Retain only entailed units. This grammar is deliberately small and informed by a known result; it does not measure open-ended discovery.

Freeze the resulting rule objects before constructing or labeling held-out cases. Apply their premises through injective signed variable renaming; the matcher must consume the frozen library. Treat equivalent rules under renaming as instances of one schema when reporting novelty. An independent truth-table check and an informal argument must establish that accepted rule instances remain sound in an arbitrary surrounding conjunction. No held-out solution set is an input to preprocessing or solving.

## Held-out corpus

Use at most 12 declared variables and the following 122 deterministic cases:

- 96 generated cases: four fixed seeds, variable counts 6/8/10/12, clause densities 2/4/6 per variable, and both pure 3-CNF and mixed 2/3-CNF. Fix the generator and seeds in source before the run.
- Eight engineered forced-literal cases, balancing literal signs relative to the solver's true-first branch order. These are mechanism checks and must not represent general performance.
- Eight parity-cycle cases with sizes 5 through 12.
- Pigeonhole instances with three pigeons/two holes and four pigeons/three holes.
- Four embedded all-sign ternary cubes, one at each declared size 6/8/10/12; these are UNSAT controls without initial binary clauses.
- Four sign-renamed or padded hidden-backbone controls based on `(x or a or b) and (x or a or not b) and (x or not a or b) and (x or not a or not b)`. They force x, although the learned binary-pair rule cannot see that clue at the root.

Record the exact input CNF and generation parameters in the artifact. Declared variables may include unused padding; report this limitation and do not interpret padded controls as scaling evidence. Ground-truth labels come from an independent truth-table evaluator, outside both tested arms, after the library is frozen.

## Comparison and costs

Both arms call the same deterministic DPLL solver: repeatedly propagate the first available unit; otherwise branch on the smallest unassigned variable occurring in the residual formula, true first. The baseline receives the original formula. The transfer arm makes one initial pass applying the frozen rule library, adds sound unit consequences, then calls the common solver. There is no repeated learned closure at internal search nodes in v1.

Each arm has the same one-million-event total budget per case. Transfer preprocessing consumes part of that budget. Resource exhaustion is unknown, never UNSAT. A wrong SAT/UNSAT answer against the independent reference invalidates the run. Record completed and unknown cases separately; a low event count for an unfinished arm is not a win.

Use checked event counters for acquisition, matching/application, and residual solving. State precisely which traversals, copies, appends, assignments, and branch events are charged. These are operational comparisons, not a proved machine-model runtime. Record independent oracle work separately from the algorithm comparison. Report library size, derived clues, search nodes, per-phase work, outcomes, and per-family totals, with better/tied/worse comparisons only where both arms complete correctly.

Show both total online work and one-time acquisition plus online work for the fixed corpus. A proposed amortization threshold must follow from observed positive net savings; do not invent a break-even point when the transfer arm is slower. Wall-clock timing is optional, supplementary evidence and is not part of the deterministic artifact.

## Interpretation

An accepted rule must have a general soundness argument; successful finite tests alone do not provide it. Its successful application on larger instances demonstrates reuse of that rule, not universal completeness. Report negative and overhead results alongside helpful cases. Do not describe the common DPLL solver as a competitive SAT baseline, claim an evolutionary advantage, or infer P = NP from this experiment.
