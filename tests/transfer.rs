use peqnp::{transfer::*, Cnf};

#[test]
fn mining_tests_every_candidate_and_freezes_four_sound_rule_instances() {
    let mined = mine().unwrap();
    assert_eq!(mined.candidates.len(), 24);
    assert_eq!(mined.library.rules().len(), 4);
    assert_eq!(mined.work.assignments, 96);
    for candidate in mined.candidates {
        assert!(candidate.satisfying_assignments > 0);
        assert_eq!(
            candidate.entailed,
            candidate.first_counterexample_assignment.is_none()
        );
        let premise: Cnf = candidate
            .rule
            .premises
            .iter()
            .map(|clause| clause.to_vec())
            .collect();
        let mut contradicted = premise.clone();
        contradicted.push(vec![-candidate.rule.conclusion]);
        assert_eq!(
            candidate.entailed,
            !peqnp::truth_table_sat(&contradicted, 2).unwrap()
        );
    }
}

#[test]
fn dpll_matches_independent_reference_on_all_three_variable_two_clause_inputs() {
    let clauses: Vec<Vec<i32>> = (0_u32..64)
        .map(|mask| {
            [1, -1, 2, -2, 3, -3]
                .iter()
                .enumerate()
                .filter_map(|(bit, &lit)| (mask & (1 << bit) != 0).then_some(lit))
                .collect()
        })
        .collect();
    let learned = mine().unwrap().library;
    for a in &clauses {
        for b in &clauses {
            let input = vec![a.clone(), b.clone()];
            let expected = peqnp::truth_table_sat(&input, 3).unwrap();
            for library in [None, Some(&learned)] {
                let arm = solve(&input, 3, library, ARM_BUDGET).unwrap();
                assert_ne!(arm.outcome, Outcome::Unknown);
                assert!(arm.outcome.agrees(expected), "{input:?}");
            }
        }
    }
}

#[test]
fn matching_consumes_library_and_handles_signed_injective_renaming() {
    let learned = mine().unwrap().library;
    let empty = FrozenLibrary::empty();
    // Neither premise is a unit. A sign-renamed known rule entails -5.
    let input = vec![vec![-5, 3], vec![-3, -5]];
    let transfer = solve(&input, 5, Some(&learned), ARM_BUDGET).unwrap();
    assert_eq!(transfer.derived_units, vec![-5]);
    assert_eq!(transfer.outcome, Outcome::Sat);
    assert!(solve(&input, 5, Some(&empty), ARM_BUDGET)
        .unwrap()
        .derived_units
        .is_empty());
    let tautology = vec![vec![2, -2], vec![2, -2]];
    assert!(solve(&tautology, 2, Some(&learned), ARM_BUDGET)
        .unwrap()
        .derived_units
        .is_empty());
}

#[test]
fn both_arms_share_one_total_budget_and_exhaustion_stays_unknown() {
    let learned = mine().unwrap().library;
    let input = vec![vec![1, 2], vec![1, -2]];
    let full = solve(&input, 2, Some(&learned), ARM_BUDGET).unwrap();
    for budget in [0, 1, full.preprocessing.work_units, full.total_work() - 1] {
        let arm = solve(&input, 2, Some(&learned), budget).unwrap();
        assert_eq!(arm.outcome, Outcome::Unknown);
        assert_eq!(arm.total_work(), budget);
    }
    let exact = solve(&input, 2, Some(&learned), full.total_work()).unwrap();
    assert_eq!(exact.outcome, Outcome::Sat);
    assert_eq!(solve(&input, 2, None, 0).unwrap().outcome, Outcome::Unknown);
}

#[test]
fn malformed_and_empty_formulas_are_distinct_from_unsat_or_budget() {
    assert_eq!(solve(&vec![], 0, None, 100).unwrap().outcome, Outcome::Sat);
    assert_eq!(
        solve(&vec![vec![]], 0, None, 100).unwrap().outcome,
        Outcome::Unsat
    );
    for input in [vec![vec![0]], vec![vec![i32::MIN]], vec![vec![3]]] {
        assert!(matches!(
            solve(&input, 2, None, 100),
            Err(Failure::InvalidInput)
        ));
        assert!(reference(&input, 2).is_err());
    }
    assert!(reference(&vec![], 13).is_err());
}

#[test]
fn corpus_is_fixed_and_no_binary_controls_do_not_gain_clues() {
    let cases = corpus();
    assert_eq!(cases.len(), 122);
    let learned = mine().unwrap().library;
    for case in cases {
        assert!((5..=12).contains(&case.variables));
        if [
            "hidden-backbone",
            "ternary-cube",
            "parity-cycle",
            "pigeonhole",
        ]
        .contains(&case.family)
        {
            let arm = solve(&case.input, case.variables, Some(&learned), ARM_BUDGET).unwrap();
            assert!(arm.derived_units.is_empty(), "{}", case.id);
            assert_ne!(arm.outcome, Outcome::Unknown);
            let expected = peqnp::truth_table_sat(&case.input, case.variables).unwrap();
            assert!(arm.outcome.agrees(expected));
            if case.family == "hidden-backbone" {
                assert!(expected);
            }
            if case.family == "ternary-cube" {
                assert!(!expected);
            }
        }
    }
}

#[test]
fn original_report_rejects_public_program_strings_and_invalid_coverage() {
    let mut records: Vec<_> = peqnp::grammar()
        .into_iter()
        .map(|program| peqnp::CandidateResult {
            preservation_passes: 65_536,
            progress_passes: if program == peqnp::UNIT_RULE {
                65_536
            } else {
                0
            },
            program,
            checked_formulas: 65_536,
            first_counterexample: None,
            work_units: 1,
        })
        .collect();
    assert!(peqnp::result_json(&records).is_ok());
    let original = records[0].program.clone();
    records[0].program = "\"untrusted-public-input".into();
    assert!(peqnp::result_json(&records).is_err());
    records[0].program = original;
    records[0].checked_formulas = 1;
    assert!(peqnp::result_json(&records).is_err());
}
