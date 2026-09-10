//! The binary-fragment interface: the implication graph of a width-at-most-
//! three formula's width-at-most-two clauses decides the fragment, and its
//! forced literals are appended as units before the common DPLL runs. No
//! truth table, reference label, or reference backbone reaches either arm.
//! See the frozen experiment protocol `experiments/fragment-interface-protocol.md`.

use crate::implication::{
    self, accumulate, add, count, difference, increment, optional, outcome_name, quoted,
    Certificate, Checks, Clue, Order, Reference,
};
use crate::transfer::{self, Arm, Event, Failure, Meter, Outcome, Rng, Work};
use crate::Cnf;
use std::collections::BTreeMap;
use std::fmt::Write;

// SHA-256 of experiments/fragment-interface-protocol.md at its freeze commit 779b715.
pub const PROTOCOL_SHA256: &str =
    "bce209aa982a9964cf235758e43c7b6f405d7735a3c52c11cc867fbe9e60ec36";
pub const ARM_BUDGET: u64 = 1_000_000;
pub const SEEDS: [u64; 4] = [3001, 6007, 12007, 24001];
pub const VARIABLES: [u32; 3] = [8, 10, 12];
pub const DENSITIES: [u32; 3] = [3, 4, 5];
pub const CONTROL_DENSITIES: [u32; 2] = [3, 5];
pub const CHAIN_SEED: u64 = 3001;
pub const CONTRADICTORY_SEED: u64 = 6007;
pub const CORPUS_CASES: usize = 162;

// ---------------------------------------------------------------------------
// The fragment arm: one checked meter across every phase.

#[derive(Clone, Debug)]
pub struct FragmentArm {
    pub outcome: Outcome,
    /// The fragment's own certificate: a fragment assignment, an empty clause,
    /// or opposite paths. Present only when the arm completed.
    pub certificate: Option<Certificate>,
    pub clues: Vec<Clue>,
    pub derived_units: Vec<i32>,
    pub construction: Work,
    pub components: Work,
    pub decision_certificate: Work,
    pub extraction: Work,
    pub append: Work,
    pub residual: Work,
    pub total: Work,
}

impl FragmentArm {
    pub(crate) fn empty() -> Self {
        Self {
            outcome: Outcome::Unknown,
            certificate: None,
            clues: Vec::new(),
            derived_units: Vec::new(),
            construction: Work::default(),
            components: Work::default(),
            decision_certificate: Work::default(),
            extraction: Work::default(),
            append: Work::default(),
            residual: Work::default(),
            total: Work::default(),
        }
    }
    /// Every phase charged before the residual solver: graph construction,
    /// components, fragment certificate, extraction, and copy plus appends.
    pub fn fragment_work(&self) -> u64 {
        let mut total = 0_u64;
        for phase in [
            &self.construction,
            &self.components,
            &self.decision_certificate,
            &self.extraction,
            &self.append,
        ] {
            total = total
                .checked_add(phase.work_units)
                .expect("shared budget bounds sum");
        }
        total
    }
    pub fn total_work(&self) -> u64 {
        self.total.work_units
    }
}

/// Run one phase on the shared meter and snapshot its charge as a difference.
/// Budget exhaustion yields `None`; every other failure propagates.
pub(crate) fn phase<T>(
    meter: &mut Meter,
    slot: &mut Work,
    run: impl FnOnce(&mut Meter) -> Result<T, Failure>,
) -> Result<Option<T>, Failure> {
    let before = meter.work.clone();
    let result = run(meter);
    *slot = difference(&meter.work, &before);
    match result {
        Ok(value) => Ok(Some(value)),
        Err(Failure::Budget) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Exhaustion anywhere means unknown: no certificate, no clue, no unit, and
/// the spent budget reported in full.
pub(crate) fn exhausted(mut arm: FragmentArm, meter: Meter) -> FragmentArm {
    arm.outcome = Outcome::Unknown;
    arm.certificate = None;
    arm.clues = Vec::new();
    arm.derived_units = Vec::new();
    arm.total = meter.work;
    arm
}

/// Build the fragment graph, decide the fragment, extract its forced
/// literals, append them to a copy of the original formula, and run the
/// common DPLL with whatever budget remains. No oracle is consulted.
pub fn solve_fragment(input: &Cnf, n: u32, budget: u64) -> Result<FragmentArm, Failure> {
    let mut meter = Meter::new(budget);
    let mut arm = FragmentArm::empty();
    let Some(graph) = phase(&mut meter, &mut arm.construction, |m| {
        implication::build_fragment(input, n, m)
    })?
    else {
        return Ok(exhausted(arm, meter));
    };
    let Some(component) = phase(&mut meter, &mut arm.components, |m| {
        implication::components(&graph, m)
    })?
    else {
        return Ok(exhausted(arm, meter));
    };
    let Some(certificate) = phase(&mut meter, &mut arm.decision_certificate, |m| {
        implication::decision_certificate(&graph, &component, n, m)
    })?
    else {
        return Ok(exhausted(arm, meter));
    };
    if !matches!(certificate, Certificate::Sat(_)) {
        arm.outcome = Outcome::Unsat;
        arm.certificate = Some(certificate);
        arm.total = meter.work;
        return Ok(arm);
    }
    let Some(clues) = phase(&mut meter, &mut arm.extraction, |m| {
        implication::extract(&graph, n, m)
    })?
    else {
        return Ok(exhausted(arm, meter));
    };
    // Copy the original formula and append each forced literal as a unit, in
    // the order found, with the clue-transfer copy and append charges.
    let Some(processed) = phase(&mut meter, &mut arm.append, |m| {
        let mut output = transfer::copy(input, m)?;
        for clue in &clues {
            m.tick(Event::LiteralRead)?;
            m.tick(Event::ClauseWrite)?;
            m.tick(Event::LiteralWrite)?;
            output.push(vec![clue.literal]);
        }
        Ok(output)
    })?
    else {
        return Ok(exhausted(arm, meter));
    };
    let Some(satisfiable) = phase(&mut meter, &mut arm.residual, |m| {
        transfer::decide(&processed, n, m)
    })?
    else {
        return Ok(exhausted(arm, meter));
    };
    arm.outcome = if satisfiable {
        Outcome::Sat
    } else {
        Outcome::Unsat
    };
    arm.certificate = Some(certificate);
    arm.derived_units = clues.iter().map(|clue| clue.literal).collect();
    arm.clues = clues;
    arm.total = meter.work;
    Ok(arm)
}

// ---------------------------------------------------------------------------
// Independent reference and raw checkers, outside both arms.

/// Direct truth-table evaluation of a width-at-most-three formula.
pub fn reference(input: &Cnf, n: u32) -> Result<Reference, Failure> {
    implication::reference_wide(input, n)
}

/// Raw-clause check of a fragment certificate against the whole formula's
/// clause list; a SAT certificate must satisfy every width-at-most-two clause.
pub fn check_certificate(
    input: &Cnf,
    n: u32,
    certificate: &Certificate,
) -> Result<(bool, Work), Failure> {
    implication::check_fragment_certificate(input, n, certificate)
}

/// Raw-clause check of one forced literal's path by original clause index.
pub fn check_clue(input: &Cnf, n: u32, clue: &Clue) -> Result<(bool, Work), Failure> {
    implication::check_fragment_clue(input, n, clue)
}

// ---------------------------------------------------------------------------
// Fixed 162-case corpus, enumerated in protocol order.

#[derive(Clone, Debug)]
pub struct Case {
    pub id: String,
    pub family: &'static str,
    pub variables: u32,
    pub input: Cnf,
    pub seed: Option<u64>,
    pub effective_seed: Option<u64>,
    pub density: Option<u32>,
    pub binary_count: Option<u32>,
    pub binary_label: Option<&'static str>,
    pub chain_length: Option<u32>,
    pub sign: Option<i32>,
    pub cycle_length: Option<u32>,
}

pub fn effective_seed(base_seed: u64, variables: u32, density: u32, binary: u32) -> u64 {
    base_seed
        ^ (u64::from(variables) << 32)
        ^ (u64::from(density) << 40)
        ^ (u64::from(binary) << 48)
}

/// One clause of the given width: draw a variable, reject repeats, then draw
/// its sign from a fresh draw's low bit (zero meaning positive).
fn draw(rng: &mut Rng, variables: u32, width: usize) -> Vec<i32> {
    let mut clause: Vec<i32> = Vec::new();
    while clause.len() < width {
        let variable = (rng.next() % u64::from(variables) + 1) as i32;
        if clause
            .iter()
            .any(|lit| lit.unsigned_abs() == variable.unsigned_abs())
        {
            continue;
        }
        clause.push(if rng.next() & 1 == 0 {
            variable
        } else {
            -variable
        });
    }
    clause
}

/// `t*n` ternary clauses followed by `b` binary clauses from one generator.
pub(crate) fn random(variables: u32, density: u32, binary: u32, seed: u64) -> Cnf {
    let mut rng = Rng(seed);
    let mut formula = Vec::new();
    for _ in 0..variables * density {
        formula.push(draw(&mut rng, variables, 3));
    }
    for _ in 0..binary {
        formula.push(draw(&mut rng, variables, 2));
    }
    formula
}

fn sign_name(sign: i32) -> &'static str {
    if sign > 0 {
        "pos"
    } else {
        "neg"
    }
}

fn cycle_length(variables: u32) -> u32 {
    match variables {
        8 => 3,
        10 => 5,
        _ => 7,
    }
}

fn plain(id: String, family: &'static str, variables: u32, input: Cnf) -> Case {
    Case {
        id,
        family,
        variables,
        input,
        seed: None,
        effective_seed: None,
        density: None,
        binary_count: None,
        binary_label: None,
        chain_length: None,
        sign: None,
        cycle_length: None,
    }
}

/// The fixed evaluation set in protocol enumeration order, rejected if any
/// exact `(declared variables, ordered CNF)` pair repeats within it or
/// appears in one of the three earlier corpora.
pub fn corpus() -> Result<Vec<Case>, Failure> {
    let mut cases = Vec::new();
    for seed in SEEDS {
        for n in VARIABLES {
            for t in DENSITIES {
                for (label, b) in [("b=0", 0), ("b=n/2", n / 2), ("b=n", n), ("b=2n", 2 * n)] {
                    let family = if b == 0 {
                        "random-ternary-only"
                    } else {
                        "random-mixed"
                    };
                    let effective = effective_seed(seed, n, t, b);
                    let mut case = plain(
                        format!("random-n{n}-t{t}-b{b}-s{seed}"),
                        family,
                        n,
                        random(n, t, b, effective),
                    );
                    case.seed = Some(seed);
                    case.effective_seed = Some(effective);
                    case.density = Some(t);
                    case.binary_count = Some(b);
                    case.binary_label = Some(label);
                    cases.push(case);
                }
            }
        }
    }
    for n in VARIABLES {
        for t in CONTROL_DENSITIES {
            for sign in [1, -1] {
                let k = n - 1;
                let effective = effective_seed(CHAIN_SEED, n, t, n);
                let mut input =
                    implication::transformed(&implication::chain(k, false), sign, Order::Forward);
                input.extend(random(n, t, 0, effective));
                let mut case = plain(
                    format!("chain-n{n}-t{t}-{}", sign_name(sign)),
                    "chain-embedded",
                    n,
                    input,
                );
                case.seed = Some(CHAIN_SEED);
                case.effective_seed = Some(effective);
                case.density = Some(t);
                case.binary_count = Some(n);
                case.chain_length = Some(k);
                case.sign = Some(sign);
                cases.push(case);
            }
        }
    }
    for n in VARIABLES {
        let m = cycle_length(n);
        for t in CONTROL_DENSITIES {
            let effective = effective_seed(CONTRADICTORY_SEED, n, t, 2 * m);
            let mut input: Cnf = (1..=m)
                .flat_map(|i| {
                    let a = i as i32;
                    let b = (i % m + 1) as i32;
                    [vec![a, b], vec![-a, -b]]
                })
                .collect();
            input.extend(random(n, t, 0, effective));
            let mut case = plain(
                format!("contradictory-n{n}-m{m}-t{t}"),
                "contradictory-fragment",
                n,
                input,
            );
            case.seed = Some(CONTRADICTORY_SEED);
            case.effective_seed = Some(effective);
            case.density = Some(t);
            case.binary_count = Some(2 * m);
            case.cycle_length = Some(m);
            cases.push(case);
        }
    }
    if cases.len() != CORPUS_CASES {
        return Err(Failure::InvalidInput);
    }
    let mut earlier: Vec<(u32, Cnf)> = Vec::new();
    earlier.extend(
        transfer::corpus()
            .into_iter()
            .map(|case| (case.variables, case.input)),
    );
    earlier.extend(
        crate::indexed::corpus()?
            .into_iter()
            .map(|case| (case.variables, case.input)),
    );
    earlier.extend(
        implication::corpus()
            .into_iter()
            .map(|case| (case.variables, case.input)),
    );
    for (index, case) in cases.iter().enumerate() {
        let same =
            |(variables, input): &(u32, Cnf)| *variables == case.variables && *input == case.input;
        if earlier.iter().any(same)
            || cases[..index]
                .iter()
                .any(|previous| same(&(previous.variables, previous.input.clone())))
        {
            return Err(Failure::InvalidInput);
        }
    }
    Ok(cases)
}

// ---------------------------------------------------------------------------
// Experiment: reference, two arms, raw checkers, acceptance, summaries.

pub struct Observation {
    pub case: Case,
    pub reference: Reference,
    pub baseline: Arm,
    pub fragment: FragmentArm,
    pub checks: Checks,
}

/// Raw checkers on the fragment certificate and every derived unit's path.
fn check(case: &Case, fragment: &FragmentArm) -> Result<Checks, Failure> {
    let mut checks = Checks::default();
    if let Some(certificate) = &fragment.certificate {
        let (valid, work) = check_certificate(&case.input, case.variables, certificate)?;
        checks.certificate_valid = Some(valid);
        accumulate(&mut checks.work, &work)?;
    }
    for clue in &fragment.clues {
        let (valid, work) = check_clue(&case.input, case.variables, clue)?;
        increment(&mut checks.clues_checked)?;
        add(&mut checks.clues_valid, u64::from(valid))?;
        accumulate(&mut checks.work, &work)?;
    }
    Ok(checks)
}

fn phases_after_certificate_are_idle(arm: &FragmentArm) -> bool {
    arm.extraction.work_units == 0 && arm.append.work_units == 0 && arm.residual.work_units == 0
}

/// Every protocol acceptance check. A violation invalidates the calibration.
fn accept(row: &Observation) -> Result<(), Failure> {
    let label = row.reference.model_count > 0;
    if label != row.reference.backbone.is_some() {
        return Err(Failure::InvalidInput);
    }
    let baseline = &row.baseline;
    let fragment = &row.fragment;
    if !baseline.outcome.agrees(label) || !fragment.outcome.agrees(label) {
        return Err(Failure::InvalidInput);
    }
    if baseline.total_work() > ARM_BUDGET || fragment.total_work() > ARM_BUDGET {
        return Err(Failure::InvalidInput);
    }
    // Unknown only with an exhausted budget, and never with partial evidence.
    if baseline.outcome == Outcome::Unknown && baseline.total_work() != ARM_BUDGET {
        return Err(Failure::InvalidInput);
    }
    if fragment.total_work() != fragment.fragment_work() + fragment.residual.work_units {
        return Err(Failure::InvalidInput);
    }
    match fragment.outcome {
        Outcome::Unknown => {
            if fragment.total_work() != ARM_BUDGET
                || fragment.certificate.is_some()
                || !fragment.clues.is_empty()
                || !fragment.derived_units.is_empty()
            {
                return Err(Failure::InvalidInput);
            }
        }
        Outcome::Unsat | Outcome::Sat => {
            let Some(certificate) = &fragment.certificate else {
                return Err(Failure::InvalidInput);
            };
            let fragment_unsat = !matches!(certificate, Certificate::Sat(_));
            if fragment_unsat {
                // Decided by the fragment alone: no extraction, append, or search.
                if fragment.outcome != Outcome::Unsat
                    || !fragment.clues.is_empty()
                    || !fragment.derived_units.is_empty()
                    || !phases_after_certificate_are_idle(fragment)
                {
                    return Err(Failure::InvalidInput);
                }
            } else if fragment.residual.work_units == 0 {
                return Err(Failure::InvalidInput);
            }
            let units: Vec<i32> = fragment.clues.iter().map(|clue| clue.literal).collect();
            if units != fragment.derived_units {
                return Err(Failure::InvalidInput);
            }
        }
    }
    // Every reported certificate and clue passed the raw checker.
    if fragment.certificate.is_some() != row.checks.certificate_valid.is_some()
        || row.checks.certificate_valid == Some(false)
        || row.checks.clues_checked != count(fragment.clues.len())?
        || row.checks.clues_valid != row.checks.clues_checked
    {
        return Err(Failure::InvalidInput);
    }
    // On satisfiable cases the derived units are a subset of the backbone.
    if let Some(backbone) = &row.reference.backbone {
        let mut seen: Vec<i32> = Vec::new();
        for unit in &fragment.derived_units {
            if !backbone.contains(unit) || seen.contains(unit) {
                return Err(Failure::InvalidInput);
            }
            seen.push(*unit);
        }
    }
    match row.case.family {
        "chain-embedded" => {
            let sign = row.case.sign.ok_or(Failure::InvalidInput)?;
            if let Some(certificate) = &fragment.certificate {
                if !matches!(certificate, Certificate::Sat(_))
                    || fragment.derived_units != vec![sign]
                {
                    return Err(Failure::InvalidInput);
                }
            }
        }
        "contradictory-fragment" => {
            if fragment.outcome != Outcome::Unsat
                || !matches!(
                    fragment.certificate,
                    Some(Certificate::EmptyClause(_) | Certificate::OppositePaths { .. })
                )
                || !phases_after_certificate_are_idle(fragment)
            {
                return Err(Failure::InvalidInput);
            }
        }
        // An empty fragment derives nothing and its certificate is a fragment assignment.
        "random-ternary-only"
            if fragment.outcome != Outcome::Unknown
                && (!fragment.derived_units.is_empty()
                    || !matches!(fragment.certificate, Some(Certificate::Sat(_)))) =>
        {
            return Err(Failure::InvalidInput);
        }
        _ => (),
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct BaselineSummary {
    pub work_units: u64,
    pub search_nodes: u64,
    pub sat: u64,
    pub unsat: u64,
    pub unknown: u64,
}

impl BaselineSummary {
    pub(crate) fn include(&mut self, arm: &Arm) -> Result<(), Failure> {
        add(&mut self.work_units, arm.total_work())?;
        add(&mut self.search_nodes, arm.residual.search_nodes)?;
        increment(match arm.outcome {
            Outcome::Sat => &mut self.sat,
            Outcome::Unsat => &mut self.unsat,
            Outcome::Unknown => &mut self.unknown,
        })
    }
    pub(crate) fn json(&self) -> String {
        format!(
            "{{\"work_units\":{},\"search_nodes\":{},\"complete\":{},\"sat\":{},\"unsat\":{},\"unknown\":{}}}",
            self.work_units,
            self.search_nodes,
            self.sat + self.unsat,
            self.sat,
            self.unsat,
            self.unknown
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct FragmentSummary {
    pub construction_work_units: u64,
    pub components_work_units: u64,
    pub certificate_work_units: u64,
    pub extraction_work_units: u64,
    pub append_work_units: u64,
    pub fragment_work_units: u64,
    pub residual_work_units: u64,
    pub total_work_units: u64,
    pub search_nodes: u64,
    pub derived_units: u64,
    pub fragment_unsat: u64,
    pub sat: u64,
    pub unsat: u64,
    pub unknown: u64,
}

impl FragmentSummary {
    pub(crate) fn include(&mut self, arm: &FragmentArm) -> Result<(), Failure> {
        add(
            &mut self.construction_work_units,
            arm.construction.work_units,
        )?;
        add(&mut self.components_work_units, arm.components.work_units)?;
        add(
            &mut self.certificate_work_units,
            arm.decision_certificate.work_units,
        )?;
        add(&mut self.extraction_work_units, arm.extraction.work_units)?;
        add(&mut self.append_work_units, arm.append.work_units)?;
        add(&mut self.fragment_work_units, arm.fragment_work())?;
        add(&mut self.residual_work_units, arm.residual.work_units)?;
        add(&mut self.total_work_units, arm.total_work())?;
        add(&mut self.search_nodes, arm.residual.search_nodes)?;
        add(&mut self.derived_units, count(arm.derived_units.len())?)?;
        if matches!(
            arm.certificate,
            Some(Certificate::EmptyClause(_) | Certificate::OppositePaths { .. })
        ) {
            increment(&mut self.fragment_unsat)?;
        }
        increment(match arm.outcome {
            Outcome::Sat => &mut self.sat,
            Outcome::Unsat => &mut self.unsat,
            Outcome::Unknown => &mut self.unknown,
        })
    }
    pub(crate) fn json(&self) -> String {
        format!(
            concat!(
                "{{\"phases\":{{\"construction_work_units\":{},\"components_work_units\":{},",
                "\"certificate_work_units\":{},\"extraction_work_units\":{},\"append_work_units\":{}}},",
                "\"fragment_work_units\":{},\"residual_work_units\":{},\"total_work_units\":{},",
                "\"search_nodes\":{},\"derived_units\":{},\"fragment_unsat\":{},\"complete\":{},",
                "\"sat\":{},\"unsat\":{},\"unknown\":{}}}"
            ),
            self.construction_work_units,
            self.components_work_units,
            self.certificate_work_units,
            self.extraction_work_units,
            self.append_work_units,
            self.fragment_work_units,
            self.residual_work_units,
            self.total_work_units,
            self.search_nodes,
            self.derived_units,
            self.fragment_unsat,
            self.sat + self.unsat,
            self.sat,
            self.unsat,
            self.unknown
        )
    }
}

/// Total online work, fragment arm on the left, compared only when both
/// arms completed.
#[derive(Clone, Debug, Default)]
pub struct Comparison {
    pub both_complete: u64,
    pub left_better: u64,
    pub tied: u64,
    pub left_worse: u64,
    pub left_complete_only: u64,
    pub right_complete_only: u64,
    pub both_unknown: u64,
}

impl Comparison {
    pub(crate) fn include(&mut self, left: Option<u64>, right: Option<u64>) -> Result<(), Failure> {
        match (left, right) {
            (Some(left), Some(right)) => {
                increment(&mut self.both_complete)?;
                increment(match left.cmp(&right) {
                    std::cmp::Ordering::Less => &mut self.left_better,
                    std::cmp::Ordering::Equal => &mut self.tied,
                    std::cmp::Ordering::Greater => &mut self.left_worse,
                })
            }
            (Some(_), None) => increment(&mut self.left_complete_only),
            (None, Some(_)) => increment(&mut self.right_complete_only),
            (None, None) => increment(&mut self.both_unknown),
        }
    }
    pub(crate) fn json(&self) -> String {
        format!(
            "{{\"both_complete\":{},\"left_better\":{},\"tied\":{},\"left_worse\":{},\"left_complete_only\":{},\"right_complete_only\":{},\"both_unknown\":{}}}",
            self.both_complete, self.left_better, self.tied, self.left_worse, self.left_complete_only, self.right_complete_only, self.both_unknown
        )
    }
}

/// A descriptive ratio as a JSON number with three decimals, computed with
/// integer arithmetic (nearest rounding); `null` when undefined.
pub(crate) fn ratio(numerator: u64, denominator: u64) -> String {
    if denominator == 0 {
        return "null".into();
    }
    let scaled = numerator
        .checked_mul(1000)
        .and_then(|value| value.checked_add(denominator / 2))
        .map(|value| value / denominator);
    match scaled {
        Some(scaled) => format!("{}.{:03}", scaled / 1000, scaled % 1000),
        None => "null".into(),
    }
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub cases: u64,
    pub reference_sat: u64,
    pub reference_unsat: u64,
    pub reference_work_units: u64,
    pub checker_work_units: u64,
    pub certificates_checked: u64,
    pub clues_checked: u64,
    pub baseline: BaselineSummary,
    pub fragment: FragmentSummary,
    pub fragment_vs_baseline: Comparison,
}

impl Summary {
    fn include(&mut self, row: &Observation) -> Result<(), Failure> {
        increment(&mut self.cases)?;
        increment(if row.reference.model_count > 0 {
            &mut self.reference_sat
        } else {
            &mut self.reference_unsat
        })?;
        add(
            &mut self.reference_work_units,
            row.reference.work.work_units,
        )?;
        add(&mut self.checker_work_units, row.checks.work.work_units)?;
        add(
            &mut self.certificates_checked,
            u64::from(row.checks.certificate_valid.is_some()),
        )?;
        add(&mut self.clues_checked, row.checks.clues_checked)?;
        self.baseline.include(&row.baseline)?;
        self.fragment.include(&row.fragment)?;
        // Acceptance already established that every completed decision
        // agrees with the reference, so completed totals are comparable.
        let fragment =
            (row.fragment.outcome != Outcome::Unknown).then(|| row.fragment.total_work());
        let baseline =
            (row.baseline.outcome != Outcome::Unknown).then(|| row.baseline.total_work());
        self.fragment_vs_baseline.include(fragment, baseline)
    }
    pub(crate) fn json(&self) -> String {
        format!(
            concat!(
                "{{\"cases\":{},\"reference_sat\":{},\"reference_unsat\":{},\"reference_work_units\":{},",
                "\"checker_work_units\":{},\"certificates_checked\":{},\"clues_checked\":{},",
                "\"arms\":{{\"baseline\":{},\"fragment\":{}}},",
                "\"comparisons\":{{\"fragment_vs_baseline\":{}}},",
                "\"enumeration_ratio\":{{\"reference_over_baseline\":{},\"reference_over_fragment\":{}}}}}"
            ),
            self.cases,
            self.reference_sat,
            self.reference_unsat,
            self.reference_work_units,
            self.checker_work_units,
            self.certificates_checked,
            self.clues_checked,
            self.baseline.json(),
            self.fragment.json(),
            self.fragment_vs_baseline.json(),
            ratio(self.reference_work_units, self.baseline.work_units),
            ratio(self.reference_work_units, self.fragment.total_work_units)
        )
    }
}

pub struct Experiment {
    pub observations: Vec<Observation>,
    pub summary: Summary,
    pub families: BTreeMap<&'static str, Summary>,
    /// Subtotals of `random-mixed` by binary count, the treatment variable.
    pub binary_counts: BTreeMap<&'static str, Summary>,
    /// Subtotals of both random families by declared variables and density.
    pub random_variables: BTreeMap<String, Summary>,
    pub random_densities: BTreeMap<String, Summary>,
}

/// Run only against the frozen protocol. No label or reference backbone
/// reaches either arm; the checkers run after both arms on each case.
pub fn experiment() -> Result<Experiment, Failure> {
    if PROTOCOL_SHA256.len() != 64 {
        return Err(Failure::InvalidInput);
    }
    let cases = corpus()?;
    let mut observations = Vec::new();
    let mut summary = Summary::default();
    let mut families = BTreeMap::<&'static str, Summary>::new();
    let mut binary_counts = BTreeMap::<&'static str, Summary>::new();
    let mut random_variables = BTreeMap::<String, Summary>::new();
    let mut random_densities = BTreeMap::<String, Summary>::new();
    for case in cases {
        let reference = reference(&case.input, case.variables)?;
        let baseline = transfer::solve(&case.input, case.variables, None, ARM_BUDGET)?;
        let fragment = solve_fragment(&case.input, case.variables, ARM_BUDGET)?;
        let checks = check(&case, &fragment)?;
        let row = Observation {
            case,
            reference,
            baseline,
            fragment,
            checks,
        };
        accept(&row)?;
        summary.include(&row)?;
        families.entry(row.case.family).or_default().include(&row)?;
        if row.case.family == "random-mixed" {
            let label = row.case.binary_label.ok_or(Failure::InvalidInput)?;
            binary_counts.entry(label).or_default().include(&row)?;
        }
        if row.case.family.starts_with("random-") {
            let density = row.case.density.ok_or(Failure::InvalidInput)?;
            random_variables
                .entry(format!("n={}", row.case.variables))
                .or_default()
                .include(&row)?;
            random_densities
                .entry(format!("t={density}"))
                .or_default()
                .include(&row)?;
        }
        observations.push(row);
    }
    Ok(Experiment {
        observations,
        summary,
        families,
        binary_counts,
        random_variables,
        random_densities,
    })
}

// ---------------------------------------------------------------------------
// Deterministic reporting. Formatting is outside every meter.

impl FragmentArm {
    pub(crate) fn json(&self) -> String {
        let mut clues = String::from("[");
        for (index, clue) in self.clues.iter().enumerate() {
            if index > 0 {
                clues.push(',');
            }
            write!(
                clues,
                "{{\"literal\":{},\"path\":{}}}",
                clue.literal,
                clue.path.json()
            )
            .expect("string write");
        }
        clues.push(']');
        format!(
            concat!(
                "{{\"outcome\":{},\"certificate\":{},\"derived_units\":{:?},\"clues\":{},\n",
                "  \"phases\":{{\"construction\":{},\"components\":{},\"certificate\":{},\"extraction\":{},\"append\":{},\"residual\":{}}},\n",
                "  \"fragment_work_units\":{},\"residual_work_units\":{},\"search_nodes\":{},\"total_work_units\":{}}}"
            ),
            quoted(outcome_name(&self.outcome)),
            self.certificate
                .as_ref()
                .map_or_else(|| "null".into(), Certificate::json),
            self.derived_units,
            clues,
            self.construction.json(),
            self.components.json(),
            self.decision_certificate.json(),
            self.extraction.json(),
            self.append.json(),
            self.residual.json(),
            self.fragment_work(),
            self.residual.work_units,
            self.residual.search_nodes,
            self.total_work()
        )
    }
}

fn observation_json(row: &Observation) -> String {
    let used = row
        .case
        .input
        .iter()
        .flatten()
        .map(|lit| lit.unsigned_abs())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let binary_clauses = row
        .case
        .input
        .iter()
        .filter(|clause| clause.len() <= 2)
        .count();
    let backbone = row
        .reference
        .backbone
        .as_ref()
        .map_or_else(|| "null".into(), |backbone| format!("{backbone:?}"));
    format!(
        concat!(
            "{{\"id\":{},\"family\":{},\"variables\":{},\"used_variables\":{},\"clauses\":{},\"fragment_clauses\":{},",
            "\"seed\":{},\"effective_seed\":{},\"density\":{},\"binary_count\":{},\"binary_label\":{},",
            "\"chain_length\":{},\"sign\":{},\"cycle_length\":{},\"input\":{:?},\n",
            " \"reference\":{{\"model_count\":{},\"sat\":{},\"backbone\":{},\"work\":{}}},\n",
            " \"baseline\":{{\"outcome\":{},\"work\":{},\"total_work_units\":{},\"search_nodes\":{}}},\n",
            " \"fragment\":{},\n",
            " \"checks\":{{\"certificate_valid\":{},\"clues_checked\":{},\"clues_valid\":{},\"work\":{}}}}}"
        ),
        quoted(&row.case.id),
        quoted(row.case.family),
        row.case.variables,
        used,
        row.case.input.len(),
        binary_clauses,
        optional(row.case.seed),
        row.case
            .effective_seed
            .map_or_else(|| "null".into(), |seed| quoted(&seed.to_string())),
        optional(row.case.density),
        optional(row.case.binary_count),
        row.case
            .binary_label
            .map_or_else(|| "null".into(), quoted),
        optional(row.case.chain_length),
        optional(row.case.sign),
        optional(row.case.cycle_length),
        row.case.input,
        row.reference.model_count,
        row.reference.model_count > 0,
        backbone,
        row.reference.work.json(),
        quoted(outcome_name(&row.baseline.outcome)),
        row.baseline.residual.json(),
        row.baseline.total_work(),
        row.baseline.residual.search_nodes,
        row.fragment.json(),
        optional(row.checks.certificate_valid),
        row.checks.clues_checked,
        row.checks.clues_valid,
        row.checks.work.json()
    )
}

const LIMITATIONS: [&str; 11] = [
    "Calibration of a known interface: the Aspvall-Plass-Tarjan decision and path-based forced literals of the binary fragment feed a toy DPLL; no novel inference rule, evolutionary advantage, or general SAT complexity result.",
    "Finite fixed corpus of at most 12 declared variables and clause width at most three; pseudorandom cases are reproducible samples, not independent or representative population estimates, and all sizes are calibration sizes chosen so the truth-table reference stays feasible, not scaling evidence.",
    "Empty fragments enable no inference and their construction cost is still charged; the random-ternary-only family is an overhead control.",
    "The fragment arm uses no learned library; the implication-graph procedure is fixed algorithmic knowledge with no acquisition step, so no acquisition cost is reported.",
    "The residual DPLL has no polynomial bound; the stated O(n + m2), O(n(n + m2)) and O(L + n) phase bounds are arguments from the earlier protocols, and measured totals do not prove them.",
    "Declared event counters exclude allocator internals, loop control, arithmetic and comparison instructions, input generation, JSON formatting, and report construction; they are operational measurements, not elapsed time or a proved bit-cost bound.",
    "Reference truth-table and raw-clause checker work are outside both arms and reported separately; the enumeration ratio is descriptive only.",
    "Budget exhaustion means unknown: no exhausted arm is reported as SAT or UNSAT, an unknown arm reports no certificate or unit, and comparisons count only cases where both arms completed; an unfinished arm's smaller work count is not a speedup.",
    "A fragment SAT certificate is an assignment of the binary fragment, not a model of the whole formula; only fragment UNSAT certificates decide the whole formula without search.",
    "UNSAT cases have no defined solution-set backbone; the subset check on derived units applies to satisfiable cases only.",
    "No required result is a positive delta; a negative result is reported with the same completeness as a positive one.",
];

pub fn json(result: &Experiment) -> String {
    let mut output = format!(
        concat!(
            "{{\n\"schema_version\":1,\"experiment\":\"fragment-interface-v1\",\"status\":\"bounded-tested\",",
            "\"proof_status\":\"no-formal-proof\",\"protocol_sha256\":{},\"arm_budget\":{},\n",
            "\"counter_model\":\"transfer-v1-events-with-graph-phases-and-unit-appends\",\n\"limitations\":["
        ),
        quoted(PROTOCOL_SHA256),
        ARM_BUDGET
    );
    for (index, limitation) in LIMITATIONS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quoted(limitation));
    }
    write!(
        output,
        concat!(
            "],\n\"corpus\":{{\"cases\":{},\"families\":{{\"random-ternary-only\":36,\"random-mixed\":108,",
            "\"chain-embedded\":12,\"contradictory-fragment\":6}},\"seeds\":{:?},\"variables\":{:?},",
            "\"ternary_densities\":{:?},\"binary_counts\":[\"0\",\"n/2\",\"n\",\"2n\"],\"control_densities\":{:?},",
            "\"chain_seed\":{},\"chain_length\":\"n - 1\",\"contradictory_seed\":{},\"cycle_lengths\":{{\"8\":3,\"10\":5,\"12\":7}},",
            "\"generator\":\"xorshift64\",\"effective_seed\":\"base_seed ^ (n << 32) ^ (t << 40) ^ (b << 48)\",",
            "\"max_variables\":12,\"max_width\":3,\"exact_earlier_overlap\":0}},\n",
            "\"observations\":[\n"
        ),
        CORPUS_CASES,
        SEEDS,
        VARIABLES,
        DENSITIES,
        CONTROL_DENSITIES,
        CHAIN_SEED,
        CONTRADICTORY_SEED
    )
    .expect("string write");
    for (index, row) in result.observations.iter().enumerate() {
        if index > 0 {
            output.push_str(",\n");
        }
        output.push_str(&observation_json(row));
    }
    write!(
        output,
        concat!(
            "\n],\n\"summary\":{},\n",
            "\"outside_arms\":{{\"reference_work_units\":{},\"checker_work_units\":{}}},\n",
            "\"comparison\":{{\"left\":\"fragment interface: graph construction, components, fragment certificate, extraction, copy and unit appends, then the common DPLL\",",
            "\"right\":\"baseline DPLL on the original formula\",",
            "\"restricted_to\":\"cases where both arms completed; every completed decision agreed with the reference\"}},\n",
            "\"encoding\":{{\"vertex_labels\":\"usize 2*(v-1) and 2*(v-1)+1, at most 24 vertices\",",
            "\"clause_indices\":\"usize positions in the original input, ternary clauses included\",",
            "\"literals\":\"i32 signed DIMACS-style\",",
            "\"counters\":\"u64 checked increments; identical in debug and release\",",
            "\"enumeration_ratio\":\"reference work units divided by arm work units, nearest thousandth\"}},\n",
            "\"families\":{{"
        ),
        result.summary.json(),
        result.summary.reference_work_units,
        result.summary.checker_work_units
    )
    .expect("string write");
    for (index, (family, summary)) in result.families.iter().enumerate() {
        if index > 0 {
            output.push_str(",\n");
        }
        write!(output, "{}:{}", quoted(family), summary.json()).expect("string write");
    }
    output.push_str("},\n\"random_mixed_by_binary_count\":{");
    for (index, (label, summary)) in result.binary_counts.iter().enumerate() {
        if index > 0 {
            output.push_str(",\n");
        }
        write!(output, "{}:{}", quoted(label), summary.json()).expect("string write");
    }
    output.push_str("},\n\"random_by_variables\":{");
    for (index, (label, summary)) in result.random_variables.iter().enumerate() {
        if index > 0 {
            output.push_str(",\n");
        }
        write!(output, "{}:{}", quoted(label), summary.json()).expect("string write");
    }
    output.push_str("},\n\"random_by_density\":{");
    for (index, (label, summary)) in result.random_densities.iter().enumerate() {
        if index > 0 {
            output.push_str(",\n");
        }
        write!(output, "{}:{}", quoted(label), summary.json()).expect("string write");
    }
    output.push_str("}\n}\n");
    output
}

pub fn summary_line(result: &Experiment) -> String {
    let s = &result.summary;
    let c = &s.fragment_vs_baseline;
    format!(
        concat!(
            "Checked {} cases in {} families ({} sat, {} unsat). Reference {}; checker {}. ",
            "Baseline {} ({} unknown, {} search nodes); fragment {} = phases {} + residual {} ",
            "({} unknown, {} search nodes, {} derived units, {} fragment-unsat). ",
            "Fragment vs baseline on {} both-complete cases: {} better, {} tied, {} worse."
        ),
        s.cases,
        result.families.len(),
        s.reference_sat,
        s.reference_unsat,
        s.reference_work_units,
        s.checker_work_units,
        s.baseline.work_units,
        s.baseline.unknown,
        s.baseline.search_nodes,
        s.fragment.total_work_units,
        s.fragment.fragment_work_units,
        s.fragment.residual_work_units,
        s.fragment.unknown,
        s.fragment.search_nodes,
        s.fragment.derived_units,
        s.fragment.fragment_unsat,
        c.both_complete,
        c.left_better,
        c.tied,
        c.left_worse
    )
}
