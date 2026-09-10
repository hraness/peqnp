use peqnp::{indexed, transfer, Cnf};

fn work_sum(work: &transfer::Work) -> u64 {
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

fn equivalent(
    input: &Cnf,
    variables: u32,
    learned: &transfer::FrozenLibrary,
    compiled: &indexed::CompiledLibrary,
) {
    let generic = transfer::solve(input, variables, Some(learned), indexed::ARM_BUDGET).unwrap();
    let indexed = indexed::solve(input, variables, compiled, indexed::ARM_BUDGET).unwrap();
    let expected = peqnp::truth_table_sat(input, variables).unwrap();
    assert_ne!(generic.outcome, transfer::Outcome::Unknown);
    assert_ne!(indexed.arm.outcome, transfer::Outcome::Unknown);
    assert!(indexed.arm.outcome.agrees(expected), "{input:?}");
    assert_eq!(indexed.arm.outcome, generic.outcome, "{input:?}");
    assert_eq!(
        indexed.arm.derived_units, generic.derived_units,
        "{input:?}"
    );
    assert_eq!(indexed.arm.residual, generic.residual, "{input:?}");
    assert_eq!(
        indexed.arm.preprocessing.work_units,
        work_sum(&indexed.arm.preprocessing)
    );
    assert_eq!(
        indexed.arm.residual.work_units,
        work_sum(&indexed.arm.residual)
    );
}

#[test]
fn compiler_consumes_four_actual_rules_and_empty_library_enables_nothing() {
    let learned = transfer::mine().unwrap().library;
    let compiled = indexed::compile(&learned, indexed::ARM_BUDGET).unwrap();
    assert_eq!(compiled.library.source_rule_instances(), 4);
    assert_eq!(compiled.library.schemas(), 1);
    assert_eq!(compiled.work.rule_attempts, 4);
    assert_eq!(compiled.work.work_units, work_sum(&compiled.work));
    assert!(matches!(
        indexed::compile(&learned, compiled.work.work_units - 1),
        Err(transfer::Failure::Budget)
    ));
    assert_eq!(
        indexed::compile(&learned, compiled.work.work_units)
            .unwrap()
            .work,
        compiled.work
    );
    let empty = indexed::compile(&transfer::FrozenLibrary::empty(), 0).unwrap();
    assert_eq!(empty.library.schemas(), 0);
    let input = vec![vec![-3, 2], vec![-2, -3]];
    assert!(indexed::solve(&input, 3, &empty.library, 10000)
        .unwrap()
        .arm
        .derived_units
        .is_empty());
    assert_eq!(
        indexed::solve(&input, 3, &compiled.library, 10000)
            .unwrap()
            .arm
            .derived_units,
        [-3]
    );
}

#[test]
fn index_matches_generic_on_every_ordered_pair_of_three_variable_canonical_clauses() {
    let clauses: Cnf = (0_u32..64)
        .map(|mask| {
            [1, -1, 2, -2, 3, -3]
                .iter()
                .enumerate()
                .filter_map(|(bit, &lit)| (mask & (1 << bit) != 0).then_some(lit))
                .collect()
        })
        .collect();
    let learned = transfer::mine().unwrap().library;
    let compiled = indexed::compile(&learned, indexed::ARM_BUDGET).unwrap();
    for a in &clauses {
        for b in &clauses {
            equivalent(&vec![a.clone(), b.clone()], 3, &learned, &compiled.library);
        }
    }
}

#[test]
fn all_ordered_binary_triples_preserve_multiple_unit_order_and_duplicate_behavior() {
    let mut clauses = Vec::new();
    for a in 1..=3 {
        for b in a + 1..=3 {
            for sa in [1, -1] {
                for sb in [1, -1] {
                    clauses.push(vec![a * sa, b * sb]);
                }
            }
        }
    }
    assert_eq!(clauses.len(), 12);
    let learned = transfer::mine().unwrap().library;
    let compiled = indexed::compile(&learned, indexed::ARM_BUDGET).unwrap();
    for a in &clauses {
        for b in &clauses {
            for c in &clauses {
                equivalent(
                    &vec![a.clone(), b.clone(), c.clone()],
                    3,
                    &learned,
                    &compiled.library,
                );
            }
        }
    }
    // Multiple pivots for one shared literal, interleaved witnesses, and
    // reversed literal order force nontrivial sorting and block reduction.
    let input = vec![
        vec![4, 3],
        vec![-4, 3],
        vec![-1, 2],
        vec![2, 1],
        vec![1, 3],
        vec![3, -1],
        vec![2, 1],
        vec![3, 4],
    ];
    for shift in 0..input.len() {
        let mut permuted = input.clone();
        permuted.rotate_left(shift);
        for clause in &mut permuted {
            clause.reverse();
        }
        equivalent(&permuted, 4, &learned, &compiled.library);
    }
}

#[test]
fn index_arrays_and_each_sort_swap_are_explicitly_charged() {
    let learned = transfer::mine().unwrap().library;
    let compiled = indexed::compile(&learned, indexed::ARM_BUDGET).unwrap();
    let input = vec![vec![3, 2], vec![3, -2], vec![1, 2, 3], vec![2, -2]];
    let result = indexed::solve(&input, 3, &compiled.library, indexed::ARM_BUDGET).unwrap();
    assert_eq!(result.index.entries, 4);
    assert_eq!(result.index.peak_stored_scalar_cells, 16); // Four triples + one witness triple + one unit.
    assert!(result.index.entry_swaps > 0);
    assert!(result.index.group_lookups >= result.index.entries);
    assert_eq!(
        result.arm.preprocessing.pair_checks,
        result.index.key_comparisons
    );
    assert!(
        result.arm.preprocessing.literal_writes
            >= result.index.entries * 3 + result.index.entry_swaps * 6
    );
    assert_eq!(
        result.arm.preprocessing.work_units,
        work_sum(&result.arm.preprocessing)
    );
    let no_binary = indexed::solve(
        &vec![vec![1, 2, 3]],
        3,
        &compiled.library,
        indexed::ARM_BUDGET,
    )
    .unwrap();
    assert_eq!(no_binary.index.entries, 0);
    assert_eq!(no_binary.index.peak_stored_scalar_cells, 0);
    assert!(no_binary.arm.derived_units.is_empty());
}

#[test]
fn preprocessing_and_residual_share_budget_and_never_relabel_exhaustion() {
    let compiled =
        indexed::compile(&transfer::mine().unwrap().library, indexed::ARM_BUDGET).unwrap();
    let input = vec![vec![1, 2], vec![1, -2]];
    let full = indexed::solve(&input, 2, &compiled.library, indexed::ARM_BUDGET).unwrap();
    for budget in [
        0,
        1,
        10,
        full.arm.preprocessing.work_units - 1,
        full.arm.preprocessing.work_units,
        full.arm.total_work() - 1,
    ] {
        let partial = indexed::solve(&input, 2, &compiled.library, budget).unwrap();
        assert_eq!(partial.arm.outcome, transfer::Outcome::Unknown);
        assert_eq!(partial.arm.total_work(), budget);
        assert_eq!(
            partial.arm.preprocessing.work_units,
            work_sum(&partial.arm.preprocessing)
        );
    }
    let exact = indexed::solve(&input, 2, &compiled.library, full.arm.total_work()).unwrap();
    assert_eq!(exact.arm.outcome, full.arm.outcome);
    assert_eq!(exact.arm.residual, full.arm.residual);
}

#[test]
fn empty_malformed_and_nonbinary_inputs_keep_their_semantics() {
    let learned = transfer::mine().unwrap().library;
    let compiled = indexed::compile(&learned, indexed::ARM_BUDGET).unwrap();
    for input in [
        vec![],
        vec![vec![]],
        vec![vec![1]],
        vec![vec![1], vec![-1]],
        vec![vec![2, 2], vec![2, -2]],
        vec![vec![1, 2, 3], vec![-1, -2, -3]],
    ] {
        equivalent(&input, 3, &learned, &compiled.library);
    }
    for input in [vec![vec![0]], vec![vec![i32::MIN]], vec![vec![4]]] {
        assert!(matches!(
            indexed::solve(&input, 3, &compiled.library, 1000),
            Err(transfer::Failure::InvalidInput)
        ));
    }
    assert!(matches!(
        indexed::solve(&vec![], 13, &compiled.library, 1000),
        Err(transfer::Failure::InvalidInput)
    ));
}
