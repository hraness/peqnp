//! Focused correctness tests from the frozen implication protocol. These are
//! implementation checks, distinct from the 120 reported cost cases.

use peqnp::implication::*;
use peqnp::transfer::{self, Failure, Outcome, Work};
use peqnp::Cnf;

/// The nine canonical non-tautological clauses of width at most two on two
/// variables: the empty clause, four units, and four binary clauses.
const CANONICAL: [&[i32]; 9] = [
    &[],
    &[1],
    &[-1],
    &[2],
    &[-2],
    &[1, 2],
    &[1, -2],
    &[-1, 2],
    &[-1, -2],
];

fn subset(mask: u32) -> Cnf {
    CANONICAL
        .iter()
        .enumerate()
        .filter(|(bit, _)| mask & (1 << bit) != 0)
        .map(|(_, clause)| clause.to_vec())
        .collect()
}

fn work_sum(work: &Work) -> u64 {
    work.formula_checks
        + work.clause_reads
        + work.literal_reads
        + work.clause_writes
        + work.literal_writes
        + work.assignments
        + work.pair_checks
        + work.rule_attempts
        + work.search_nodes
}

fn chain(k: u32) -> Cnf {
    let y = |i: u32| (i + 1) as i32;
    let mut clauses = vec![vec![1, y(1)]];
    for i in 1..k {
        clauses.push(vec![-y(i), y(i + 1)]);
    }
    clauses.push(vec![-y(k), 1]);
    clauses
}

/// Compare the graph arm against the truth table and the crate's oldest oracle.
fn agrees_with_reference(input: &Cnf, n: u32) -> GraphArm {
    let reference = reference(input, n).unwrap();
    let oracle = peqnp::truth_table_sat(input, n).unwrap();
    assert_eq!(reference.model_count > 0, oracle, "{input:?}");
    let arm = analyze(input, n, ARM_BUDGET).unwrap();
    let witness = arm.certificate.as_ref().expect("full budget completes");
    let (valid, _) = check_certificate(input, n, witness).unwrap();
    assert!(valid, "{input:?}");
    if let Some(backbone) = &reference.backbone {
        assert_eq!(arm.decision, Outcome::Sat, "{input:?}");
        assert!(matches!(witness, Certificate::Sat(_)));
        assert_eq!(arm.backbone_status, BackboneStatus::Complete);
        let found: Vec<i32> = arm.clues.iter().map(|clue| clue.literal).collect();
        assert_eq!(&found, backbone, "{input:?}");
        for clue in &arm.clues {
            let (valid, _) = check_clue(input, n, clue).unwrap();
            assert!(valid, "{input:?} {clue:?}");
            assert_eq!(clue.path.nodes.first(), Some(&-clue.literal));
            assert_eq!(clue.path.nodes.last(), Some(&clue.literal));
        }
    } else {
        assert_eq!(arm.decision, Outcome::Unsat, "{input:?}");
        assert!(!matches!(witness, Certificate::Sat(_)));
        assert_eq!(arm.backbone_status, BackboneStatus::NotDefinedUnsat);
        assert!(arm.clues.is_empty());
    }
    let dpll = transfer::solve(input, n, None, ARM_BUDGET).unwrap();
    assert!(dpll.outcome.agrees(oracle));
    assert_ne!(dpll.outcome, Outcome::Unknown);
    for phase in [
        &arm.construction,
        &arm.scc,
        &arm.decision_certificate,
        &arm.backbone,
    ] {
        assert_eq!(phase.work_units, work_sum(phase));
    }
    assert!(arm.total_work() <= ARM_BUDGET);
    arm
}

#[test]
fn all_512_canonical_two_variable_subsets_match_the_truth_table() {
    for mask in 0..512 {
        agrees_with_reference(&subset(mask), 2);
    }
}

#[test]
fn duplicate_and_tautological_clauses_keep_their_boolean_meaning() {
    let inputs: Vec<Cnf> = vec![
        vec![vec![1, 2], vec![1, 2], vec![1, 2]],
        vec![vec![1, -1]],
        vec![vec![1, -1], vec![2, -2], vec![-1, 2]],
        vec![vec![1], vec![1], vec![-1, 2], vec![-1, 2]],
        vec![vec![1, 1], vec![-1, -1]],
        vec![vec![2, 2], vec![-1, 2], vec![1, -2], vec![1, -2]],
        vec![vec![1, -1], vec![1, -1], vec![1], vec![-1]],
        vec![vec![], vec![1, -1]],
        vec![vec![-3, -3], vec![3, 1], vec![1, 3], vec![-1, 2]],
    ];
    for input in &inputs {
        agrees_with_reference(input, 3);
    }
    // Repeated clauses contribute repeated edges and therefore more work.
    let once = analyze(&vec![vec![1, 2]], 2, ARM_BUDGET).unwrap();
    let twice = analyze(&vec![vec![1, 2], vec![1, 2]], 2, ARM_BUDGET).unwrap();
    assert!(twice.construction.work_units > once.construction.work_units);
    assert_eq!(once.clues.len(), 0);
    // A duplicate unit still yields one clue whose path cites a real clause.
    let unit = analyze(&vec![vec![2], vec![2]], 2, ARM_BUDGET).unwrap();
    assert_eq!(unit.clues.len(), 1);
    assert_eq!(unit.clues[0].literal, 2);
    assert_eq!(unit.clues[0].path.clauses, vec![0]);
    // The certificate for the empty clause cites the first empty index.
    let empty = analyze(&vec![vec![1], vec![], vec![]], 1, ARM_BUDGET).unwrap();
    assert_eq!(empty.certificate, Some(Certificate::EmptyClause(1)));
}

#[test]
fn raw_certificate_tampering_is_rejected_by_the_checker() {
    let n = 5;
    let input = chain(4);
    let arm = analyze(&input, n, ARM_BUDGET).unwrap();
    let Some(Certificate::Sat(values)) = arm.certificate.clone() else {
        panic!("expected a SAT certificate");
    };
    assert!(
        check_certificate(&input, n, &Certificate::Sat(values.clone()))
            .unwrap()
            .0
    );
    let mut flipped = values.clone();
    flipped[0] = false; // x is forced true.
    assert!(
        !check_certificate(&input, n, &Certificate::Sat(flipped))
            .unwrap()
            .0
    );
    assert!(
        !check_certificate(&input, n, &Certificate::Sat(values[..4].to_vec()))
            .unwrap()
            .0
    );
    assert!(
        !check_certificate(&input, n, &Certificate::EmptyClause(0))
            .unwrap()
            .0
    );
    assert!(
        !check_certificate(&input, n, &Certificate::EmptyClause(99))
            .unwrap()
            .0
    );

    let clue = arm.clues.iter().find(|clue| clue.literal == 1).unwrap();
    assert!(check_clue(&input, n, clue).unwrap().0);
    let mut wrong_literal = clue.clone();
    wrong_literal.literal = -1;
    assert!(!check_clue(&input, n, &wrong_literal).unwrap().0);
    let mut wrong_clause = clue.clone();
    wrong_clause.path.clauses[0] = (wrong_clause.path.clauses[0] + 1) % input.len();
    assert!(!check_clue(&input, n, &wrong_clause).unwrap().0);
    let mut dropped_node = clue.clone();
    dropped_node.path.nodes.remove(1);
    assert!(!check_clue(&input, n, &dropped_node).unwrap().0);
    let mut foreign_clause = clue.clone();
    foreign_clause.path.clauses[0] = input.len();
    assert!(!check_clue(&input, n, &foreign_clause).unwrap().0);
    let mut out_of_range = clue.clone();
    out_of_range.path.nodes[1] = 9;
    assert!(!check_clue(&input, n, &out_of_range).unwrap().0);
    let phantom = Clue {
        literal: 3,
        path: Path {
            nodes: vec![-3, 3],
            clauses: vec![0],
        },
    };
    assert!(!check_clue(&input, n, &phantom).unwrap().0);

    // UNSAT: an odd parity cycle yields two opposite paths.
    let cycle: Cnf = (1..=3)
        .flat_map(|i| {
            let next = i % 3 + 1;
            [vec![i, next], vec![-i, -next]]
        })
        .collect();
    let arm = analyze(&cycle, 3, ARM_BUDGET).unwrap();
    let Some(Certificate::OppositePaths {
        variable,
        positive_to_negative,
        negative_to_positive,
    }) = arm.certificate.clone()
    else {
        panic!("expected opposite paths");
    };
    assert_eq!(variable, 1);
    let genuine = Certificate::OppositePaths {
        variable,
        positive_to_negative: positive_to_negative.clone(),
        negative_to_positive: negative_to_positive.clone(),
    };
    assert!(check_certificate(&cycle, 3, &genuine).unwrap().0);
    let swapped = Certificate::OppositePaths {
        variable,
        positive_to_negative: negative_to_positive.clone(),
        negative_to_positive: positive_to_negative.clone(),
    };
    assert!(!check_certificate(&cycle, 3, &swapped).unwrap().0);
    let renamed = Certificate::OppositePaths {
        variable: 2,
        positive_to_negative: positive_to_negative.clone(),
        negative_to_positive: negative_to_positive.clone(),
    };
    assert!(!check_certificate(&cycle, 3, &renamed).unwrap().0);
    let mut truncated = positive_to_negative.clone();
    truncated.nodes.pop();
    truncated.clauses.pop();
    let truncated = Certificate::OppositePaths {
        variable,
        positive_to_negative: truncated,
        negative_to_positive: negative_to_positive.clone(),
    };
    assert!(!check_certificate(&cycle, 3, &truncated).unwrap().0);
    let mut miscited = negative_to_positive.clone();
    miscited.clauses[0] = (miscited.clauses[0] + 1) % cycle.len();
    let miscited = Certificate::OppositePaths {
        variable,
        positive_to_negative,
        negative_to_positive: miscited,
    };
    assert!(!check_certificate(&cycle, 3, &miscited).unwrap().0);
    // The certificate of one formula does not transfer to another.
    assert!(!check_certificate(&chain(2), 3, &genuine).unwrap().0);
    // Invalid inputs are rejected before any check.
    assert_eq!(
        check_certificate(&vec![vec![1, 2, 3]], 3, &genuine),
        Err(Failure::InvalidInput)
    );
    assert_eq!(
        check_clue(&vec![vec![0]], 3, &phantom),
        Err(Failure::InvalidInput)
    );
}

fn rename(input: &Cnf, permutation: &[i32]) -> Cnf {
    input
        .iter()
        .map(|clause| {
            clause
                .iter()
                .map(|&lit| {
                    let target = permutation[(lit.unsigned_abs() - 1) as usize];
                    if lit > 0 {
                        target
                    } else {
                        -target
                    }
                })
                .collect()
        })
        .collect()
}

#[test]
fn signed_and_order_transformations_preserve_decision_and_backbone() {
    let n = 6;
    let base = chain(5);
    let baseline = agrees_with_reference(&base, n);
    let baseline_backbone: Vec<i32> = baseline.clues.iter().map(|clue| clue.literal).collect();
    assert_eq!(baseline_backbone, vec![1]);
    for permutation in [
        vec![1, 2, 3, 4, 5, 6],
        vec![-1, 2, 3, 4, 5, 6],
        vec![6, 5, 4, 3, 2, 1],
        vec![-3, 1, -6, 2, -4, 5],
        vec![2, -1, 4, -3, 6, -5],
    ] {
        for reversed in [false, true] {
            let mut input = rename(&base, &permutation);
            if reversed {
                input.reverse();
            }
            let arm = agrees_with_reference(&input, n);
            let found: Vec<i32> = arm.clues.iter().map(|clue| clue.literal).collect();
            let expected: Vec<i32> = baseline_backbone
                .iter()
                .map(|&lit| {
                    let target = permutation[(lit.unsigned_abs() - 1) as usize];
                    if lit > 0 {
                        target
                    } else {
                        -target
                    }
                })
                .collect();
            assert_eq!(found, expected, "{permutation:?} reversed={reversed}");
            assert_eq!(arm.decision, baseline.decision);
            // Renaming variables and reversing clause order cannot change the
            // number of edges or the size of the stored certificate.
            assert_eq!(
                arm.construction.clause_writes,
                baseline.construction.clause_writes
            );
            assert_eq!(
                arm.construction.literal_writes,
                baseline.construction.literal_writes
            );
        }
    }
    // Swapping literal order inside each clause is also harmless.
    let swapped: Cnf = base
        .iter()
        .map(|clause| clause.iter().rev().copied().collect())
        .collect();
    let arm = agrees_with_reference(&swapped, n);
    assert_eq!(arm.clues.len(), 1);
    // An UNSAT cycle stays UNSAT under any signed renaming.
    let cycle: Cnf = (1..=5)
        .flat_map(|i| {
            let next = i % 5 + 1;
            [vec![i, next], vec![-i, -next]]
        })
        .collect();
    for permutation in [vec![5, 4, 3, 2, 1], vec![-2, 3, -1, 5, -4]] {
        let mut input = rename(&cycle, &permutation);
        input.reverse();
        assert_eq!(agrees_with_reference(&input, 5).decision, Outcome::Unsat);
    }
}

#[test]
fn budget_exhaustion_at_each_phase_boundary_stays_unknown() {
    let n = 8;
    let input = chain(7);
    let full = analyze(&input, n, ARM_BUDGET).unwrap();
    assert_eq!(full.decision, Outcome::Sat);
    assert_eq!(full.backbone_status, BackboneStatus::Complete);
    let construction = full.construction.work_units;
    let scc = construction + full.scc.work_units;
    let decision = scc + full.decision_certificate.work_units;
    assert_eq!(decision, full.decision_work());
    let total = decision + full.backbone.work_units;
    assert_eq!(total, full.total_work());
    assert!(full.backbone.work_units > 0);
    for budget in [
        0,
        1,
        construction - 1,
        construction,
        scc - 1,
        scc,
        decision - 1,
    ] {
        let arm = analyze(&input, n, budget).unwrap();
        assert_eq!(arm.decision, Outcome::Unknown);
        assert!(arm.certificate.is_none());
        assert_eq!(arm.backbone_status, BackboneStatus::Unknown);
        assert!(arm.clues.is_empty());
        assert_eq!(arm.total_work(), budget);
        assert_eq!(arm.backbone.work_units, 0);
        if budget < construction {
            assert_eq!(arm.scc.work_units, 0);
        }
        if budget < scc {
            assert_eq!(arm.decision_certificate.work_units, 0);
        }
    }
    for budget in [decision, decision + 1, total - 1] {
        let arm = analyze(&input, n, budget).unwrap();
        assert_eq!(arm.decision, Outcome::Sat);
        assert!(arm.certificate == full.certificate);
        assert_eq!(arm.decision_work(), decision);
        assert_eq!(arm.backbone_status, BackboneStatus::Unknown);
        assert!(arm.clues.is_empty());
        assert_eq!(arm.total_work(), budget);
    }
    let exact = analyze(&input, n, total).unwrap();
    assert_eq!(exact.backbone_status, BackboneStatus::Complete);
    assert_eq!(exact.clues, full.clues);
    assert_eq!(exact.total_work(), total);

    // Exhaustion never turns into UNSAT, and UNSAT never has a backbone.
    let cycle: Cnf = (1..=7)
        .flat_map(|i| {
            let next = i % 7 + 1;
            [vec![i, next], vec![-i, -next]]
        })
        .collect();
    let unsat = analyze(&cycle, 7, ARM_BUDGET).unwrap();
    assert_eq!(unsat.decision, Outcome::Unsat);
    assert_eq!(unsat.backbone_status, BackboneStatus::NotDefinedUnsat);
    assert_eq!(unsat.backbone.work_units, 0);
    assert_eq!(unsat.total_work(), unsat.decision_work());
    for budget in [0, unsat.construction.work_units, unsat.decision_work() - 1] {
        let arm = analyze(&cycle, 7, budget).unwrap();
        assert_eq!(arm.decision, Outcome::Unknown);
        assert_eq!(arm.backbone_status, BackboneStatus::Unknown);
    }
    assert_eq!(
        analyze(&cycle, 7, unsat.decision_work()).unwrap().decision,
        Outcome::Unsat
    );

    // The local and DPLL arms report exhaustion as incomplete/unknown too.
    let library = transfer::mine().unwrap().library;
    let local_full = local(&input, n, &library, ARM_BUDGET).unwrap();
    assert!(local_full.complete);
    let starved = local(&input, n, &library, local_full.work.work_units - 1).unwrap();
    assert!(!starved.complete);
    assert!(starved.units.is_empty());
    let dpll = transfer::solve(&input, n, None, ARM_BUDGET).unwrap();
    assert_eq!(dpll.outcome, Outcome::Sat);
    let starved = transfer::solve(&input, n, None, dpll.total_work() - 1).unwrap();
    assert_eq!(starved.outcome, Outcome::Unknown);
}

#[test]
fn corpus_has_exactly_120_distinct_cases_in_protocol_order() {
    let cases = corpus();
    assert_eq!(cases.len(), 120);
    let families: Vec<&str> = cases.iter().map(|case| case.family).collect();
    let counted = |family: &str| families.iter().filter(|f| **f == family).count();
    assert_eq!(counted("long-explanation"), 24);
    assert_eq!(counted("broken-explanation"), 24);
    assert_eq!(counted("immediate-pattern"), 12);
    assert_eq!(counted("parity-cycle"), 10);
    assert_eq!(counted("empty-control"), 2);
    assert_eq!(counted("random-2cnf"), 48);
    let mut order = families.clone();
    order.dedup();
    assert_eq!(
        order,
        [
            "long-explanation",
            "broken-explanation",
            "immediate-pattern",
            "parity-cycle",
            "empty-control",
            "random-2cnf"
        ]
    );
    for (index, case) in cases.iter().enumerate() {
        assert!(case.variables <= 12);
        assert!(case.input.iter().all(|clause| clause.len() <= 2));
        assert!(!cases[..index].iter().any(|previous| previous.id == case.id));
        assert!(
            !cases[..index].iter().any(
                |previous| previous.variables == case.variables && previous.input == case.input
            ),
            "{}",
            case.id
        );
    }
    assert_eq!(cases[0].id, "long-explanation-k2-pos-forward");
    assert_eq!(cases[0].input, vec![vec![1, 2], vec![-2, 3], vec![-3, 1]]);
    assert_eq!(cases[1].id, "long-explanation-k2-pos-reversed");
    assert_eq!(cases[1].input, vec![vec![-3, 1], vec![-2, 3], vec![1, 2]]);
    assert_eq!(cases[2].id, "long-explanation-k2-neg-forward");
    assert_eq!(cases[2].input, vec![vec![-1, 2], vec![-2, 3], vec![-3, -1]]);
    assert_eq!(cases[24].id, "broken-explanation-k2-pos-forward");
    assert_eq!(cases[24].input, vec![vec![1, 2], vec![-3, 1]]);
    let broken5 = &cases[24 + 8];
    assert_eq!(broken5.id, "broken-explanation-k5-pos-forward");
    assert_eq!(
        broken5.input,
        vec![
            vec![1, 2],
            vec![-2, 3],
            vec![-4, 5],
            vec![-5, 6],
            vec![-6, 1]
        ]
    );
    assert_eq!(cases[48].id, "immediate-pattern-n2-pos");
    assert_eq!(cases[49].id, "immediate-pattern-n2-neg");
    assert_eq!(cases[49].input, vec![vec![-1, 2], vec![-1, -2]]);
    assert_eq!(cases[60].id, "parity-cycle-n3");
    assert_eq!(cases[69].id, "parity-cycle-n12");
    assert_eq!(cases[70].id, "empty-formula");
    assert!(cases[70].input.is_empty() && cases[70].variables == 0);
    assert_eq!(cases[71].id, "empty-clause");
    assert_eq!(cases[71].input, vec![Vec::<i32>::new()]);
    assert_eq!(cases[72].id, "random-n6-d1-s2027");
    assert_eq!(cases[72].effective_seed, Some(2027 ^ (6 << 32) ^ (1 << 16)));
    assert_eq!(cases[72].input.len(), 6);
    assert_eq!(cases[119].id, "random-n12-d4-s999983");
    assert_eq!(cases[119].input.len(), 48);
    for case in &cases[72..] {
        for clause in &case.input {
            assert_eq!(clause.len(), 2);
            assert_ne!(clause[0].unsigned_abs(), clause[1].unsigned_abs());
        }
    }
    // Two draws by hand from the documented generator for the first clause.
    let mut x: u64 = 2027 ^ (6 << 32) ^ (1 << 16);
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let first = (next() % 6 + 1) as i32;
    let first = if next() & 1 == 0 { first } else { -first };
    assert_eq!(cases[72].input[0][0], first);
}

#[test]
fn experiment_passes_every_acceptance_check_and_reports_deterministically() {
    assert_eq!(PROTOCOL_SHA256.len(), 64);
    let first = experiment().unwrap();
    assert_eq!(first.summary.cases, 120);
    assert_eq!(first.mining.library.rules().len(), 4);
    assert_eq!(first.observations.len(), 120);
    assert_eq!(first.families.len(), 6);
    let summary = &first.summary;
    assert_eq!(summary.local.unknown, 0);
    assert_eq!(summary.dpll.unknown, 0);
    assert_eq!(summary.graph.decision_unknown, 0);
    assert_eq!(summary.graph.backbone_unknown, 0);
    assert_eq!(summary.reference_sat + summary.reference_unsat, 120);
    assert_eq!(summary.certificates_checked, 120);
    assert_eq!(summary.graph_coverage.cases, summary.reference_sat);
    assert_eq!(summary.graph_coverage.missed_literals, 0);
    assert!(summary.local_coverage.found_literals <= summary.local_coverage.reference_literals);
    let chains = &first.families["long-explanation"];
    assert_eq!(chains.local_coverage.found_literals, 0);
    assert_eq!(chains.local_coverage.reference_literals, 24);
    assert_eq!(chains.graph_coverage.found_literals, 24);
    let immediate = &first.families["immediate-pattern"];
    assert_eq!(immediate.local_coverage.missed_literals, 0);
    assert_eq!(immediate.local_coverage.reference_literals, 42);
    assert_eq!(first.families["parity-cycle"].reference_unsat, 5);
    let json_first = json(&first);
    let second = experiment().unwrap();
    assert_eq!(json_first, json(&second));
    assert!(json_first.contains("\"experiment\":\"implication-calibration-v1\""));
    assert!(json_first.contains(PROTOCOL_SHA256));
    assert!(summary_line(&first).contains("120 cases"));
}
