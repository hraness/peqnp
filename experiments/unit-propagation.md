# Unit propagation calibration

This is a completed finite calibration experiment, not a P versus NP result.

Run `cargo run --locked --release -- experiment artifacts/calibration.json`.
The Rust executable is compiled once, then interprets eight candidate ASTs. The
experiment uses complete enumeration, so there is no random seed. Repeating the
command produces the same JSON bytes. The JSON includes every candidate's scores,
first preservation counterexample when present, and measured interpreter work.

## Search and reference

The candidate grammar is

```text
(with-unit input (rewrite input L D E))
L ::= unit | (neg unit)
D ::= false | true
E ::= false | true
```

`with-unit` evaluates its source formula and selects its first singleton clause.
The body receives that source as `input` and its literal as `unit`. If there is no
singleton clause, the source is returned unchanged. `rewrite` optionally drops
clauses containing the chosen literal (`D`) and optionally removes its negation
from remaining clauses (`E`). `unit` is a Literal and `input` is a Formula; the
parser rejects wrong types, unbound units, unknown operators, and wrong arities.

The corpus contains the 16 subsets of `{x, not x, y, not y}` as canonical clauses,
including the empty clause and tautological clauses. Every one of the 65,536
subsets of those clauses is checked, in a fixed order. This covers canonical
two-variable clause sets, not every ordering or repetition of a syntactic formula.
An independent truth-table evaluator enumerates all four assignments when needed
to decide whether an input or output is satisfiable. It is outside the candidate
language; no candidate can invoke it.

Each candidate has two independently measured obligations:

1. Input and output have the same satisfiability value.
2. When a unit was selected, neither polarity of its variable remains in the
   output; when no unit exists, the formula is unchanged.

The unique candidate satisfying both obligations throughout the corpus is

```lisp
(with-unit input (rewrite input unit true true))
```

It passes all 65,536 preservation and progress checks, using 10,221,568 recorded
work units across its evaluations. Identity also preserves satisfiability but
fails the elimination requirement. Assigning the negation of the unit can
eliminate the variable while changing satisfiability. Thus neither raw pruning
nor preservation alone is the selection objective.

The planted false claim that unit propagation decides every CNF is rejected by

```text
(x or y) and (x or not y) and (not x or y) and (not x or not y)
```

This formula has no unit clauses and is unsatisfiable. Every one of the four
assignments falsifies a clause. The discovered transformation leaves it unchanged.

## General argument for the recovered local rule

The following is an informal mathematical proof, not a kernel-checked artifact.
For every finite CNF F that contains singleton clause `{l}`, let R be the formula
obtained by deleting clauses containing l and deleting the opposite literal from
every other clause. Then F is satisfiable if and only if R is satisfiable.

Every satisfying assignment of F makes l true. Deleted clauses are already true,
and deleted opposite literals are false, so restricting that assignment to the
remaining variables satisfies R. Conversely, extend any assignment satisfying R
by making l true. It satisfies every deleted clause and every original remaining
clause, so it satisfies F. This argument covers empty clauses, tautologies, and
repeated literals as well as the canonical experimental corpus. When no singleton
clause exists, the language returns the input and preservation is immediate.

The rule never adds a clause or literal. When applied, it removes both polarities
of the selected variable. A direct implementation takes a constant number of
traversals of the input. Repeated naive application can make at most the original
number of variable eliminations, but can stall before deciding the formula, as
the counterexample shows. This is a local soundness and progress argument; it is
not a completeness argument or a new complexity theorem.

## Accounting, numerical semantics, and limits

CNFs use signed 32-bit nonzero literals; `i32::MIN` is rejected so negation is
defined. Indices here are concrete labels, not arbitrary-precision integers.
The abstract argument above covers arbitrary finite variable sets, whereas this
implementation and its oracle have explicit finite representational limits. The
oracle rejects domains above 20 variables.

Fuel is charged for AST visits, clause reads/writes, and literal reads/writes,
including input validation and copies. Every charged operation updates checked
64-bit counters; a fuel limit returns `UnknownBudget` and no output formula.
Arithmetic overflow checks remain enabled in release builds. Vector allocation,
capacity growth, byte copying by the allocator, and allocator/runtime overhead are
not exact bit-operation counts. The event counters measure declared operations;
they are not a proved translation to a standard computation model. The pilot
uses fuel 100,000 per evaluation; no evaluation exhausted it.

The grammar is deliberately hand-scoped around a known rule. This is enumerative
synthesis, without a genetic population, model-generated mutations, novel
discovery, or a measured comparison between search strategies. The experiment
establishes that this small harness distinguishes preservation, useful local
progress, and an incorrect completeness claim. Larger claims require new evidence.
