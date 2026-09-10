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
bun run check
```

The experiment writes [calibration.json](artifacts/calibration.json). Eight programs are checked against all 65,536 canonical two-variable clause sets. Exactly one satisfies both satisfiability preservation and variable elimination:

```lisp
(with-unit input (rewrite input unit true true))
```

It recovers known unit propagation and rejects a planted claim that this rule alone decides every CNF. The [calibration report](experiments/unit-propagation.md) defines the language, domain, and informal soundness proof.

The [clue-transfer experiment](experiments/clue-transfer.md) mines four sound rule instances from six tiny formulas and freezes them before testing 122 larger cases. They produce 127 clues and reduce search nodes from 738 to 620, but total measured work grows from 424,997 to 1,676,645 because matching is expensive. Every case is solved correctly; this implementation is slower by the declared event metric on every case. These are finite research results, not a polynomial SAT solver. `bun run check` checks Rust and ledger behavior and reproduces both artifacts byte for byte.

Restore the canonical [Oh](https://github.com/hraness/oh) research ledger after cloning:

```sh
bun run oh:init
bun run oh:verify
```

The Git-versioned [ledger](ledger/manifest.json) is the source of truth for research claims, evidence, reviewed arguments, and open questions. Each clone rebuilds its ignored `.oh/research.sqlite` from the same operation history. Research Markdown is the readable layer, checked against recorded document editions. Its records distinguish hypotheses and bounded observations; replay integrity does not establish mathematical truth. See [ledger operation](docs/oh.md) to add and publish new learning.

## Research approach

Rust implements a small typed Lisp for candidate transformations. The [proposal](docs/research-proposal.md) extends this into parallel search across rule families, counterexample-guided refinement, and eventual convergence on precise theorem statements. Evolutionary search and model-driven populations are planned; their value must be measured against simpler search under equal budgets.

The [knowledge-and-complexity framework](docs/knowledge-and-complexity.md) investigates clues from solved examples, reusable knowledge bases, and information about opponents. The [transfer theory](docs/clue-transfer-theory.md) connects this to forced values, local explanations, and small branching sets. Next, test cheaper rule matching and inference that composes longer implications, counting acquisition, representation, and residual solving separately.

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
