//! Focused correctness tests from the frozen fragment-interface protocol.
//! These are implementation checks, distinct from the 162 reported cases.

use peqnp::fragment::*;
use peqnp::implication::Certificate;
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

/// The eight ternary clauses on variables 1, 2, 3.
fn ternary(signs: u32) -> Vec<i32> {
    (0..3_i32)
        .map(|bit| {
            let variable = bit + 1;
            if signs & (1 << bit) == 0 {
                variable
            } else {
                -variable
            }
        })
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

fn phases(arm: &FragmentArm) -> [&Work; 6] {
    [
        &arm.construction,
        &arm.components,
        &arm.decision_certificate,
        &arm.extraction,
        &arm.append,
        &arm.residual,
    ]
}

/// Compare the fragment arm and the baseline against the truth table and the
/// crate's oldest oracle, and run the raw checkers on all reported evidence.
fn agrees_with_reference(input: &Cnf, n: u32) -> FragmentArm {
    let reference = reference(input, n).unwrap();
    let oracle = peqnp::truth_table_sat(input, n).unwrap();
    assert_eq!(reference.model_count > 0, oracle, "{input:?}");
    let baseline = transfer::solve(input, n, None, ARM_BUDGET).unwrap();
    assert_ne!(baseline.outcome, Outcome::Unknown);
    assert!(baseline.outcome.agrees(oracle), "{input:?}");
    let arm = solve_fragment(input, n, ARM_BUDGET).unwrap();
    assert_ne!(arm.outcome, Outcome::Unknown, "{input:?}");
    assert!(arm.outcome.agrees(oracle), "{input:?}");
    let certificate = arm.certificate.as_ref().expect("full budget completes");
    let (valid, _) = check_certificate(input, n, certificate).unwrap();
    assert!(valid, "{input:?}");
    let found: Vec<i32> = arm.clues.iter().map(|clue| clue.literal).collect();
    assert_eq!(found, arm.derived_units);
    for clue in &arm.clues {
        let (valid, _) = check_clue(input, n, clue).unwrap();
        assert!(valid, "{input:?} {clue:?}");
        assert_eq!(clue.path.nodes.first(), Some(&-clue.literal));
        assert_eq!(clue.path.nodes.last(), Some(&clue.literal));
        // Every cited clause is a width-at-most-two clause of the original input.
        for &index in &clue.path.clauses {
            assert!(input[index].len() <= 2);
        }
    }
    if matches!(certificate, Certificate::Sat(_)) {
        // The residual solver decided the whole formula on the processed copy.
        assert!(arm.residual.work_units > 0);
        assert_eq!(
            arm.append.clause_writes as usize,
            input.len() + arm.clues.len()
        );
    } else {
        assert_eq!(arm.outcome, Outcome::Unsat);
        assert!(!oracle);
        assert!(arm.clues.is_empty());
        assert_eq!(arm.extraction.work_units, 0);
        assert_eq!(arm.append.work_units, 0);
        assert_eq!(arm.residual.work_units, 0);
    }
    if let Some(backbone) = &reference.backbone {
        for unit in &arm.derived_units {
            assert!(backbone.contains(unit), "{input:?} {unit}");
        }
    }
    let mut total = 0;
    for phase in phases(&arm) {
        assert_eq!(phase.work_units, work_sum(phase));
        total += phase.work_units;
    }
    assert_eq!(total, arm.total_work());
    assert_eq!(arm.total.work_units, work_sum(&arm.total));
    assert!(arm.total_work() <= ARM_BUDGET);
    arm
}

#[test]
fn all_4096_canonical_subsets_with_a_ternary_clause_match_the_truth_table() {
    let mut checked = 0;
    let mut fragment_sat_whole_unsat = 0;
    for mask in 0..512 {
        for signs in 0..8 {
            let mut input = subset(mask);
            input.push(ternary(signs));
            let arm = agrees_with_reference(&input, 3);
            if matches!(arm.certificate, Some(Certificate::Sat(_))) && arm.outcome == Outcome::Unsat
            {
                fragment_sat_whole_unsat += 1;
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 4096);
    // Variable 3 occurs only in the ternary clause, so a satisfiable fragment
    // always extends to a model here; the mismatch case has its own test.
    assert_eq!(fragment_sat_whole_unsat, 0);
    // The ternary clause placed first shifts every original index by one.
    for mask in 0..512 {
        for signs in 0..8 {
            let mut input = vec![ternary(signs)];
            input.extend(subset(mask));
            let arm = agrees_with_reference(&input, 3);
            for clue in &arm.clues {
                assert!(clue.path.clauses.iter().all(|&index| index >= 1));
            }
        }
    }
}

#[test]
fn fragment_satisfiable_but_whole_unsatisfiable_is_decided_by_the_residual() {
    let mut input = chain(2);
    input.extend((0..8).map(ternary));
    let arm = agrees_with_reference(&input, 3);
    assert!(matches!(arm.certificate, Some(Certificate::Sat(_))));
    assert_eq!(arm.derived_units, vec![1]);
    assert_eq!(arm.outcome, Outcome::Unsat);
    assert!(arm.residual.search_nodes > 0);
    // The fragment's own certificate satisfies only its width-at-most-two clauses.
    let Some(Certificate::Sat(values)) = &arm.certificate else {
        panic!("expected a fragment SAT certificate");
    };
    assert!(values[0]);
    assert!(
        check_certificate(&input, 3, &Certificate::Sat(values.clone()))
            .unwrap()
            .0
    );
    let mut flipped = values.clone();
    flipped[0] = false;
    assert!(
        !check_certificate(&input, 3, &Certificate::Sat(flipped))
            .unwrap()
            .0
    );
}

#[test]
fn fragment_forced_literal_lies_in_the_reference_backbone() {
    let mut input = chain(3);
    input.push(vec![2, -3, 4]);
    input.push(vec![-1, 3, -4]);
    let reference = reference(&input, 4).unwrap();
    let backbone = reference.backbone.clone().expect("satisfiable");
    assert!(backbone.contains(&1));
    let arm = agrees_with_reference(&input, 4);
    assert_eq!(arm.outcome, Outcome::Sat);
    assert_eq!(arm.derived_units, vec![1]);
    assert_eq!(arm.clues[0].path.nodes, vec![-1, 2, 3, 4, 1]);
    assert_eq!(arm.clues[0].path.clauses, vec![0, 1, 2, 3]);
    // The negated chain forces the opposite literal, still inside the backbone.
    let negated: Cnf = input
        .iter()
        .map(|clause| {
            clause
                .iter()
                .map(|&lit| if lit.abs() == 1 { -lit } else { lit })
                .collect()
        })
        .collect();
    let arm = agrees_with_reference(&negated, 4);
    assert_eq!(arm.derived_units, vec![-1]);
    // A tampered clue citing a ternary clause is rejected by the raw checker.
    let mut tampered = arm.clues[0].clone();
    tampered.path.clauses[0] = 4;
    assert!(!check_clue(&negated, 4, &tampered).unwrap().0);
}

#[test]
fn empty_and_malformed_inputs_keep_their_semantics_or_are_rejected() {
    let empty = solve_fragment(&vec![], 0, ARM_BUDGET).unwrap();
    assert_eq!(empty.outcome, Outcome::Sat);
    assert_eq!(empty.certificate, Some(Certificate::Sat(vec![])));
    assert!(empty.derived_units.is_empty());
    let empty_clause = solve_fragment(&vec![vec![]], 0, ARM_BUDGET).unwrap();
    assert_eq!(empty_clause.outcome, Outcome::Unsat);
    assert_eq!(empty_clause.certificate, Some(Certificate::EmptyClause(0)));
    assert_eq!(empty_clause.residual.work_units, 0);
    let ternary_only = agrees_with_reference(&vec![vec![1, 2, 3], vec![-1, -2, -3]], 3);
    assert!(ternary_only.derived_units.is_empty());
    // Without edges every positive vertex finishes before its negation, so the
    // fragment assignment is all-true.
    assert_eq!(
        ternary_only.certificate,
        Some(Certificate::Sat(vec![true; 3]))
    );
    assert!(ternary_only.construction.work_units > 0);
    agrees_with_reference(&vec![vec![1, 1], vec![-1, -1, 2]], 2);
    agrees_with_reference(&vec![vec![1, -1], vec![2, -2, 1]], 2);
    for input in [
        vec![vec![0]],
        vec![vec![i32::MIN]],
        vec![vec![1, 2, 3, 4]],
        vec![vec![4]],
        vec![vec![1, 2], vec![3, -1, 0]],
    ] {
        assert_eq!(
            solve_fragment(&input, 3, ARM_BUDGET).map(|_| ()),
            Err(Failure::InvalidInput),
            "{input:?}"
        );
        assert_eq!(reference(&input, 3).map(|_| ()), Err(Failure::InvalidInput));
        assert_eq!(
            check_certificate(&input, 3, &Certificate::Sat(vec![true; 3])).map(|_| ()),
            Err(Failure::InvalidInput)
        );
    }
    assert_eq!(
        solve_fragment(&vec![], 13, ARM_BUDGET).map(|_| ()),
        Err(Failure::InvalidInput)
    );
    assert_eq!(
        reference(&vec![], 13).map(|_| ()),
        Err(Failure::InvalidInput)
    );
}

#[test]
fn budget_exhaustion_at_each_phase_boundary_stays_unknown() {
    let n = 6;
    let mut input = chain(5);
    input.push(vec![2, -4, 6]);
    input.push(vec![-3, 5, -6]);
    let full = solve_fragment(&input, n, ARM_BUDGET).unwrap();
    assert_eq!(full.outcome, Outcome::Sat);
    assert_eq!(full.derived_units, vec![1]);
    let mut boundaries = Vec::new();
    let mut running = 0;
    for phase in phases(&full) {
        assert!(phase.work_units > 0);
        running += phase.work_units;
        boundaries.push(running);
    }
    assert_eq!(running, full.total_work());
    for (index, &boundary) in boundaries.iter().enumerate() {
        for budget in [boundary - 1, boundary] {
            if budget == full.total_work() {
                continue;
            }
            let arm = solve_fragment(&input, n, budget).unwrap();
            assert_eq!(arm.outcome, Outcome::Unknown, "budget {budget}");
            assert!(arm.certificate.is_none());
            assert!(arm.clues.is_empty());
            assert!(arm.derived_units.is_empty());
            assert_eq!(arm.total_work(), budget);
            let mut spent = 0;
            for (position, phase) in phases(&arm).iter().enumerate() {
                if position > index + usize::from(budget == boundary) {
                    assert_eq!(phase.work_units, 0);
                }
                spent += phase.work_units;
            }
            assert_eq!(spent, budget);
        }
    }
    for budget in [0, 1] {
        let arm = solve_fragment(&input, n, budget).unwrap();
        assert_eq!(arm.outcome, Outcome::Unknown);
        assert_eq!(arm.total_work(), budget);
    }
    let exact = solve_fragment(&input, n, full.total_work()).unwrap();
    assert_eq!(exact.outcome, Outcome::Sat);
    assert_eq!(exact.certificate, full.certificate);
    assert_eq!(exact.clues, full.clues);
    assert_eq!(exact.residual, full.residual);

    // A contradictory fragment decides at the certificate boundary and never
    // relabels exhaustion as UNSAT.
    let mut cycle: Cnf = (1..=3)
        .flat_map(|i| {
            let next = i % 3 + 1;
            [vec![i, next], vec![-i, -next]]
        })
        .collect();
    cycle.push(vec![1, 2, 3]);
    let unsat = solve_fragment(&cycle, 3, ARM_BUDGET).unwrap();
    assert_eq!(unsat.outcome, Outcome::Unsat);
    assert!(matches!(
        unsat.certificate,
        Some(Certificate::OppositePaths { .. })
    ));
    assert_eq!(unsat.residual.work_units, 0);
    assert_eq!(unsat.total_work(), unsat.fragment_work());
    for budget in [0, unsat.construction.work_units, unsat.total_work() - 1] {
        let arm = solve_fragment(&cycle, 3, budget).unwrap();
        assert_eq!(arm.outcome, Outcome::Unknown);
        assert!(arm.certificate.is_none());
        assert_eq!(arm.total_work(), budget);
    }
    assert_eq!(
        solve_fragment(&cycle, 3, unsat.total_work())
            .unwrap()
            .outcome,
        Outcome::Unsat
    );
}

#[test]
fn corpus_has_exactly_162_distinct_cases_in_protocol_order() {
    let cases = corpus().unwrap();
    assert_eq!(cases.len(), 162);
    let families: Vec<&str> = cases.iter().map(|case| case.family).collect();
    let counted = |family: &str| families.iter().filter(|f| **f == family).count();
    assert_eq!(counted("random-ternary-only"), 36);
    assert_eq!(counted("random-mixed"), 108);
    assert_eq!(counted("chain-embedded"), 12);
    assert_eq!(counted("contradictory-fragment"), 6);
    let mut order = families.clone();
    order.dedup();
    // Each (seed, n, t) block emits the b = 0 control and then three mixed cases.
    let expected: Vec<&str> = std::iter::repeat_n(["random-ternary-only", "random-mixed"], 36)
        .flatten()
        .chain(["chain-embedded", "contradictory-fragment"])
        .collect();
    assert_eq!(order, expected);
    for (index, case) in cases.iter().enumerate() {
        assert!(case.variables <= 12);
        assert!(case.input.iter().all(|clause| clause.len() <= 3));
        assert!(case.input.iter().all(|clause| clause
            .iter()
            .all(|lit| *lit != 0 && lit.unsigned_abs() <= case.variables)));
        assert!(!cases[..index].iter().any(|previous| previous.id == case.id));
        assert!(
            !cases[..index].iter().any(
                |previous| previous.variables == case.variables && previous.input == case.input
            ),
            "{}",
            case.id
        );
    }
    let first = &cases[0];
    assert_eq!(first.id, "random-n8-t3-b0-s3001");
    assert_eq!(first.family, "random-ternary-only");
    assert_eq!(first.effective_seed, Some(3001 ^ (8 << 32) ^ (3 << 40)));
    assert_eq!(first.input.len(), 24);
    assert!(first.input.iter().all(|clause| clause.len() == 3));
    let second = &cases[1];
    assert_eq!(second.id, "random-n8-t3-b4-s3001");
    assert_eq!(second.family, "random-mixed");
    assert_eq!(second.binary_label, Some("b=n/2"));
    assert_eq!(
        second.effective_seed,
        Some(3001 ^ (8 << 32) ^ (3 << 40) ^ (4 << 48))
    );
    assert_eq!(second.input.len(), 28);
    assert!(second.input[..24].iter().all(|clause| clause.len() == 3));
    assert!(second.input[24..].iter().all(|clause| clause.len() == 2));
    assert_eq!(cases[3].id, "random-n8-t3-b16-s3001");
    assert_eq!(cases[143].id, "random-n12-t5-b24-s24001");
    for case in &cases[..144] {
        for clause in &case.input {
            for (i, a) in clause.iter().enumerate() {
                for b in &clause[i + 1..] {
                    assert_ne!(a.unsigned_abs(), b.unsigned_abs(), "{}", case.id);
                }
            }
        }
    }
    // Three draws by hand from the documented generator for the first clause.
    let mut x: u64 = 3001 ^ (8 << 32) ^ (3 << 40);
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let variable = (next() % 8 + 1) as i32;
    let literal = if next() & 1 == 0 { variable } else { -variable };
    assert_eq!(first.input[0][0], literal);

    let chain_case = &cases[144];
    assert_eq!(chain_case.id, "chain-n8-t3-pos");
    assert_eq!(chain_case.chain_length, Some(7));
    assert_eq!(chain_case.input[..8], chain(7)[..]);
    assert_eq!(chain_case.input.len(), 8 + 24);
    assert_eq!(
        chain_case.effective_seed,
        Some(3001 ^ (8 << 32) ^ (3 << 40) ^ (8 << 48))
    );
    let negative = &cases[145];
    assert_eq!(negative.id, "chain-n8-t3-neg");
    assert_eq!(negative.input[0], vec![-1, 2]);
    assert_eq!(negative.input[7], vec![-8, -1]);
    assert_eq!(negative.input[8..], chain_case.input[8..]);
    assert_eq!(cases[155].id, "chain-n12-t5-neg");
    let contradictory = &cases[156];
    assert_eq!(contradictory.id, "contradictory-n8-m3-t3");
    assert_eq!(contradictory.cycle_length, Some(3));
    assert_eq!(
        contradictory.input[..6],
        [
            vec![1, 2],
            vec![-1, -2],
            vec![2, 3],
            vec![-2, -3],
            vec![3, 1],
            vec![-3, -1]
        ]
    );
    assert_eq!(contradictory.input.len(), 6 + 24);
    assert_eq!(
        contradictory.effective_seed,
        Some(6007 ^ (8 << 32) ^ (3 << 40) ^ (6 << 48))
    );
    assert_eq!(cases[161].id, "contradictory-n12-m7-t5");
    assert_eq!(cases[161].input.len(), 14 + 60);
}

#[test]
fn experiment_passes_every_acceptance_check_and_reports_deterministically() {
    assert_eq!(PROTOCOL_SHA256.len(), 64);
    let first = experiment().unwrap();
    assert_eq!(first.summary.cases, 162);
    assert_eq!(first.observations.len(), 162);
    assert_eq!(first.families.len(), 4);
    assert_eq!(first.binary_counts.len(), 3);
    let summary = &first.summary;
    assert_eq!(summary.reference_sat + summary.reference_unsat, 162);
    assert_eq!(summary.baseline.unknown, 0);
    assert_eq!(summary.fragment.unknown, 0);
    assert_eq!(summary.certificates_checked, 162);
    assert_eq!(summary.clues_checked, summary.fragment.derived_units);
    assert_eq!(summary.fragment_vs_baseline.both_complete, 162);
    assert_eq!(
        first.families["random-ternary-only"].fragment.derived_units,
        0
    );
    assert_eq!(first.families["chain-embedded"].fragment.derived_units, 12);
    let contradictory = &first.families["contradictory-fragment"];
    assert_eq!(contradictory.fragment.unsat, 6);
    assert_eq!(contradictory.fragment.fragment_unsat, 6);
    assert_eq!(contradictory.fragment.residual_work_units, 0);
    assert_eq!(contradictory.reference_unsat, 6);
    for row in &first.observations {
        assert_eq!(
            row.fragment.total_work(),
            row.fragment.fragment_work() + row.fragment.residual.work_units
        );
    }
    let json_first = json(&first);
    let second = experiment().unwrap();
    assert_eq!(json_first, json(&second));
    assert!(json_first.contains("\"experiment\":\"fragment-interface-v1\""));
    assert!(json_first.contains(PROTOCOL_SHA256));
    assert!(json_first.contains("\"random_mixed_by_binary_count\":{\"b=2n\":"));
    assert!(summary_line(&first).contains("162 cases"));
}
