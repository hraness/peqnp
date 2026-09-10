# A software research strategy for P = NP

Prepared 9 September 2026. Status: research program. This proposal combines analysis of constraint-pruning ideas, a bounded primary-source literature search, and proposed engineering choices. See the README for current implementation status. No P-versus-NP proof or experimental breakthrough is claimed. Clay still lists the problem as unsolved. [Clay status](https://www.claymath.org/millennium/p-vs-np/)

The recommendation is to build a small laboratory for discovering algorithms and lemmas: a typed Lisp-style language implemented in Rust, counterexample-guided search, several independent populations, and a separate route to mathematical proof. Begin with a bounded pilot that establishes whether this method produces useful, checked discoveries more efficiently than simpler alternatives.

P = NP can guide the search. It cannot become an assumption inside the argument that supposedly establishes P = NP. Every promoted result must identify its assumptions, and the final claim must stand without assuming the desired conclusion.

## What would count as success

The constructive target is one fixed deterministic algorithm that correctly decides every instance of an NP-complete problem such as 3-SAT, with a proven polynomial worst-case bound in the encoded input length. A sufficiently precise target is:

\[
\exists A,C,k\;\forall F:\quad
A(F)=\operatorname{SAT}(F)\quad\text{and}\quad
T_A(F)\le C(|F|+1)^k.
\]

Here A is a finite program, C > 0 and k in the natural numbers are fixed constants, and F ranges over valid finite formula encodings. The runtime accounts for bit operations, intermediate representation sizes, and every subroutine. It must finish on both satisfiable and unsatisfiable inputs; specify malformed-input rejection separately. SAT decision also gives witness search through polynomially many variable-fixing queries. These are the formal targets established by the standard definitions and reductions. [Cook's official statement](https://www.claymath.org/wp-content/uploads/2022/06/pvsnp.pdf)

Fast tests at many finite sizes, a separate optimized program for each size, a randomized expected-time result, or a polynomial number of calls to an unaccounted SAT oracle would establish different things. Even a correct polynomial algorithm could have impractical constants or degree. The research process may use enormous search resources; the algorithm it discovers must meet the stated bound.

## Constraint pruning and preserving the problem

An originating problem sketch extends Clay's dormitory example with constraint pruning and forced pairings. The following analysis makes those ideas self-contained and distinguishes the two problems involved.

The central idea is to prioritize constraints that expose forced choices or discard large parts of the search space. That suggests research on sound propagation, inference rules, structural decompositions, and useful intermediate representations. The missing theorem is that a sufficient set of such clues can always be obtained and used within a polynomial total cost.

There are four specific issues to carry into the experiment design:

1. **Selection becomes pairing.** The original problem chooses k of n students so that no incompatible pair occurs anywhere in the chosen group. This is Independent Set on the incompatibility graph. The pairing variant moves toward pairing already chosen students, with incompatibility applied within each room. That is perfect matching in the compatibility graph, which is already polynomial-time solvable. If incompatibility remains global, pairing does not remove it. For example, students 1 and 2 can occupy different compatible pairs while still violating the original condition. [Karp's reductions](https://www.cs.umd.edu/~gasarch/BLOGPAPERS/Karp.pdf), [Edmonds's matching algorithm](https://doi.org/10.4153/CJM-1965-045-4)
2. **The forced pair has a precondition.** If all 100 students must be paired, and student 1's only compatible partner is 100, every valid perfect matching includes that pair. This does not force either student into the original selected subset. Nor does it establish that a full matching exists.
3. **Counting needs an explicit convention.** For 100 distinct students in 50 unlabeled pairs, the number of pairings is 100!/(2^50 50!). Fixing one particular pair leaves 98!/(2^49 49!), a factor of 99 fewer. For labeled rooms the count differs. The alternative counts 100!/2^50 and 98!/2^49 have a ratio of 4,950; neither that ratio nor a claimed factor of 50 describes the unlabeled-pair count above. Neither reduction alone establishes a polynomial bound. Also, 100 students produce 50 pairs, not 100 pairs.
4. **A clue has a cost and an information source.** A clue deduced from the input is legitimate, but its derivation must be counted. A clue supplied by someone who knows a solution changes the information available to the algorithm. Reviewing all valid solutions to extract a clue may already require the computation being avoided. Early rejection of some wrong answers does not show how to construct a right one or decide that none exists.

One subtlety makes this a good control task: pairwise incompatibilities can be written as negative 2-CNF clauses, but adding the requirement to select at least k students recovers Independent Set. Dropping the cardinality condition makes the all-false assignment trivial. Changing representation must preserve every constraint.

## Research precedents worth borrowing

| Work | What was demonstrated | What transfers to this project |
| --- | --- | --- |
| [Counterexample-Driven Genetic Programming, 2017/2018](https://www.ijcai.org/proceedings/2018/0742.pdf) | Typed program evolution using SMT verification and counterexamples on specification-based synthesis tasks. | The proposed evolutionary feedback loop has a concrete precedent. Its small synthesis domains do not settle scalable verification of arbitrary SAT algorithms. |
| [FunSearch, 2023](https://www.nature.com/articles/s41586-023-06924-6) | Program evolution produced cap-set constructions and bin-packing heuristics; island populations maintained diversity. | Evolve compact generators or rules and evaluate their outputs. Keep distinct populations alive. |
| [AlphaEvolve, 2025](https://arxiv.org/abs/2506.13131) | Automated code evolution produced independently evaluable scientific and algorithmic constructions. | Use inexpensive generation for breadth, stronger reasoning for survivors, and staged evaluation. |
| [AlphaEvolve complexity-theory work, 2025; revised 2026](https://arxiv.org/abs/2509.18057) | Evolved finite combinatorial gadgets inside established frameworks, including improved MAX-4-CUT inapproximability. | This is the closest template: find a finite object whose verified property connects to a theorem over arbitrary sizes. Final gadgets were independently checked using the original exhaustive verifier. [Authors' account](https://www.research.google/blog/ai-as-a-research-partner-advancing-theoretical-computer-science-with-alphaevolve/) |
| [AlphaProof](https://research.google/pubs/olympiad-level-formal-mathematical-reasoning-with-reinforcement-learning/), [Aristotle, 2025](https://arxiv.org/abs/2510.01346), [DeepSeek-Prover-V2, 2025](https://arxiv.org/abs/2504.21801) | Formal proof search, informal lemma generation, and decomposition achieved strong results on specified mathematics benchmarks. | Use formal provers for bounded proof obligations. Check that the formal statement matches the intended theorem. |
| [Tao's Erdős #1026 account, December 2025](https://terrytao.wordpress.com/2025/12/08/the-story-of-erdos-problem-126/) | A collaboration combined numerical exploration, human interpretation, literature discovery, and proof. Some apparent novelty recovered prior work. | Treat discovery, generalization, literature checking, and proof as separate jobs. |
| [First Proof Second Batch, June 2026](https://arxiv.org/html/2606.18119v1) | Four systems produced 39 submissions on ten solved-but-unpublished research problems. Seven problems had a passing submission across systems, meaning essentially flawless or requiring minor revisions. | Research-level progress is real, but reviewers repeatedly found missing decisive steps and inaccurate citations. Harnesses could improve quality at substantial cost. Measure verified progress per total resource cost. |

A directly relevant caution is the 2023 paper [*Large Language Model for Science: A Study on P vs. NP*](https://arxiv.org/abs/2309.05689). It reports a dialogue-derived P≠NP proof schema. That is not an independently established solution. In this bounded search I found no accepted agent-produced resolution of P versus NP.

Older complexity work also matters. Williams's [ACC circuit lower bound](https://www.cs.cmu.edu/~ryanw/acc-lbs-ccc.pdf) shows that algorithmic ideas can lead to major complexity theorems, but its NEXP-versus-ACC result does not settle P versus NP. It is a model for choosing a precise intermediate theorem.

## Substrate recommendation

Use **a small typed S-expression language implemented in Rust**, with a thin orchestration layer. Lisp is the language the agents write; Rust is the language that implements its parser, type checker, interpreter, and explicit cost accounting. Choose Bun/TypeScript for the outer layer if the pilot immediately needs Oh's SDK; Python is equally reasonable when solver and scientific tooling dominate. The mathematical intermediate language should be independent of that choice.

Rust is a good fit for explicit AST variants, exhaustive pattern matching, controlled allocation, and native evaluation. Its ownership model supports memory safety without a garbage collector, and libraries such as Rayon support parallel evaluation. These are useful engineering properties; actual throughput must be measured, and Rust does not prove the interpreter correct or its candidates polynomial. [Rust enums](https://doc.rust-lang.org/book/ch06-00-enums.html), [Rust ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html), [Rayon](https://docs.rs/rayon/latest/rayon/)

| Component | Proposed role | Constraint |
| --- | --- | --- |
| Lisp-style typed AST | Compact candidate programs, rules, predicates, and proof obligations | Explicit semantics, bounded data constructors, canonical serialization, and meaningful names |
| Rust core | Parser, types, deterministic interpreter, execution limits, cost counters, and batch evaluation | Keep the first evaluator small and auditable; memory safety is distinct from mathematical correctness |
| Racket/Rosette | Optional symbolic interpreter for selected synthesis sketches | Add only when automatic symbolic execution saves work; prove or test correspondence with Rust semantics and record query bounds |
| SAT/SMT tools | Counterexample discovery, finite reference answers, and checking local obligations | Their search cost belongs to the research harness; they cannot be free primitives in a claimed polynomial SAT solver |
| Prolog | Optional relational search for lemmas or derivations | Backtracking and tabling do not establish polynomial runtime; proof traces and termination still need analysis |
| egg/egglog | Optional sharing of equivalent expressions and sound rewrite results | E-class equality requires semantic equivalence in every allowed context; keep whole-formula equisatisfiable transformations as directed checked reductions with explicit conditions |
| Lean | Check stable definitions and important lemmas; later audit the complete theorem | A successful proof must cover the intended statement, its assumptions, and cost model |
| Oh and repository notes | Experiment provenance, durable findings, dependencies, and readable synthesis | Evidence storage must preserve the distinction between observation, conjecture, and proof |

[Rosette's official description](https://emina.github.io/rosette/) supports its optional solver-aided role. Rust can also emit explicit SMT-LIB queries directly to a solver such as [Z3](https://microsoft.github.io/z3guide/docs/logic/intro/), avoiding a second interpreter until needed. [egg](https://github.com/egraphs-good/egg) and [egglog](https://arxiv.org/abs/2304.04332) provide concrete machinery for shared equivalence and relational reasoning. This combination is a design recommendation, not an empirically established winner for this task.

Start with Boolean operations, explicit finite collections, input-indexed iteration, conditionals, and structural transformations. Every primitive declares both its semantics and cost. Track intermediate bit lengths, number of generated clauses or states, and total work across branches. A loop that removes one variable per level can still spawn exponentially many branches. A compact symbolic expression can still expand exponentially.

A polynomial-by-construction fragment can cheaply reject unbounded computation and focus search on correctness. Its expressiveness is limited, so keep a separate exploratory lane for promising algorithms whose complexity requires a new argument. Failure to find a solution in either grammar says nothing about all possible algorithms. Revise the grammar when evidence identifies a missing concept.

Implement a direct interpreter first, with pure expressions, immutable values, explicit iteration, and execution and allocation limits. Compile the interpreter once and feed it candidate ASTs rather than compiling every candidate. Defer macros, a JIT, general-purpose metaprogramming, and a large standard library. Reuse a parser library if it preserves the chosen grammar cleanly. If Rosette would simplify a specific synthesis task, compare that task against the Rust-to-SMT path before adding it. An independent tiny Python reference evaluator is still useful for differential checking; it need not become the production substrate.

Choose explicit integer or bitvector semantics. Checked overflow, modular arithmetic, and arbitrary-precision integers represent different mathematical operations. Preserve those choices across build profiles, the reference evaluator, and solver encodings. Charge copying, allocation, comparison, and arithmetic as a function of represented size; host machine instructions are not automatically unit-cost mathematical operations. [Rust overflow semantics](https://doc.rust-lang.org/reference/expressions/operator-expr.html#overflow)

Measure total tokens, repair attempts, solver time, reproducibility, and ease of proving the resulting rules. Do not infer token efficiency from character count or parentheses. Shared definitions, structured failures, and short retrieved context are likely more valuable than cryptic syntax. Parallel evaluation improves the research harness's throughput; exponential total work remains exponential even when distributed across workers.

## Wide exploration followed by selective convergence

Use a portfolio of independent conceptual families. Keep the ultimate target 3-SAT, with the originating sketch's Independent Set formulation as an interpretable secondary representation.

| Search family | Candidates to generate | Required bridge toward the target |
| --- | --- | --- |
| Constraint inference | Sound propagation and reduction rules | Why enough progress is always available and cheap |
| Representation and decomposition | Encodings, summaries, component splits, compressed states | Why construction and intermediate size stay polynomial on arbitrary inputs |
| Invariants and potential functions | Progress measures and amortized analyses | A bound on total work, including all branches and data growth |
| Proof construction | Uniform generators of checkable UNSAT derivations or useful auxiliary definitions | Polynomial construction time and coverage of all UNSAT instances, combined with a complete decision procedure |

Each island runs the same basic cycle:

1. Retrieve its exact target, allowed primitives, nearest checked lemmas, and representative counterexamples.
2. Propose a typed program fragment, reduction rule, invariant, or finite mathematical object with a specific claim.
3. Reject malformed candidates, invalid primitive use, and known counterexamples cheaply.
4. Test survivors against exact small references, hidden examples, adversarial families, and independent symbolic checks.
5. Minimize counterexamples and share them with every affected island. Preserve promising but unproved candidates with their precise open obligation.
6. Ask an independent reviewer to assess the mathematical generalization and audit claimed runtime. Formalize survivors where it materially resolves uncertainty.
7. Recombine compatible checked components and allocate more work to families producing reusable results.

Use typed mutation and contract-preserving crossover. A candidate that deletes every possible solution is not successful because it prunes aggressively. A SAT-only witness finder is incomplete on UNSAT instances. A timeout means unknown, not UNSAT or disproof.

An SMT result of UNSAT for a counterexample query establishes the absence of counterexamples only under that query's exact encoding, domain, and abstraction. Bounded unrolling, fixed bit widths, or finite fuel cannot silently become a proof for all input sizes. [Rosette reasoning precision](https://docs.racket-lang.org/rosette-guide/ch_essentials.html)

Keep separate records for **refuted**, **bounded-tested**, **proved on a stated class**, and **proved generally**. Do not collapse these into one score. Within a status and domain, compare useful progress, observed cost, proven bounds, representation size, novelty, and verification cost. Prefer a Pareto frontier to a single fitness number.

Initially spread candidate-generation effort across the conceptual families. After comparable trials, devote more effort to survivors while retaining an exploratory reserve, for example 20%. This is a starting policy to test. Maintain counterexample generation, review, and literature work as separately budgeted functions so they survive convergence.

Run a same-budget comparison against independent best-of-N sampling, a simple beam search, and seeded conventional synthesis. Evolution earns its place only if it improves checked output per token and CPU-hour. Save failed attempts by mathematical failure mode so subsequent agents avoid rediscovering them.

## The first bounded pilot

The first observable outcome should be a reliable search-and-check loop plus a handful of proved or refuted rules. It should expose the originating sketch's selection-versus-pairing mistake and recover known valid simplifications before attempting novel generalization.

**Step 1: establish specifications and controls.** Define arbitrary-length CNF/3-CNF inputs and graph-selection inputs with k included. Keep perfect matching as an explicitly separate control. Use tiny exhaustive reference evaluators and a second independent implementation to detect harness errors. Include satisfiable, unsatisfiable, and malformed inputs.

Audit symbolic encodings explicitly: finding a formula that a candidate falsely accepts can require showing that every assignment falsifies the formula. An ordinary existential SAT query is not automatically a complete checker for that claim. Begin with ground instances and tiny truth-table labels; record quantifiers and abstractions when adding symbolic counterexample search.

**Step 2: rediscover a small known rule.** Hide a unit-propagation template behind a synthesis hole: if a CNF contains unit literal l, satisfy l, remove satisfied clauses, and delete its negation from other clauses. Synthesize the transformation, search for counterexamples, and prove satisfiability preservation using assignment restriction and extension. Measure actual traversal and representation cost. This is a calibration exercise; rediscovery is not novel mathematics.

**Step 3: expose a false generalization.** The formula

\[
(x\lor y)\land(x\lor\neg y)\land(\neg x\lor y)\land(\neg x\lor\neg y)
\]

has no unit clause and is unsatisfiable: for each of the four assignments one clause is false. Unit propagation alone therefore stalls even on this small 2-CNF instance. The harness should find or reproduce this counterexample and refuse to promote the claim that propagation always decides satisfiability. The example is a general-CNF control; a separate 3-CNF corpus must obey its exact encoding convention.

**Step 4: widen the rule search.** Explore sound bounded-width implications, substitutions, decomposition rules, and candidate progress measures. Vary sizes and structures, including pigeonhole, parity/Tseitin, and crafted cases tailored to survivors. Include held-out families, not just random formulas. Treat runtime curves as a way to find failures, not a proof of asymptotic complexity.

**Step 5: select one theorem-sized follow-up.** Continue only when a candidate has a clear universal statement, checked local behavior, and a plausible bridge from finite observations to an unbounded family. A proved improvement on a restricted class, a new counterexample family, or a better proof generator is a legitimate intermediate result. Label its distance from general 3-SAT precisely.

For an initial pilot, propose a ceiling such as 100 candidate proposals per family, three independent seeds, and fixed per-candidate checking limits. Register total token and CPU budgets before execution, and split them comparably across the methods being compared. These are pilot parameters to choose after measuring evaluator cost, not a forecast of discovery time or a request to launch paid compute now.

Acceptance requires deterministic replay, correct handling of unknown results, independent reference agreement, a proved calibration rule, rejection of planted false claims, complete resource accounting, and a comparative report. If the evaluator is unreliable or the evolutionary method loses to simpler sampling, repair the evaluator or use the simpler method before increasing scale. Keep raw traces so any erroneous promotion can be retracted and its dependent claims marked for review.

## Mathematical limits that should guide search

Resolution-only refutation systems have known exponential lower bounds on some families. Merely evolving better branching choices within that restriction cannot give polynomial refutations everywhere. Stronger representations can change the situation: explicit auxiliary-variable constructions yield short DRAT proofs for pigeonhole formulas. That is useful evidence for representation search, while remaining a result about a particular family. [Grosof, Zhang, and Heule, 2022](https://arxiv.org/abs/2207.11284)

Short proofs and efficiently finding proofs are separate properties. Polynomially bounded propositional proof systems correspond to NP = coNP; that alone does not establish P = NP. A proof-search route needs the constructive algorithm and its runtime, not only the existence of compact certificates. [Cook and Reckhow](https://www.cs.toronto.edu/~sacook/homepage/cook_reckhow.pdf)

Relativization and algebrization limit broad proof techniques. Natural-proofs results concern particular circuit-lower-bound methods under stated assumptions; they are not a blanket prohibition on searching for a polynomial algorithm. Audit whether a proposed theorem falls inside a known restriction rather than treating barrier names as a universal veto. [Aaronson and Wigderson's algebrization paper](https://www.scottaaronson.com/papers/alg.pdf), [Razborov and Rudich's natural proofs](https://doi.org/10.1006/jcss.1997.1494)

Formalization can begin early with the stable language semantics and a calibration lemma, without asking every speculative idea to begin in Lean. Before any P = NP claim, require independent mathematical review and a full proof chain with explicit assumptions, a faithful formal target, and a justified connection between the interpreter's cost model and standard computation. Formal verification is valuable evidence; it does not automatically confer Millennium Prize recognition.

## How Oh and research notes can help

Oh is a local-first ontology kernel with versioned records, explicit dependencies, an append-only operation history, and derived search/projection facilities. Its records can separate a statement from an attributable assertion and from the evidence supporting it. These are appropriate building blocks for a research ledger. [Official Oh repository](https://github.com/hraness/oh)

Use an application profile to represent the research question, exact source editions, candidate program hashes, conjectures, proof obligations, observations, counterexamples, reviewer decisions, and artifacts. For each experimental result, retain the input generator or corpus digest, seed, toolchain, verifier version, bounds, logs, and total cost. Record whether evidence is exhaustive within a finite domain, symbolic under an abstraction, a human-reviewed proof, or a proof accepted by a named formal kernel.

Oh's integrity checks establish custody and consistency of records under its contracts. They do not establish mathematical truth. Likewise, a derived graph trace is not a Lean theorem. Claims and dependencies still need the appropriate mathematical checker.

Oh's finite positive-rule projections may also help with bounded dependency queries. If a research island explores Datalog as a SAT language, polynomial closure alone is insufficient: it must establish a lossless polynomial-size encoding, all-instance correctness, and bounds on relation arity, rule width, generated domains, and total materialization. The cost cannot be hidden in the translation. [Oh projection specification](https://github.com/hraness/oh/blob/main/spec/v1/projection.md)

Oh in peqnp is the canonical research record. Its reviewed, Git-versioned operation history carries the project's claims, evidence, arguments, and open questions into each clone; the local SQLite database is a replayed materialization. Repository Markdown remains the readable layer for the proposal, lessons, definitions, rejected approaches, and open obligations, synchronized with recorded document editions. Record new learning in this ledger. Follow the [ledger workflow](oh.md), preserve the pinned contract and explicit record schema, and review the complete exported history before publication.

## Decision

Proceed with a small typed Lisp interpreter in Rust and a counterexample-guided portfolio. Add Rosette or egglog only for a demonstrated research need. Evolve bounded mathematical components with explicit correctness claims. Converge on a theorem and the hardest unresolved proof obligation as evidence accumulates.

The central question is: **Can we always derive enough sound constraints to decide an arbitrary instance, while bounding the total derivation and representation cost by a fixed polynomial?** This program makes that question testable in small pieces and preserves useful discoveries even if the P = NP hypothesis proves unproductive.

## Side information and collaboration

A complementary paper direction studies whether clues from solved instances, domain knowledge, or an opponent model can reduce the choices a deterministic procedure must explore. The source, reliability, size, and acquisition cost of that information are part of the model. The [knowledge and complexity framework](knowledge-and-complexity.md) distinguishes this hypothesis from a proof that arbitrary SAT has a uniform polynomial algorithm.

[Album Shen](https://www.linkedin.com/in/albumshen) contributed the motivating taxonomy of human, machine, guessing, chance, and adversarial problems and discussion of clue-driven search. Ben Guo initiated the research program and proposed investigating reusable knowledge bases. These credits describe conceptual contributions; the formalizations and experimental conclusions carry their own evidence.
