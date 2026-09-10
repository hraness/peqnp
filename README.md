# peqnp

A research laboratory for exploring P = NP through small programs, counterexamples, and explicit proofs. Researchers can reproduce candidate SAT transformations and inspect exactly which claims survive independent checks.

We seek one uniform deterministic algorithm for an NP-complete problem, with correctness on every input and a proved polynomial worst-case bound. [P versus NP remains unsolved](https://www.claymath.org/millennium/p-vs-np/).

## Run the experiments

Requires Git, [Rust 1.97.1](https://www.rust-lang.org/tools/install) with Cargo, rustfmt and Clippy, and [Bun 1.3.14](https://bun.sh/docs/installation). Rustup uses the checked-in toolchain pin. Installation downloads the pinned Oh release; the experiment itself runs locally without a model or service account.

```sh
git clone https://github.com/hraness/peqnp.git
cd peqnp
bun install --frozen-lockfile --ignore-scripts
cargo run --locked --release -- experiment artifacts/calibration.json
cargo run --locked --release -- transfer artifacts/clue-transfer.json
cargo run --locked --release -- indexed artifacts/indexed-transfer.json
cargo run --locked --release -- implication artifacts/implication-calibration.json
cargo run --locked --release -- fragment artifacts/fragment-interface.json
bun run check
```

The experiment writes [calibration.json](artifacts/calibration.json). Eight programs are checked against all 65,536 canonical two-variable clause sets. Exactly one satisfies both satisfiability preservation and variable elimination:

```lisp
(with-unit input (rewrite input unit true true))
```

It recovers known unit propagation and rejects a planted claim that this rule alone decides every CNF. The [calibration report](experiments/unit-propagation.md) defines the language, domain, and informal soundness proof.

The [clue-transfer experiment](experiments/clue-transfer.md) mines four sound rule instances from six tiny formulas and freezes them before testing 122 larger cases. They produce 127 clues and reduce search nodes from 738 to 620, but total measured work grows from 424,997 to 1,676,645 because matching is expensive. Every case is solved correctly; this implementation is slower by the declared event metric on every case.

The [indexed-transfer experiment](experiments/indexed-transfer.md) replaces pair enumeration with a sorted index that derives the same units in the same order. On 160 fresh cases it cuts preprocessing from 11,235,212 to 1,048,871 events and finishes four cases the generic matcher could not, yet still costs more than no preprocessing on every case: 1,452,179 events against a 520,161 baseline. The [implication-calibration experiment](experiments/implication-calibration.md) builds the standard 2-SAT implication graph instead. On 120 two-variable-clause cases it recovers all 173 forced literals where the two-clause library finds 63, including every long-chain case the library cannot start; its linear-time decision with an explicit certificate still costs more declared events than the toy solver on 102 of 120 cases. The [fragment-interface experiment](experiments/fragment-interface.md) moves to 162 mixed 2/3-CNFs where search is expensive and reads only the binary fragment through that graph. The interface delta stays negative on 140 cases and turns positive on 22, all but one of them cases where the fragment's own contradiction decided the whole formula without search. These are finite research results, not a polynomial SAT solver. `bun run check` checks Rust and ledger behavior and reproduces all five artifacts byte for byte.

Restore the canonical [Oh](https://github.com/hraness/oh) research ledger after cloning:

```sh
bun run oh:init
bun run oh:verify
```

The Git-versioned [ledger](ledger/manifest.json) is the source of truth for research claims, evidence, reviewed arguments, and open questions. Each clone rebuilds its ignored `.oh/research.sqlite` from the same operation history. Research Markdown is the readable layer, checked against recorded document editions. Its records distinguish hypotheses and bounded observations; replay integrity does not establish mathematical truth. See [ledger operation](docs/oh.md) to add and publish new learning.

## Research approach

Rust implements a small typed Lisp for candidate transformations. The [proposal](docs/research-proposal.md) extends this into parallel search across rule families, counterexample-guided refinement, and eventual convergence on precise theorem statements. Evolutionary search and model-driven populations are planned; their value must be measured against simpler search under equal budgets.

The [knowledge-and-complexity framework](docs/knowledge-and-complexity.md) investigates clues from solved examples, reusable knowledge bases, and information about opponents. The [transfer theory](docs/clue-transfer-theory.md) connects this to forced values, local explanations, and small branching sets. Four completed experiments now separate three costs: acquiring a rule library, reading its clues through a representation, and the search that remains. Cheaper reading and complete coverage were each achieved, and on formulas with expensive search the interface paid for itself exactly when its read was the decision. Next, make extraction linear and measure representations a solver builds anyway.

### Interfaces and free lunches

Michael Levin's [Ingressing Minds](https://doi.org/10.3390/philosophies11050161) describes bodies, machines, and algorithms as pointers into a space of patterns, where a good interface returns more than was put in: two angles of a triangle determine the third, and one transistor makes every logic gate available without evolving each truth table. Levin stresses that a pointer is not compression, because nothing recovers the seed from the pattern it unfolds into. That asymmetry is NP: a certificate is a pointer that is cheap to follow, and P versus NP asks whether one is always cheap to find. In this project's terms the free lunch is entailment, and its price is the cost of the representation that exposes it. A backbone literal is fixed by its formula, yet reading it from an arbitrary CNF is coNP-hard, while reading it from a 2-CNF implication graph takes linear time. We therefore score a representation by its interface delta: baseline work minus build, read, and residual work under one declared cost model. Pair matching scored negative on all 122 pilot cases, the sorted index on all 160 fresh cases, and the implication graph, which reads every forced literal, on 102 of 120 decisions. On 162 formulas with expensive search, 126 of them mixed 2/3-CNFs, the graph of the binary fragment scored positive on 22 cases, 21 of them where its certificate decided the formula outright. Levin's framework guides which interfaces to try and how to measure them; it cannot appear in a proof, whose cost model must be closed. The [full connection](docs/knowledge-and-complexity.md#interfaces-free-lunches-and-the-cost-of-reading-a-clue) gives the definition and the resulting strategy changes.

## Why P = NP would matter

A constructive solution with practical constants could transform constraint-based scheduling and design, and searching for polynomially bounded, efficiently checkable proofs. A polynomial bound alone need not be practical. [Cook's problem statement](https://www.claymath.org/wp-content/uploads/2022/06/pvsnp.pdf)

Album Shen's taxonomy connects that question to human decisions and games. It also helps identify what computation can and cannot supply:

| Examples | What must be specified |
| --- | --- |
| Parking, translation, choosing food | Available facts and a precise objective; finding the nearest known available space is already easy |
| Wordle and other guessing games | Hidden information and permitted observations; a clue changes what is known |
| Blackjack | Probabilities and revealed cards; faster computation does not reveal an unobserved random outcome |
| Chess, MMORPGs, racing | Rules, horizon, opponent model, and observations; insider knowledge can change the strategic problem |

These are motivating categories, not blanket NP-completeness claims. Each formal problem needs its own classification.

Collaborators: Ben Guo initiated the project and knowledge-base direction. [Album Shen](https://www.linkedin.com/in/albumshen) contributed the problem taxonomy and clue-driven search discussions. These credits describe conceptual contributions; formal claims require their own evidence.

[Agent instructions](AGENTS.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [MIT license](LICENSE)
