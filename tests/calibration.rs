use peqnp::*;

#[test]
fn parser_enforces_types_scope_arity_and_roundtrip() {
    for source in grammar() {
        let parsed = parse(&source).unwrap();
        assert_eq!(parsed.to_string(), source);
    }
    for source in [
        "unit",
        "(rewrite input unit true true)",
        "(with-unit input (rewrite input input true true))",
        "(with-unit input (rewrite input unit input true))",
        "(with-unit input)",
        "(with-unit input input",
        "input input",
    ] {
        assert!(
            parse(source).is_err(),
            "accepted ill-typed program: {source}"
        );
    }
}

#[test]
fn exhaustion_is_unknown_and_never_a_formula() {
    let ast = parse(UNIT_RULE).unwrap();
    let input = vec![vec![1], vec![-1, 2]];
    let full = evaluate(&ast, &input, 100_000);
    assert!(full.result.is_ok());
    for fuel in [0, 1, full.cost.work_units - 1] {
        let limited = evaluate(&ast, &input, fuel);
        assert_eq!(limited.result, Err(EvalError::UnknownBudget));
        assert_eq!(limited.cost.work_units, fuel);
    }
    assert_eq!(evaluate(&ast, &input, full.cost.work_units), full);
}

#[test]
fn empty_and_contradictory_inputs_have_explicit_semantics() {
    let ast = parse(UNIT_RULE).unwrap();
    for input in [vec![], vec![vec![]], vec![vec![1], vec![-1]]] {
        let output = evaluate(&ast, &input, 100_000).result.unwrap();
        assert_eq!(
            truth_table_sat(&input, 2).unwrap(),
            truth_table_sat(&output, 2).unwrap()
        );
    }
    assert!(truth_table_sat(&vec![], 0).unwrap());
    assert!(!truth_table_sat(&vec![vec![]], 0).unwrap());
    assert_eq!(
        evaluate(&ast, &vec![vec![0]], 100).result,
        Err(EvalError::InvalidLiteral(0))
    );
    assert_eq!(
        evaluate(&ast, &vec![vec![i32::MIN]], 100).result,
        Err(EvalError::InvalidLiteral(i32::MIN))
    );
}

#[test]
fn bounded_oracle_rejects_out_of_domain_inputs() {
    assert!(truth_table_sat(&vec![vec![3]], 2).is_err());
    assert!(truth_table_sat(&vec![], 21).is_err());
}

#[test]
fn exhaustive_calibration_recovers_rule_and_reports_real_failures() {
    let records = calibration().unwrap();
    let survivors: Vec<_> = records
        .iter()
        .filter(|r| r.preservation_passes == 65_536 && r.progress_passes == 65_536)
        .collect();
    assert_eq!(survivors.len(), 1);
    assert_eq!(survivors[0].program, UNIT_RULE);
    assert!(records.iter().any(|r| r.first_counterexample.is_some()));
    let identity = records
        .iter()
        .find(|r| r.program == "(with-unit input (rewrite input unit false false))")
        .unwrap();
    assert_eq!(identity.preservation_passes, 65_536);
    assert!(identity.progress_passes < 65_536);
    assert!(result_json(&records).unwrap().contains("\"rejected\":true"));
}

#[test]
fn unit_propagation_is_not_a_complete_decider() {
    let input = false_completeness_example();
    assert!(!truth_table_sat(&input, 2).unwrap());
    assert!(input.iter().all(|clause| clause.len() == 2));
    assert_eq!(
        evaluate(&parse(UNIT_RULE).unwrap(), &input, 100_000)
            .result
            .unwrap(),
        input
    );
}
