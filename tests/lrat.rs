//! Independent checks of the LRAT checker against CaDiCaL 3.0.1 proofs
//! (`--lrat --binary=false`) and against deliberately damaged proofs.

use peqnp::lrat::{check, LratError};
use peqnp::Cnf;
use std::path::PathBuf;
use std::time::Instant;

const R16: &str = include_str!("lrat/r16.cnf");
const P16: &str = include_str!("lrat/p16-1.lrat");
const R30: &str = include_str!("lrat/r30.cnf");
const P30: &str = include_str!("lrat/p30-1.lrat");

/// Location of the large pigeonhole fixture (`php.cnf` and `php.lrat`),
/// overridable with `PEQNP_LRAT_PHP_DIR`. The 7 MB proof is not committed;
/// the test skips when the files are absent.
const PHP_DEFAULT_DIR: &str = "tests/lrat";

fn dimacs(text: &str) -> (Cnf, u32) {
    let mut variables = 0;
    let mut clauses = Vec::new();
    let mut current = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('c') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("p cnf") {
            variables = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .expect("variable count");
            continue;
        }
        for token in line.split_whitespace() {
            let lit: i32 = token.parse().expect("literal");
            if lit == 0 {
                clauses.push(std::mem::take(&mut current));
            } else {
                current.push(lit);
            }
        }
    }
    assert!(current.is_empty(), "unterminated clause");
    (clauses, variables)
}

fn edit_line(proof: &str, prefix: &str, replacement: &str) -> String {
    let mut found = false;
    let edited: Vec<String> = proof
        .lines()
        .map(|line| {
            if line.starts_with(prefix) {
                found = true;
                replacement.to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect();
    assert!(found, "no line starts with {prefix:?}");
    edited.join("\n") + "\n"
}

#[test]
fn random_16_proof_verifies_unsat() {
    let (input, n) = dimacs(R16);
    assert_eq!(input.len(), 68);
    let report = check(&input, n, P16).expect("valid proof");
    assert!(report.proves_unsat);
    assert_eq!(report.lemmas, 15);
    assert_eq!(report.deletions, 2);
    assert_eq!(report.work.formula_checks, 15);
    assert!(report.work.clause_reads > 0);
    assert!(report.work.literal_writes > 0);
    assert_eq!(report.work.clause_writes, 0);
    assert_eq!(report.work.assignments, 0);
}

#[test]
fn work_accounting_for_one_lemma() {
    let (input, n) = dimacs(R16);
    let report = check(&input, n, "69 -13 -15 -16 0 42 54 40 46 28 12 0\n").expect("valid");
    assert!(!report.proves_unsat);
    assert_eq!(report.lemmas, 1);
    // One formula check; 3 lemma literals plus 6 hints of 3 literals read;
    // 3 assumption writes plus 5 propagated units; 6 hinted clauses read.
    assert_eq!(report.work.formula_checks, 1);
    assert_eq!(report.work.literal_reads, 21);
    assert_eq!(report.work.literal_writes, 8);
    assert_eq!(report.work.clause_reads, 6);
    assert_eq!(report.work.work_units, 36);
}

#[test]
fn random_30_proof_of_lemmas_without_empty_clause() {
    let (input, n) = dimacs(R30);
    assert_eq!(input.len(), 128);
    let report = check(&input, n, P30).expect("valid proof");
    assert!(!report.proves_unsat);
    assert_eq!(report.lemmas, 19);
    assert_eq!(report.deletions, 4);
}

#[test]
fn truncated_proof_is_valid_but_not_a_refutation() {
    let (input, n) = dimacs(R16);
    let prefix: Vec<&str> = P16.lines().take(P16.lines().count() - 1).collect();
    let report = check(&input, n, &prefix.join("\n")).expect("valid prefix");
    assert!(!report.proves_unsat);
    assert_eq!(report.lemmas, 14);
}

#[test]
fn pigeonhole_proof_verifies_quickly() {
    let dir = std::env::var_os("PEQNP_LRAT_PHP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(PHP_DEFAULT_DIR));
    let (Ok(cnf), Ok(proof)) = (
        std::fs::read_to_string(dir.join("php.cnf")),
        std::fs::read_to_string(dir.join("php.lrat")),
    ) else {
        eprintln!("skipping: php.cnf/php.lrat not found in {}", dir.display());
        return;
    };
    let (input, n) = dimacs(&cnf);
    assert_eq!(input.len(), 297);
    let started = Instant::now();
    let report = check(&input, n, &proof).expect("valid proof");
    let elapsed = started.elapsed();
    eprintln!(
        "php.lrat: {} lemmas, {} deletions, {} work units in {:?}",
        report.lemmas, report.deletions, report.work.work_units, elapsed
    );
    assert!(report.proves_unsat);
    assert_eq!(report.lemmas, 57_042);
    assert_eq!(report.deletions, 52_137);
    assert_eq!(report.work.formula_checks, 57_042);
    // Timing is reported, never asserted: wall time is machine-dependent.
    eprintln!("php proof check took {elapsed:?}");
}

#[test]
fn flipped_literal_fails_propagation() {
    let (input, n) = dimacs(R16);
    let proof = edit_line(P16, "69 ", "69 13 -15 -16 0 42 54 40 46 28 12 0");
    assert_eq!(
        check(&input, n, &proof),
        Err(LratError::HintNotUnit {
            lemma: 69,
            hint: 40
        })
    );
}

#[test]
fn missing_hint_fails_propagation() {
    let (input, n) = dimacs(R16);
    let proof = edit_line(P16, "69 ", "69 -13 -15 -16 0 42 40 46 28 12 0");
    assert_eq!(
        check(&input, n, &proof),
        Err(LratError::HintNotUnit {
            lemma: 69,
            hint: 40
        })
    );
}

#[test]
fn reordered_hints_fail_propagation() {
    let (input, n) = dimacs(R16);
    let proof = edit_line(P16, "69 ", "69 -13 -15 -16 0 12 42 54 40 46 28 0");
    assert_eq!(
        check(&input, n, &proof),
        Err(LratError::HintNotUnit {
            lemma: 69,
            hint: 12
        })
    );
}

#[test]
fn exhausted_hints_without_conflict_fail() {
    let (input, n) = dimacs(R16);
    let proof = edit_line(P16, "69 ", "69 -13 -15 -16 0 42 54 40 46 28 0");
    assert_eq!(
        check(&input, n, &proof),
        Err(LratError::HintNotUnit {
            lemma: 69,
            hint: 28
        })
    );
}

#[test]
fn deleted_clause_cannot_be_hinted() {
    let (input, n) = dimacs(R16);
    // Clause 54 is deleted by the line `70 d 54 0`, before lemma 71.
    let proof = edit_line(P16, "71 ", "71 8 -16 0 54 70 12 9 47 0");
    assert_eq!(
        check(&input, n, &proof),
        Err(LratError::UnknownClause { id: 54 })
    );
}

#[test]
fn deleting_an_inactive_clause_fails() {
    let (input, n) = dimacs(R16);
    let proof = edit_line(P16, "73 d", "73 d 54 0");
    assert_eq!(
        check(&input, n, &proof),
        Err(LratError::UnknownClause { id: 54 })
    );
}

#[test]
fn reused_id_is_rejected() {
    let (input, n) = dimacs(R16);
    let proof = edit_line(P16, "71 ", "70 8 -16 0 70 12 9 47 0");
    assert_eq!(check(&input, n, &proof), Err(LratError::IdNotIncreasing));
    let proof = edit_line(P16, "69 ", "68 -13 -15 -16 0 42 54 40 46 28 12 0");
    assert_eq!(check(&input, n, &proof), Err(LratError::IdNotIncreasing));
}

#[test]
fn negative_hint_is_unsupported() {
    let (input, n) = dimacs(R16);
    let proof = edit_line(P16, "69 ", "69 -13 -15 -16 0 42 -54 40 46 28 12 0");
    assert_eq!(check(&input, n, &proof), Err(LratError::Unsupported));
}

#[test]
fn malformed_lines_report_their_number() {
    let (input, n) = dimacs(R16);
    for (line, text) in [
        (1, "69 -13 -15 0 42\n"),
        (1, "69 -13 x 0 42 0\n"),
        (1, "69 -13 0 42 0 extra\n"),
        (1, "d 1 0\n"),
        (2, "69 -13 -15 -16 0 42 54 40 46 28 12 0\n69 d 1\n"),
        (3, "69 -13 -15 -16 0 42 54 40 46 28 12 0\n\n70 d 1 0 3\n"),
    ] {
        assert_eq!(
            check(&input, n, text),
            Err(LratError::Parse { line }),
            "{text:?}"
        );
    }
}

#[test]
fn literals_out_of_range_are_rejected() {
    let (input, n) = dimacs(R16);
    assert_eq!(
        check(&input, n, "69 17 0 1 0\n"),
        Err(LratError::LiteralOutOfRange)
    );
    assert_eq!(
        check(&input, n, "69 -2147483648 0 1 0\n"),
        Err(LratError::LiteralOutOfRange)
    );
    assert_eq!(
        check(&input, 15, ""),
        Err(LratError::LiteralOutOfRange),
        "input literal above n"
    );
}

#[test]
fn empty_proof_on_satisfiable_formula() {
    let (input, n) = dimacs(R30);
    let report = check(&input, n, "").expect("empty proof is valid");
    assert!(!report.proves_unsat);
    assert_eq!(report.lemmas, 0);
    assert_eq!(report.deletions, 0);
    assert_eq!(report.work.work_units, 0);
}

#[test]
fn empty_clause_in_input_is_refuted_by_one_hint() {
    let input: Cnf = vec![vec![], vec![1, 2]];
    let report = check(&input, 2, "3 0 1 0\n").expect("valid");
    assert!(report.proves_unsat);
    assert_eq!(report.lemmas, 1);
    let input: Cnf = vec![vec![]];
    let report = check(&input, 0, "2 0 1 0").expect("valid without newline");
    assert!(report.proves_unsat);
    assert_eq!(
        check(&input, 0, "2 0 0\n"),
        Err(LratError::HintNotUnit { lemma: 2, hint: 0 }),
        "no hints yields no conflict"
    );
}

#[test]
fn satisfied_hint_is_not_unit() {
    let input: Cnf = vec![vec![1, 2], vec![-1, 2]];
    // Under -2, clause 1 is unit on 1; clause 1 again is satisfied, not unit.
    assert_eq!(
        check(&input, 2, "3 2 0 1 1 2 0\n"),
        Err(LratError::HintNotUnit { lemma: 3, hint: 1 })
    );
    let report = check(&input, 2, "3 2 0 1 2 0\n").expect("valid");
    assert_eq!(report.lemmas, 1);
    assert!(!report.proves_unsat);
}
