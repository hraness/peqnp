# A controlled experiment in reusable clues

Design and mathematical analysis, 9 September 2026. This extends the local-rule calibration with acquisition from solved examples, a frozen rule library, and transfer to larger formulas. The completed bounded pilot is recorded in [clue-transfer.json](../artifacts/clue-transfer.json); future extensions below remain proposals. The aim is to measure when additional sound inference saves total solving work, not to infer P=NP from small tests.

## What the tiny corpus can teach

Use the four non-tautological binary clauses on two distinct variables:

\[
C_{++}=x\lor y,\quad C_{+-}=x\lor\neg y,\quad
C_{-+}=\neg x\lor y,\quad C_{--}=\neg x\lor\neg y.
\]

There are six unordered pairs of distinct clauses. Each pair has exactly two satisfying assignments: each clause excludes a different one of the four assignments. Enumerate every assignment and extract only literals true in every satisfying assignment, requiring the model set to be nonempty.

| Pair | Extracted literal |
| --- | --- |
| C++ and C+- | x |
| C++ and C-+ | y |
| C++ and C-- | None |
| C+- and C-+ | None |
| C+- and C-- | not y |
| C-+ and C-- | not x |

Retain the four mined rule records as the frozen library; the preprocessor must match those records rather than secretly invoking a separately hardcoded learner. Mathematically, these four conclusions are one rule up to variable exchange and sign reversal:

\[
(a\lor b)\land(a\lor\neg b)\models a.
\]

This is a familiar resolution inference, rediscovered through finite model analysis. The legitimate result is a reproducible path from examples to a checked reusable schema. It is not novel mathematics or evidence that a six-example learner has acquired general SAT reasoning. The pilot records all 24 candidate implications, their satisfying-assignment counts and first rejection counterexamples, and the four frozen rules. The complete model sets can be reconstructed from the recorded premises and the exhaustive enumerator; they are not separately serialized.

## Why transfer is sound

**Renaming lemma.** Let a finite clause set P entail a literal l. Map each variable occurring in P or l to a signed literal on a distinct target variable, preserving complementation: σ(not v)=not σ(v). If a CNF F contains every clause of σ(P), then F entails σ(l).

**Proof.** Any satisfying assignment of F satisfies σ(P). Pulling its values back through the signed injective map gives an assignment satisfying P, hence l. The original assignment therefore satisfies σ(l). This argument applies regardless of extra clauses and variables in F. If F is unsatisfiable, entailment still holds, but do not describe its literals as empirical backbones of a nonempty solution space.

For the displayed schema, an even shorter check is available: if a were false, the two clauses would require both b and not b. Adding an entailed unit preserves the entire model set. Subsequent unit elimination preserves satisfiability, with the usual assignment restriction/extension argument. The pilot records derived units but does not serialize a proof trace for each match. A future trace mode should record the source clauses, variable map, and conclusion so a separate checker can verify each resolution step.

For this first transfer experiment, allow only syntactically matching, two-clause antecedents with distinct variables. Do not accept a merely similar neighborhood, a rule whose conclusion held only on sampled models, or a pattern match that silently drops a precondition.

## What prior research adds

The closest formal concept is a **local backbone**: a forced variable with an explanation using a bounded number of clauses. De Haan, Kanj, and Szeider also study iterative local backbones after assignments simplify the formula. Their experiments distinguish structured instances, where many backbones were local, from random instances with fewer local backbones. That motivates testing both kinds; it does not predict our rule's success rate. [Local Backbones](https://arxiv.org/abs/1304.5479)

The same two-clause inference is detected by a failed-literal test: assume not a, propagate, derive a conflict, and conclude a. Failed-literal reasoning and stronger propagation already have an established algorithmic role. Our restricted lookup is a possible way of reusing a small proof pattern, not a new logical capability absent from existing SAT methods. [Davies and Bacchus, Using More Reasoning to Improve #SAT Solving](https://www.cs.cmu.edu/~airg/readings/2011_11_16_reasoning_to_improve_sharpSAT.pdf)

Clause learning records explanations of failed search branches as new constraints. Its proof-complexity analysis demonstrates why reusing derived constraints can outperform tree-like search, while separating short proofs from a strategy that efficiently finds them. Our cross-instance schema library differs from an instance-specific learned clause: transferring the latter without its supporting premises is unsound. [Beame, Kautz, and Sabharwal](https://www.cs.cornell.edu/~sabhar/publications/learnJAIR04.pdf)

Backdoors identify variables whose assignment exposes a tractable residual problem. A strong backdoor of size k permits checking all 2^k assignments with a polynomial subsolver, whereas finding an appropriate small backdoor can itself be difficult. A backbone is a forced value; a backdoor is defined relative to what a solver can finish. They are not interchangeable. [Williams, Gomes, and Selman](https://www.cs.cornell.edu/selman/papers/pdf/03.ijcai.backdoors.pdf)

Our resulting lesson is to measure both **local explanatory coverage** and **remaining search**, rather than treating number of discovered forced literals as a complete measure of usefulness. A useful clue need not identify a forced value: it could identify a small set of variables worth branching on. If a promised family has strong backdoors of O(log N) variables that a uniform algorithm finds in polynomial time, where N is the encoded input length, enumerating their assignments and running the subsolver gives a polynomial algorithm for that family. Neither small backdoors nor their efficient discovery is established for arbitrary SAT.

## Comparison and evaluation corpus

Use one deterministic DPLL solver with unit propagation, a fixed variable ordering, and a fixed polarity ordering in both conditions. Compare its ordinary run against the same solver after a frozen clue preprocessor. The approved v1 protocol applies one root pass before calling that common solver. Repeated clue closure, or application at every search node, is a different intervention and must be labeled and metered separately. The baseline already contains algorithmic knowledge; describe it as “without this additional library,” not “without knowledge.”

The approved pilot has 122 cases: 96 unconditioned pseudorandom formulas (pure ternary or mixed binary/ternary, four seeds, n in {6,8,10,12}, and clause densities {2,4,6}); eight engineered forced-literal cases balanced across signs; eight XOR cycles with n=5 through 12; pigeonhole instances 3→2 and 4→3; four all-sign ternary UNSAT cubes; and four hidden-backbone controls. Both arms have one million total declared events per instance, including preprocessing in the treatment arm. Mining is a separately reported acquisition cost. The [protocol](../experiments/clue-transfer-protocol.md) was fixed in commit `4931f7d` before execution, with SHA-256 `5a4e14e2f637e6bc6ee3cb691f76d84e556d14d4eb126e55ecd387d59e89d561`; this was not external preregistration.

The strata serve different purposes:

1. **Mechanism controls:** sign-balanced disjoint occurrences of the learned pattern on larger variable labels. These are deliberately favorable transfer checks, not evidence of typical-case advantage. Focused matcher tests also exercise reordered literals and signed renaming.
2. **Unconditioned samples:** mixed binary/ternary CNFs with distinct variables per clause, several variable counts and clause densities, and fixed seeds. Do not retain only instances where the library helps. Determine SAT/UNSAT labels after generation. The small deterministic sample is not representative; sharing a seed across densities also makes some formulas prefixes of larger ones.
3. **No-match overhead controls:** pure ternary formulas without input units, and opposite binary pairs with no backbone. If preprocessing is root-only, pure ternary cases cannot initially match this library.
4. **Adversarial coverage controls:** formulas with hidden forced literals requiring more than the learned two-clause explanation, conflicting implications, and small parity or pigeonhole encodings. Keep exact encodings and avoid treating a named family as a proved hard case for the particular implementation.

A concrete missed-clue control is

\[
G=(x\lor a\lor b)\land(x\lor a\lor\neg b)
\land(x\lor\neg a\lor b)\land(x\lor\neg a\lor\neg b).
\]

G is satisfiable and forces x, but contains no unit or binary clauses. The proposed root preprocessor cannot find x. Failure to match therefore does not establish absence of implicit clues. Use an independent truth-table reference for evaluation sizes small enough to exhaustively check. Report SAT, UNSAT, and unknown separately; an exhausted budget is not a negative answer.

Balance signed variable renamings and orderings across matched baseline/treatment pairs: otherwise fixed DPLL branch preferences can manufacture apparent gains. Include both decision outcomes and publish every generated case, including regressions. Give per-family results before any aggregate, and do not pool favorable synthetic motifs into a headline “general SAT speedup.”

## Count acquisition, application, and residual work

Record three costs separately: one-time mining and validation; per-instance matching and preprocessing; residual DPLL. Track clause/literal scans and copies, pattern comparisons, unit assignments, representation growth, and branch nodes. Oracle verification and artifact construction are additional research costs, outside the candidate solver. Report warm-library cost and cold total acquisition-plus-batch cost; divide acquisition by the actual batch size only when presenting an explicitly amortized figure.

Fewer branch nodes alone do not establish faster solving. A matcher that scans all clause pairs can spend more than the search it avoids. With m clauses, straightforward pair matching uses O(m²) constant-width comparisons per pass. The pilot has one pass. In a future iterative version, if each successful pass eliminates a previously unassigned variable and no clauses grow, there are at most n successful passes plus a final scan. This gives polynomial preprocessing for that specific implementation, not a bound on residual DPLL. If adding rather than immediately simplifying units, include up to 2n distinct signed units and stop on contradiction or no progress. An implementation must justify these preconditions before claiming the bound.

Use the same declared operation categories and limits for both paths. Keep event vectors and measured wall time distinct from a proved bit-operation count. Report regressions, timeouts, rule matches, assignments eliminated, and total work by family. Do not tune the library or dataset against the evaluation outcomes; a change requires a new experiment identity and fresh evaluation set.

The pilot found 127 derived units and reduced total search nodes from 738 to 620, but increased declared online work from 424,997 to 1,676,645 events. Every one of the 122 cases cost more after adding preprocessing; all outcomes agreed with the reference and neither arm exhausted its budget. Mining cost another 1,000 events. These observations show sound reuse with reduced search and an unfavorable total cost for this implementation and corpus. They provide no positive amortization threshold.

A stronger future question is whether broader bounded explanations expose a cheaply recognizable residual class often enough to repay their extraction cost. The negative result also makes a cheap indexed matcher worth testing against a fresh, separately specified corpus; it does not justify changing the completed v1 result.

## A future control with arbitrarily long binary explanations

The following deduction is outside the frozen 122-case protocol. It was later tested under the [implication-calibration protocol](../experiments/implication-protocol.md); the [completed report](../experiments/implication-calibration.md) confirms that the two-clause library derives nothing on any F_k case while the implication graph recovers the forced literal on all of them. The [fragment-interface experiment](../experiments/fragment-interface.md) then embedded F_k among random ternary clauses: the forced literal was recovered on all 12 cases and still did not repay its extraction cost, because one clue among n variables removes little of a ternary search. For k≥2 and distinct variables, set

\[
F_k=(x\lor y_1)\land\bigwedge_{i=1}^{k-1}(\neg y_i\lor y_{i+1})\land(\neg y_k\lor x).
\]

This satisfiable 2-CNF has k+1 clauses and forces x: setting x=false propagates y_1 through y_k to true, contradicting the last clause. Every proper clause subset allows x=false. If the first or last clause is removed, choose all y values false or true respectively; if an internal link i is removed, choose y_1 through y_i true and the remaining y values false. These witnesses also satisfy any smaller subset. Thus all k+1 clauses are needed to entail x. With x=true, both the all-false and all-true y assignments satisfy F_k, so no y literal is entailed by F_k or any subset of its clauses. No two-clause subset can entail any unit, and the two-clause library cannot initiate an inference.

The implication path not x→y_1→…→y_k→x exposes the missing clue. A future experiment can compare binary implication reachability or binary-clause derivation against the tiny unit-only library. General 2-SAT already has a linear-time implication-graph algorithm using strongly connected components; applying it to F_k together with not x also certifies the forced value. [Aspvall, Plass, and Tarjan](https://doi.org/10.1016/0020-0190(79)90002-4) This is a limit of our chosen explanation size, not hardness of the underlying family. Keep an established polynomial 2-SAT solver as the relevant future baseline.
