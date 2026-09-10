//! Focused correctness tests from the frozen extraction-cost protocol.
//! These are implementation checks, distinct from the reported cases.

use peqnp::extraction::*;
use peqnp::fragment::{self, FragmentArm};
use peqnp::implication::Certificate;
use peqnp::transfer::{Failure, Outcome, Work};
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

/// `F_k` over x=1 and y_i=i+1.
fn chain(k: u32) -> Cnf {
    let y = |i: u32| (i + 1) as i32;
    let mut clauses = vec![vec![1, y(1)]];
    for i in 1..k {
        clauses.push(vec![-y(i), y(i + 1)]);
    }
    clauses.push(vec![-y(k), 1]);
    clauses
}

fn negated(input: &Cnf, variable: u32) -> Cnf {
    input
        .iter()
        .map(|clause| {
            clause
                .iter()
                .map(|&lit| {
                    if lit.unsigned_abs() == variable {
                        -lit
                    } else {
                        lit
                    }
                })
                .collect()
        })
        .collect()
}

fn parity_cycle(n: u32) -> Cnf {
    (1..=n)
        .flat_map(|i| {
            let a = i as i32;
            let b = (i % n + 1) as i32;
            [vec![a, b], vec![-a, -b]]
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

fn sorted(literals: &[i32]) -> Vec<i32> {
    let mut result = literals.to_vec();
    result.sort_unstable();
    result
}

fn check_arm(input: &Cnf, n: u32, arm: &FragmentArm, oracle: bool, backbone: Option<&Vec<i32>>) {
    assert_ne!(arm.outcome, Outcome::Unknown, "{input:?}");
    assert!(arm.outcome.agrees(oracle), "{input:?}");
    let certificate = arm.certificate.as_ref().expect("full budget completes");
    assert!(
        fragment::check_certificate(input, n, certificate)
            .unwrap()
            .0,
        "{input:?}"
    );
    let found: Vec<i32> = arm.clues.iter().map(|clue| clue.literal).collect();
    assert_eq!(found, arm.derived_units);
    for clue in &arm.clues {
        assert!(fragment::check_clue(input, n, clue).unwrap().0, "{input:?}");
        assert_eq!(clue.path.nodes.first(), Some(&-clue.literal));
        assert_eq!(clue.path.nodes.last(), Some(&clue.literal));
        assert_eq!(clue.path.nodes.len(), clue.path.clauses.len() + 1);
        for &index in &clue.path.clauses {
            assert!(input[index].len() <= 2);
        }
        // Reported paths are simple.
        let mut nodes = clue.path.nodes.clone();
        nodes.sort_unstable();
        nodes.dedup();
        assert_eq!(nodes.len(), clue.path.nodes.len(), "{input:?}");
    }
    if matches!(certificate, Certificate::Sat(_)) {
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
    if let Some(backbone) = backbone {
        for unit in &arm.derived_units {
            assert!(backbone.contains(unit), "{input:?}");
        }
    }
    let mut total = 0;
    for phase in phases(arm) {
        assert_eq!(phase.work_units, work_sum(phase));
        total += phase.work_units;
    }
    assert_eq!(total, arm.total_work());
    assert!(arm.total_work() <= ARM_BUDGET);
}

/// Run both extractions with the full budget, compare them against the
/// truth table and each other, and run the raw checkers on all evidence.
fn both(input: &Cnf, n: u32) -> (FragmentArm, SettledArm) {
    let reference = fragment::reference(input, n).unwrap();
    let oracle = peqnp::truth_table_sat(input, n).unwrap();
    assert_eq!(reference.model_count > 0, oracle, "{input:?}");
    let v1 = fragment::solve_fragment(input, n, ARM_BUDGET).unwrap();
    let per_literal = solve_per_literal(input, n, ARM_BUDGET).unwrap();
    assert!(same_arm(&per_literal, &v1), "{input:?}");
    let settled = solve_settled(input, n, ARM_BUDGET).unwrap();
    check_arm(input, n, &per_literal, oracle, reference.backbone.as_ref());
    check_arm(input, n, &settled.arm, oracle, reference.backbone.as_ref());
    // Identical unit sets, identical ordered units, identical processed
    // formulas, and identical counters in every phase except extraction.
    assert_eq!(
        sorted(&per_literal.derived_units),
        sorted(&settled.arm.derived_units),
        "{input:?}"
    );
    assert_eq!(per_literal.derived_units, settled.arm.derived_units);
    assert_eq!(per_literal.outcome, settled.arm.outcome);
    assert_eq!(per_literal.certificate, settled.arm.certificate);
    assert_eq!(per_literal.construction, settled.arm.construction);
    assert_eq!(per_literal.components, settled.arm.components);
    assert_eq!(
        per_literal.decision_certificate,
        settled.arm.decision_certificate
    );
    assert_eq!(per_literal.append, settled.arm.append);
    assert_eq!(per_literal.residual, settled.arm.residual);
    let extracted = matches!(settled.arm.certificate, Some(Certificate::Sat(_)));
    assert_eq!(settled.stats.is_some(), extracted);
    if let Some(stats) = &settled.stats {
        assert!(stats.searches + stats.inherited <= 2 * u64::from(n));
        assert!(stats.settled_good <= 2 * u64::from(n));
        assert_eq!(
            stats.vertices_visited,
            stats.searches + settled.arm.extraction.search_nodes
        );
        // The per-literal arm always runs exactly 2n searches.
        assert_eq!(per_literal.extraction.rule_attempts, 2 * u64::from(n));
    }
    (per_literal, settled)
}

#[test]
fn all_canonical_subsets_and_ternary_extensions_agree() {
    let mut checked = 0;
    for mask in 0..512 {
        both(&subset(mask), 2);
        checked += 1;
        for signs in 0..8 {
            let mut input = subset(mask);
            input.push(ternary(signs));
            both(&input, 3);
            let mut input = vec![ternary(signs)];
            input.extend(subset(mask));
            both(&input, 3);
            checked += 2;
        }
    }
    assert_eq!(checked, 512 + 2 * 4096);
}

#[test]
fn chains_f_k_force_exactly_x_under_both_extractions() {
    for k in 2..=11 {
        let n = k + 1;
        let input = chain(k);
        let (per_literal, settled) = both(&input, n);
        assert_eq!(per_literal.derived_units, vec![1]);
        assert_eq!(settled.arm.derived_units, vec![1]);
        assert_eq!(settled.arm.clues[0].path.nodes.len() as u32, k + 2);
        let (per_literal, settled) = both(&negated(&input, 1), n);
        assert_eq!(per_literal.derived_units, vec![-1]);
        assert_eq!(settled.arm.derived_units, vec![-1]);
        // The settled arm runs every search here: no inheritance is available
        // on a chain visited from its sinks, and the searches are cheaper only
        // by the shared initialization.
        let stats = settled.stats.unwrap();
        assert_eq!(
            stats.searches + stats.inherited + stats.settled_good,
            2 * u64::from(n)
        );
    }
}

#[test]
fn odd_and_even_parity_cycles_decide_by_the_fragment() {
    for n in 3..=12 {
        let (per_literal, settled) = both(&parity_cycle(n), n);
        if n % 2 == 1 {
            assert_eq!(settled.arm.outcome, Outcome::Unsat);
            assert!(matches!(
                settled.arm.certificate,
                Some(Certificate::OppositePaths { .. })
            ));
            assert!(settled.stats.is_none());
        } else {
            assert_eq!(settled.arm.outcome, Outcome::Sat);
            assert!(per_literal.derived_units.is_empty());
            assert!(settled.arm.derived_units.is_empty());
            let stats = settled.stats.unwrap();
            // Two components of n vertices each: the first search of each
            // component settles the whole component good.
            assert_eq!(stats.searches, 2);
            assert_eq!(stats.settled_good, 2 * u64::from(n) - 2);
        }
    }
}

#[test]
fn ladder_controls_force_exactly_the_ladder_literals() {
    assert_eq!(
        ladder(2, 6, 1),
        vec![
            vec![3, 1],
            vec![-1, 2],
            vec![-2, 3],
            vec![-3, 4],
            vec![-4, 5],
            vec![-5, 6],
            vec![-6, 7],
            vec![-7, 8]
        ]
    );
    assert_eq!(
        ladder(1, 2, -1),
        vec![vec![-2, 1], vec![-1, -2], vec![2, -3]]
    );
    for (k, j) in LADDERS {
        for sign in [1, -1] {
            let n = k + j;
            let input = ladder(k, j, sign);
            assert_eq!(input.len() as u32, k + 1 + j - 1);
            let (per_literal, settled) = both(&input, n);
            let expected = ladder_units(k, j, sign);
            assert_eq!(per_literal.derived_units, expected);
            assert_eq!(settled.arm.derived_units, expected);
            assert!(settled
                .arm
                .derived_units
                .iter()
                .all(|unit| unit.unsigned_abs() > k));
            // Explanations grow along the ladder.
            let lengths: Vec<usize> = settled
                .arm
                .clues
                .iter()
                .map(|clue| clue.path.nodes.len())
                .collect();
            assert!(
                lengths.windows(2).all(|pair| pair[0] < pair[1]),
                "{lengths:?}"
            );
            let stats = settled.stats.unwrap();
            assert!(stats.inherited > 0, "ladder k={k} j={j}");
            assert!(settled.arm.extraction.work_units < per_literal.extraction.work_units);
        }
    }
}

#[test]
fn several_forced_literals_share_one_explanation() {
    let mut input = chain(3);
    input.push(vec![-1, 5]);
    input.push(vec![-1, 6]);
    input.push(vec![-5, 7]);
    let (per_literal, settled) = both(&input, 7);
    assert_eq!(per_literal.derived_units, vec![1, 5, 6, 7]);
    assert_eq!(settled.arm.derived_units, vec![1, 5, 6, 7]);
    let stats = settled.stats.unwrap();
    // The duals of 5, 6 and 7 inherit the recorded path of -1.
    assert!(stats.inherited >= 3);
    for clue in &settled.arm.clues[1..] {
        assert!(clue.path.nodes.contains(&-1) && clue.path.nodes.contains(&1));
    }
    // A wide formula whose fragment forces literals through the same explanation.
    input.push(vec![2, -5, 7]);
    input.push(vec![-3, 6, -7]);
    let (per_literal, settled) = both(&input, 7);
    assert_eq!(per_literal.derived_units, settled.arm.derived_units);
    assert_eq!(settled.arm.derived_units, vec![1, 5, 6, 7]);
}

#[test]
fn empty_and_malformed_inputs_keep_their_semantics_or_are_rejected() {
    let empty = solve_settled(&vec![], 0, ARM_BUDGET).unwrap();
    assert_eq!(empty.arm.outcome, Outcome::Sat);
    assert_eq!(empty.arm.certificate, Some(Certificate::Sat(vec![])));
    assert!(empty.arm.derived_units.is_empty());
    assert_eq!(empty.stats, Some(SettledStats::default()));
    let empty_clause = solve_settled(&vec![vec![]], 0, ARM_BUDGET).unwrap();
    assert_eq!(empty_clause.arm.outcome, Outcome::Unsat);
    assert_eq!(
        empty_clause.arm.certificate,
        Some(Certificate::EmptyClause(0))
    );
    assert_eq!(empty_clause.arm.residual.work_units, 0);
    assert!(empty_clause.stats.is_none());
    let (_, ternary_only) = both(&vec![vec![1, 2, 3], vec![-1, -2, -3]], 3);
    assert!(ternary_only.arm.derived_units.is_empty());
    assert_eq!(
        ternary_only.arm.certificate,
        Some(Certificate::Sat(vec![true; 3]))
    );
    // Without edges every vertex is searched and nothing is inherited.
    let stats = ternary_only.stats.unwrap();
    assert_eq!(stats.searches, 6);
    assert_eq!(stats.inherited, 0);
    assert_eq!(stats.vertices_visited, 6);
    both(&vec![vec![1, 1], vec![-1, -1, 2]], 2);
    both(&vec![vec![1, -1], vec![2, -2, 1]], 2);
    both(&vec![vec![1], vec![-1, 2], vec![-2, 3], vec![3, 1, 2]], 3);
    for input in [
        vec![vec![0]],
        vec![vec![i32::MIN]],
        vec![vec![1, 2, 3, 4]],
        vec![vec![4]],
        vec![vec![1, 2], vec![3, -1, 0]],
    ] {
        assert_eq!(
            solve_settled(&input, 3, ARM_BUDGET).map(|_| ()),
            Err(Failure::InvalidInput),
            "{input:?}"
        );
        assert_eq!(
            solve_per_literal(&input, 3, ARM_BUDGET).map(|_| ()),
            Err(Failure::InvalidInput),
            "{input:?}"
        );
    }
    assert_eq!(
        solve_settled(&vec![], 13, ARM_BUDGET).map(|_| ()),
        Err(Failure::InvalidInput)
    );
}

#[test]
fn budget_exhaustion_at_each_phase_boundary_of_the_settled_arm_stays_unknown() {
    let n = 6;
    let mut input = chain(5);
    input.push(vec![2, -4, 6]);
    input.push(vec![-3, 5, -6]);
    let full = solve_settled(&input, n, ARM_BUDGET).unwrap();
    assert_eq!(full.arm.outcome, Outcome::Sat);
    assert_eq!(full.arm.derived_units, vec![1]);
    let mut boundaries = Vec::new();
    let mut running = 0;
    for phase in phases(&full.arm) {
        assert!(phase.work_units > 0);
        running += phase.work_units;
        boundaries.push(running);
    }
    assert_eq!(running, full.arm.total_work());
    for (index, &boundary) in boundaries.iter().enumerate() {
        for budget in [boundary - 1, boundary] {
            if budget == full.arm.total_work() {
                continue;
            }
            let settled = solve_settled(&input, n, budget).unwrap();
            let arm = &settled.arm;
            assert_eq!(arm.outcome, Outcome::Unknown, "budget {budget}");
            assert!(arm.certificate.is_none());
            assert!(arm.clues.is_empty());
            assert!(arm.derived_units.is_empty());
            assert!(settled.stats.is_none());
            assert_eq!(arm.total_work(), budget);
            let mut spent = 0;
            for (position, phase) in phases(arm).iter().enumerate() {
                if position > index + usize::from(budget == boundary) {
                    assert_eq!(phase.work_units, 0);
                }
                spent += phase.work_units;
            }
            assert_eq!(spent, budget);
        }
    }
    for budget in [0, 1] {
        let settled = solve_settled(&input, n, budget).unwrap();
        assert_eq!(settled.arm.outcome, Outcome::Unknown);
        assert_eq!(settled.arm.total_work(), budget);
    }
    let exact = solve_settled(&input, n, full.arm.total_work()).unwrap();
    assert_eq!(exact.arm.outcome, Outcome::Sat);
    assert_eq!(exact.arm.certificate, full.arm.certificate);
    assert_eq!(exact.arm.clues, full.arm.clues);
    assert_eq!(exact.arm.residual, full.arm.residual);
    assert_eq!(exact.stats, full.stats);

    // A contradictory fragment decides at the certificate boundary and never
    // relabels exhaustion as UNSAT.
    let mut cycle = parity_cycle(3);
    cycle.push(vec![1, 2, 3]);
    let unsat = solve_settled(&cycle, 3, ARM_BUDGET).unwrap();
    assert_eq!(unsat.arm.outcome, Outcome::Unsat);
    assert!(matches!(
        unsat.arm.certificate,
        Some(Certificate::OppositePaths { .. })
    ));
    assert_eq!(unsat.arm.residual.work_units, 0);
    assert_eq!(unsat.arm.total_work(), unsat.arm.fragment_work());
    for budget in [
        0,
        unsat.arm.construction.work_units,
        unsat.arm.total_work() - 1,
    ] {
        let arm = solve_settled(&cycle, 3, budget).unwrap().arm;
        assert_eq!(arm.outcome, Outcome::Unknown);
        assert!(arm.certificate.is_none());
        assert_eq!(arm.total_work(), budget);
    }
    assert_eq!(
        solve_settled(&cycle, 3, unsat.arm.total_work())
            .unwrap()
            .arm
            .outcome,
        Outcome::Unsat
    );
}

#[test]
fn corpus_has_176_primary_and_162_secondary_cases_in_protocol_order() {
    let cases = corpus().unwrap();
    assert_eq!(cases.len(), PRIMARY_CASES + SECONDARY_CASES);
    let primary: Vec<&Case> = cases.iter().filter(|case| !case.secondary).collect();
    let secondary: Vec<&Case> = cases.iter().filter(|case| case.secondary).collect();
    assert_eq!(primary.len(), 176);
    assert_eq!(secondary.len(), 162);
    assert!(cases[..176].iter().all(|case| !case.secondary));
    assert!(cases[176..].iter().all(|case| case.secondary));
    let counted = |family: &str| primary.iter().filter(|c| c.family == family).count();
    assert_eq!(counted("random-mixed"), 144);
    assert_eq!(counted("fragment-only"), 24);
    assert_eq!(counted("ladder-control"), 8);
    let mut order: Vec<&str> = primary.iter().map(|case| case.family.as_str()).collect();
    order.dedup();
    assert_eq!(order, ["random-mixed", "fragment-only", "ladder-control"]);
    for (index, case) in cases.iter().enumerate() {
        assert!(case.variables <= 12);
        assert!(case.input.iter().all(|clause| clause.len() <= 3));
        assert!(case.input.iter().all(|clause| clause
            .iter()
            .all(|lit| *lit != 0 && lit.unsigned_abs() <= case.variables)));
        assert!(!cases[..index].iter().any(|previous| previous.id == case.id));
        if !case.secondary {
            assert!(
                !cases
                    .iter()
                    .enumerate()
                    .any(|(other, previous)| other != index
                        && previous.variables == case.variables
                        && previous.input == case.input),
                "{}",
                case.id
            );
        }
    }
    let first = &cases[0];
    assert_eq!(first.id, "random-n8-t3-b8-s4001");
    assert_eq!(first.family, "random-mixed");
    assert_eq!(first.binary_label.as_deref(), Some("b=n"));
    assert_eq!(
        first.effective_seed,
        Some(4001 ^ (8 << 32) ^ (3 << 40) ^ (8 << 48))
    );
    assert_eq!(first.input.len(), 24 + 8);
    assert!(first.input[..24].iter().all(|clause| clause.len() == 3));
    assert!(first.input[24..].iter().all(|clause| clause.len() == 2));
    // Three draws by hand from the documented generator for the first clause.
    let mut x: u64 = 4001 ^ (8 << 32) ^ (3 << 40) ^ (8 << 48);
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let variable = (next() % 8 + 1) as i32;
    let literal = if next() & 1 == 0 { variable } else { -variable };
    assert_eq!(first.input[0][0], literal);
    assert_eq!(cases[3].id, "random-n8-t3-b32-s4001");
    assert_eq!(cases[3].binary_label.as_deref(), Some("b=4n"));
    assert_eq!(cases[143].id, "random-n12-t5-b48-s32003");
    let fragment_only = &cases[144];
    assert_eq!(fragment_only.id, "fragment-only-n8-b8-s4001");
    assert_eq!(fragment_only.family, "fragment-only");
    assert_eq!(fragment_only.density, Some(0));
    assert_eq!(fragment_only.input.len(), 8);
    assert!(fragment_only.input.iter().all(|clause| clause.len() == 2));
    assert_eq!(
        fragment_only.effective_seed,
        Some(4001 ^ (8 << 32) ^ (8 << 48))
    );
    assert_eq!(cases[167].id, "fragment-only-n12-b24-s32003");
    for case in &cases[..168] {
        for clause in &case.input {
            for (i, a) in clause.iter().enumerate() {
                for b in &clause[i + 1..] {
                    assert_ne!(a.unsigned_abs(), b.unsigned_abs(), "{}", case.id);
                }
            }
        }
    }
    let ladders = &cases[168..176];
    let ids: Vec<&str> = ladders.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "ladder-k2-j6-pos",
            "ladder-k2-j6-neg",
            "ladder-k3-j9-pos",
            "ladder-k3-j9-neg",
            "ladder-k5-j7-pos",
            "ladder-k5-j7-neg",
            "ladder-k1-j11-pos",
            "ladder-k1-j11-neg"
        ]
    );
    assert_eq!(ladders[0].input, ladder(2, 6, 1));
    assert_eq!(ladders[0].variables, 8);
    assert_eq!(ladders[7].input, ladder(1, 11, -1));
    assert_eq!(ladders[7].variables, 12);
    assert_eq!(ladders[7].ladder, Some((1, 11)));
    assert_eq!(ladders[7].sign, Some(-1));
    let v1 = fragment::corpus().unwrap();
    for (mine, theirs) in secondary.iter().zip(&v1) {
        assert_eq!(mine.id, format!("v1:{}", theirs.id));
        assert_eq!(mine.family, format!("v1:{}", theirs.family));
        assert_eq!(mine.variables, theirs.variables);
        assert_eq!(mine.input, theirs.input);
        assert_eq!(mine.binary_label.as_deref(), theirs.binary_label);
    }
    assert_eq!(cases[176].id, "v1:random-n8-t3-b0-s3001");
    assert_eq!(cases[337].id, "v1:contradictory-n12-m7-t5");
}

// A minimal scan of the fragment-interface-v1 artifact's known structure.
fn number_after(text: &str, from: usize, key: &str) -> (u64, usize) {
    let start = text[from..].find(key).expect(key) + from + key.len();
    let end = text[start..]
        .find(|c: char| !c.is_ascii_digit())
        .expect("number end")
        + start;
    (text[start..end].parse().expect("number"), end)
}

fn text_after<'a>(text: &'a str, from: usize, key: &str, until: char) -> (&'a str, usize) {
    let start = text[from..].find(key).expect(key) + from + key.len();
    let end = text[start..].find(until).expect("delimiter") + start;
    (&text[start..end], end)
}

#[test]
fn arm_two_reproduces_the_fragment_interface_artifact_per_case() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/artifacts/fragment-interface.json"
    );
    let artifact = std::fs::read_to_string(path).expect("checked-in v1 artifact");
    assert!(artifact.contains("\"experiment\":\"fragment-interface-v1\""));
    let mut compared = 0;
    for case in corpus().unwrap().into_iter().filter(|case| case.secondary) {
        let v1_id = case.id.strip_prefix("v1:").unwrap();
        let block = artifact
            .find(&format!("{{\"id\":\"{v1_id}\","))
            .expect("case in artifact");
        let arm = solve_per_literal(&case.input, case.variables, ARM_BUDGET).unwrap();
        let v1 = fragment::solve_fragment(&case.input, case.variables, ARM_BUDGET).unwrap();
        assert!(same_arm(&arm, &v1), "{}", case.id);
        let (outcome, at) = text_after(&artifact, block, " \"fragment\":{\"outcome\":\"", '"');
        let expected = match arm.outcome {
            Outcome::Sat => "sat",
            Outcome::Unsat => "unsat",
            Outcome::Unknown => "unknown-budget",
        };
        assert_eq!(outcome, expected, "{}", case.id);
        let (units, at) = text_after(&artifact, at, "\"derived_units\":[", ']');
        assert_eq!(
            format!("[{units}]"),
            format!("{:?}", arm.derived_units),
            "{}",
            case.id
        );
        let mut at = artifact[at..].find("\"phases\":{").unwrap() + at;
        for (key, phase) in [
            ("\"construction\":{\"work_units\":", &arm.construction),
            ("\"components\":{\"work_units\":", &arm.components),
            (
                "\"certificate\":{\"work_units\":",
                &arm.decision_certificate,
            ),
            ("\"extraction\":{\"work_units\":", &arm.extraction),
            ("\"append\":{\"work_units\":", &arm.append),
            ("\"residual\":{\"work_units\":", &arm.residual),
        ] {
            let (value, next) = number_after(&artifact, at, key);
            assert_eq!(value, phase.work_units, "{} {key}", case.id);
            let (nodes, next) = number_after(&artifact, next, "\"search_nodes\":");
            assert_eq!(nodes, phase.search_nodes, "{} {key}", case.id);
            at = next;
        }
        let (fragment_work, at) = number_after(&artifact, at, "\"fragment_work_units\":");
        assert_eq!(fragment_work, arm.fragment_work(), "{}", case.id);
        let (residual_work, at) = number_after(&artifact, at, "\"residual_work_units\":");
        assert_eq!(residual_work, arm.residual.work_units, "{}", case.id);
        let (total, _) = number_after(&artifact, at, "\"total_work_units\":");
        assert_eq!(total, arm.total_work(), "{}", case.id);
        compared += 1;
    }
    assert_eq!(compared, 162);
}

#[test]
fn experiment_passes_every_acceptance_check_and_reports_deterministically() {
    assert_eq!(PROTOCOL_SHA256.len(), 64);
    let first = experiment().unwrap();
    assert_eq!(first.observations.len(), 338);
    let primary = &first.primary.summary;
    assert_eq!(primary.cases, 176);
    assert_eq!(primary.reference_sat + primary.reference_unsat, 176);
    assert_eq!(primary.baseline.unknown, 0);
    assert_eq!(primary.per_literal.unknown, 0);
    assert_eq!(primary.settled.unknown, 0);
    assert_eq!(primary.certificates_checked, 2 * 176);
    assert_eq!(
        primary.clues_checked,
        primary.per_literal.derived_units + primary.settled.derived_units
    );
    assert_eq!(
        primary.per_literal.derived_units,
        primary.settled.derived_units
    );
    assert_eq!(primary.settled_vs_per_literal.both_complete, 176);
    assert_eq!(primary.settled_vs_baseline.both_complete, 176);
    assert_eq!(primary.per_literal_vs_baseline.both_complete, 176);
    assert_eq!(primary.settled_vs_per_literal.left_worse, 0);
    assert_eq!(first.primary.families.len(), 3);
    assert_eq!(first.primary.binary_counts.len(), 4);
    let ladders = &first.primary.families["ladder-control"];
    assert_eq!(ladders.cases, 8);
    assert_eq!(ladders.per_literal.derived_units, 2 * (6 + 9 + 7 + 11));
    assert_eq!(ladders.settled.derived_units, 2 * (6 + 9 + 7 + 11));
    assert_eq!(ladders.reference_sat, 8);
    let secondary = &first.secondary.summary;
    assert_eq!(secondary.cases, 162);
    assert_eq!(first.secondary.families.len(), 4);
    assert_eq!(first.secondary.binary_counts.len(), 3);
    assert_eq!(secondary.settled_vs_per_literal.both_complete, 162);
    // Arm 2 on the secondary corpus is the fragment-interface-v1 arm.
    let v1 = fragment::experiment().unwrap();
    for (mine, theirs) in first
        .observations
        .iter()
        .filter(|row| row.case.secondary)
        .zip(&v1.observations)
    {
        assert_eq!(mine.case.id, format!("v1:{}", theirs.case.id));
        assert!(same_arm(&mine.per_literal, &theirs.fragment));
    }
    assert_eq!(
        secondary.per_literal.total_work_units,
        v1.summary.fragment.total_work_units
    );
    for row in &first.observations {
        assert_eq!(
            row.settled.arm.total_work(),
            row.settled.arm.fragment_work() + row.settled.arm.residual.work_units
        );
    }
    let json_first = json(&first);
    let second = experiment().unwrap();
    assert_eq!(json_first, json(&second));
    assert!(json_first.contains("\"experiment\":\"extraction-cost-v1\""));
    assert!(json_first.contains(PROTOCOL_SHA256));
    assert!(json_first.contains("\"random_mixed_by_binary_count\":{\"b=2n\":"));
    assert!(json_first.contains("\"v1_random_mixed_by_binary_count\":{\"b=2n\":"));
    assert!(summary_line(&first).contains("176 cases"));
}
