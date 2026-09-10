# Knowledge, clues, and computational complexity

Research note, 9 September 2026. [Album Shen](https://www.linkedin.com/in/albumshen) contributed the motivating taxonomy and discussion of clues that make subsequent solving more manageable. The motivation includes both clues revealed by solutions and side information such as insider knowledge about an opponent. The formal statements below are this project's analysis; conceptual collaboration does not imply endorsement of those statements. No resolution of P versus NP is claimed.

## Make the clue hypothesis precise

The useful hypothesis is that **reliable side information can eliminate choices until an exact deterministic procedure becomes tractable**. This includes inferred constraints, reusable domain facts, observations of hidden state, and information about an opponent's strategy. The research question is whether acquiring, representing, selecting, and applying enough such clues can have polynomial total worst-case cost for every input. “More manageable” in experiments is a valuable, weaker outcome. Additional knowledge is not established as necessary for every efficient algorithm.

For a Boolean formula F, let Models(F) be its satisfying assignments. An entailed clause c holds in every member of Models(F). A backbone literal is a literal true in every satisfying assignment of a satisfiable formula. Such information can be implicit:

\[
F=(x\lor y)\land(x\lor\neg y)\quad\Longrightarrow\quad x.
\]

Neither input clause is a unit, yet x is forced. Here a short resolution argument exposes the clue directly. Examining solutions can suggest the rule; a proof establishes its validity beyond the examples. Agreement among sampled solutions is insufficient: an unseen solution may violate the apparent clue. Require nonempty Models(F) when discussing backbones, since universal statements over an empty solution set hold vacuously.

There is a precise obstacle to making extraction free. Given any CNF F, introduce fresh x and define

\[
G=\bigwedge_{C\in F}(x\lor C).
\]

G is always satisfiable by setting x=true. Setting x=false leaves exactly F. Therefore **x is a backbone of G if and only if F is unsatisfiable**. This polynomial transformation proves that exact backbone membership on arbitrary satisfiable CNFs is coNP-hard. It even supplies an easy satisfying assignment; one known solution does not make all forced clues easy to recover. This is an elementary deduction here from the definitions and standard SAT completeness. [Cook's statement](https://www.claymath.org/wp-content/uploads/2022/06/pvsnp.pdf)

## Which kind of knowledge is available?

| Knowledge resource | Mathematical interpretation | Required accounting |
| --- | --- | --- |
| One fixed finite library of proved rules or finite-precision model weights | Can be included in one uniform program | Lookup, inference, and application costs; fixed knowledge is permitted by P |
| K(x) computed from the current input, or K_n generated from 1^n | Uniform preprocessing | Include generation, representation size, and subsequent computation |
| A polynomial-length K_n supplied separately for each input length n | Polynomial advice: the same advice serves every n-bit input | Exact polynomial-time use gives P/poly, without guaranteeing an efficient advice generator |
| Arbitrary information chosen for the particular x | Instance-specific assistance | An unverified answer bit already trivializes decision; a SAT witness only certifies a yes-instance |
| Access to an external set O through queries | An oracle model such as P^O | Specify the oracle and query costs; access to SAT does not prove SAT belongs to ordinary P |
| Feedback about a hidden state, as in a guessing game | Interactive information acquisition | Define observations, possible hidden states, rounds, and success guarantees |
| Insider information about an opponent's policy or private state | Side information in a specified game model | State reliability and acquisition; a policy prediction need not support a worst-case guarantee against every opponent |

Polynomial advice and uniform algorithms are different quantifier structures: “for every n, an appropriate K_n exists” does not supply one efficient construction for all n. The standard circuit/advice equivalence explains why polynomial-size learned artifacts, if supplied by length and exactly correct, naturally fit P/poly. [Arora and Barak, Chapter 6](https://theory.cs.princeton.edu/complexity/book.pdf)

For learning, state the data source, access protocol, training resources, model size, and error guarantee. Distributional accuracy does not imply exact correctness on every input. If labels or feedback require solving the hard problem, acquisition cannot disappear from a claimed uniform bound. [Valiant's original learning framework](https://courses.grainger.illinois.edu/ece544na/fa2014/valiant84.pdf)

## A simple theorem and the unresolved work

**Composition lemma.** Fix a finite library B and deterministic algorithms C and D. Suppose C(x,B) produces K in at most p(n) bit operations with |K|≤p(n), where n=|x|. Suppose D(x,K,B) always returns the correct decision in at most q(n+|K|) bit operations, for fixed nondecreasing polynomial bounds p and q. Then their composition decides the language in polynomial time.

**Proof.** Include B in the program. Compute K, then run D. Total time is at most p(n)+q(n+p(n)), a polynomial. The claim requires total correctness on yes- and no-instances, including all intermediate data costs.

Applied to general SAT, these hypotheses would establish P=NP. The lemma is elementary closure under composition; it supplies no missing compiler or decider. To make the investigation substantive, fix a clue language, sound extraction rules, a deterministic consumer, and an explicit bound. Seek proofs of adequacy and cost for increasingly broad families, retaining counterexamples when a claim fails.

Knowledge compilation studies a closely related tradeoff: compile a theory offline into a representation supporting cheap repeated queries. Its three distinct concerns are representation size, supported queries, and supported transformations. Fast online answers can coexist with expensive compilation. [Darwiche and Marquis](https://www.cs.cmu.edu/afs/cs.cmu.edu/project/jair/pub/volume17/darwiche02a.pdf) For m queries, report both total cost T_compile+ΣT_query and its amortized value; amortization does not prove polynomial one-instance SAT complexity.

Some proposed representations have known limitations: there are CNFs requiring exponentially larger equivalent DNNFs. Thus universal polynomial compilation into that specific language is not an open route to assume; use a stated restricted family or investigate another representation. [Bova et al.](https://www.ijcai.org/Proceedings/16/Papers/147.pdf)

## A clue-transfer experiment

The [first completed clue-transfer experiment](../experiments/clue-transfer.md) implements a small version of this design: exhaustively solve a tiny training corpus, extract entailed units, justify their reuse under renaming, freeze the library, and compare the same deterministic solver on larger cases with and without its preprocessing. All 122 cases were solved correctly, but matching overhead exceeded the residual search savings on every case. The [theory note](clue-transfer-theory.md) explains the soundness argument and why this narrow rule library misses some implicit structure even in 2-SAT.

Wider experiments should extract short entailed clauses or other reusable rule schemas and independently prove each schema sound. Count extraction, training, lookup, rule application, intermediate growth, and residual search separately. Preserve failures and distinguish finite observations from general proofs.

This tests three specific ideas: **acquisition** of implicit structure, **compression** into reusable rules, and **transfer** that reduces later work. It does not grant the tested solver access to the held-out solution sets. A later game experiment can compare strategies with and without specified opponent information, measuring benefit conditional on its accuracy. Clues may remove branches without removing exponential worst-case growth. Nondeterminism in NP is a mathematical computation model, not simply uncertainty about hidden facts or an opponent. Additional observations change the information available; their benefit must be distinguished from solving the original information-restricted task.

## Turn the taxonomy into specifications

Human versus machine, guessing versus competition, and single versus multiple solutions are useful descriptive axes. They do not assign P or NP-completeness to parking, translation, eating, or a named game. Each research task needs an input encoding and size parameter, output or decision relation, available information, horizon, and correctness criterion. Distinguish deciding existence, finding one solution, counting, enumeration, and strategy selection. A formula can have an easy existence answer and exponentially many assignments to enumerate. Classify only the resulting formal problem, including promises and information access, after establishing the relevant bounds.
