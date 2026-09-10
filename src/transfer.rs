//! A frozen-library transfer experiment. Truth-table oracles never run in the
//! preprocessor or DPLL solver. The operation counter is not a bit-cost proof.

use crate::Cnf;
use std::fmt::Write;

pub const PROTOCOL_SHA256: &str =
    "5a4e14e2f637e6bc6ee3cb691f76d84e556d14d4eb126e55ecd387d59e89d561";
pub const ARM_BUDGET: u64 = 1_000_000;
pub const SEEDS: [u64; 4] = [7, 41, 2026, 65537];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Work {
    pub work_units: u64,
    pub formula_checks: u64,
    pub clause_reads: u64,
    pub literal_reads: u64,
    pub clause_writes: u64,
    pub literal_writes: u64,
    pub assignments: u64,
    pub pair_checks: u64,
    pub rule_attempts: u64,
    pub search_nodes: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum Event {
    Formula,
    ClauseRead,
    LiteralRead,
    ClauseWrite,
    LiteralWrite,
    Assignment,
    Pair,
    Rule,
    Search,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    Budget,
    CounterOverflow,
    InvalidInput,
}

pub(crate) struct Meter {
    limit: u64,
    pub(crate) work: Work,
}

impl Meter {
    pub(crate) fn new(limit: u64) -> Self {
        Self {
            limit,
            work: Work::default(),
        }
    }
    pub(crate) fn tick(&mut self, event: Event) -> Result<(), Failure> {
        let next = self
            .work
            .work_units
            .checked_add(1)
            .ok_or(Failure::CounterOverflow)?;
        if next > self.limit {
            return Err(Failure::Budget);
        }
        let field = match event {
            Event::Formula => &mut self.work.formula_checks,
            Event::ClauseRead => &mut self.work.clause_reads,
            Event::LiteralRead => &mut self.work.literal_reads,
            Event::ClauseWrite => &mut self.work.clause_writes,
            Event::LiteralWrite => &mut self.work.literal_writes,
            Event::Assignment => &mut self.work.assignments,
            Event::Pair => &mut self.work.pair_checks,
            Event::Rule => &mut self.work.rule_attempts,
            Event::Search => &mut self.work.search_nodes,
        };
        *field = field.checked_add(1).ok_or(Failure::CounterOverflow)?;
        self.work.work_units = next;
        Ok(())
    }
}

impl Work {
    pub(crate) fn json(&self) -> String {
        format!(
            concat!(
                "{{\"work_units\":{},\"formula_checks\":{},\"clause_reads\":{},",
                "\"literal_reads\":{},\"clause_writes\":{},\"literal_writes\":{},",
                "\"assignments\":{},\"pair_checks\":{},\"rule_attempts\":{},\"search_nodes\":{}}}"
            ),
            self.work_units,
            self.formula_checks,
            self.clause_reads,
            self.literal_reads,
            self.clause_writes,
            self.literal_writes,
            self.assignments,
            self.pair_checks,
            self.rule_attempts,
            self.search_nodes
        )
    }
}

pub(crate) fn validate(input: &Cnf, variables: u32, meter: &mut Meter) -> Result<(), Failure> {
    if variables > 12 {
        return Err(Failure::InvalidInput);
    }
    for clause in input {
        meter.tick(Event::ClauseRead)?;
        for &lit in clause {
            meter.tick(Event::LiteralRead)?;
            if lit == 0 || lit == i32::MIN || lit.unsigned_abs() > variables {
                return Err(Failure::InvalidInput);
            }
        }
    }
    Ok(())
}

pub(crate) fn copy(input: &Cnf, meter: &mut Meter) -> Result<Cnf, Failure> {
    let mut result = Vec::new();
    for clause in input {
        meter.tick(Event::ClauseRead)?;
        meter.tick(Event::ClauseWrite)?;
        let mut out = Vec::new();
        for &lit in clause {
            meter.tick(Event::LiteralRead)?;
            meter.tick(Event::LiteralWrite)?;
            out.push(lit);
        }
        result.push(out);
    }
    Ok(result)
}

fn value(lit: i32, assignment: u64, meter: &mut Meter) -> Result<bool, Failure> {
    meter.tick(Event::LiteralRead)?;
    Ok(((assignment >> (lit.unsigned_abs() - 1)) & 1 != 0) == (lit > 0))
}

fn satisfies(input: &Cnf, assignment: u64, meter: &mut Meter) -> Result<bool, Failure> {
    meter.tick(Event::Formula)?;
    for clause in input {
        meter.tick(Event::ClauseRead)?;
        let mut yes = false;
        for &lit in clause {
            if value(lit, assignment, meter)? {
                yes = true;
                break;
            }
        }
        if !yes {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Independent bounded labeling, outside both compared algorithms.
pub fn reference(input: &Cnf, variables: u32) -> Result<(bool, Work), Failure> {
    let mut meter = Meter::new(u64::MAX);
    validate(input, variables, &mut meter)?;
    let mut sat = false;
    // Enumerate the full domain, including after a witness, for explicit coverage.
    for assignment in 0..(1_u64 << variables) {
        meter.tick(Event::Assignment)?;
        sat |= satisfies(input, assignment, &mut meter)?;
    }
    Ok((sat, meter.work))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub premises: [[i32; 2]; 2],
    pub conclusion: i32,
}

impl Rule {
    pub(crate) fn json(&self) -> String {
        format!(
            "{{\"premises\":{:?},\"conclusion\":{}}}",
            self.premises, self.conclusion
        )
    }
}

#[derive(Clone, Debug)]
pub struct FrozenLibrary {
    rules: Vec<Rule>,
}

impl FrozenLibrary {
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }
}

#[derive(Clone, Debug)]
pub struct MiningCandidate {
    pub rule: Rule,
    pub entailed: bool,
    pub satisfying_assignments: u64,
    pub first_counterexample_assignment: Option<u64>,
}

pub struct Mining {
    pub library: FrozenLibrary,
    pub candidates: Vec<MiningCandidate>,
    pub work: Work,
}

pub fn mine() -> Result<Mining, Failure> {
    let clauses = [[1, 2], [1, -2], [-1, 2], [-1, -2]];
    let mut meter = Meter::new(ARM_BUDGET);
    let mut candidates = Vec::new();
    let mut rules = Vec::new();
    for i in 0..clauses.len() {
        for j in (i + 1)..clauses.len() {
            for conclusion in [1, -1, 2, -2] {
                meter.tick(Event::Rule)?;
                let premises = [clauses[i], clauses[j]];
                // Account for creating the candidate's two-clause premise value.
                let mut input = Vec::new();
                for clause in &premises {
                    meter.tick(Event::ClauseRead)?;
                    meter.tick(Event::ClauseWrite)?;
                    let mut copied = Vec::new();
                    for &lit in clause {
                        meter.tick(Event::LiteralRead)?;
                        meter.tick(Event::LiteralWrite)?;
                        copied.push(lit);
                    }
                    input.push(copied);
                }
                let mut first_counterexample_assignment = None;
                let mut satisfying_assignments = 0_u64;
                for assignment in 0..4 {
                    meter.tick(Event::Assignment)?;
                    if satisfies(&input, assignment, &mut meter)? {
                        satisfying_assignments = satisfying_assignments
                            .checked_add(1)
                            .ok_or(Failure::CounterOverflow)?;
                        if !value(conclusion, assignment, &mut meter)?
                            && first_counterexample_assignment.is_none()
                        {
                            first_counterexample_assignment = Some(assignment);
                        }
                    }
                }
                let entailed = first_counterexample_assignment.is_none();
                let rule = Rule {
                    premises,
                    conclusion,
                };
                if entailed {
                    meter.tick(Event::ClauseWrite)?;
                    for _ in 0..5 {
                        meter.tick(Event::LiteralWrite)?;
                    }
                    rules.push(rule.clone());
                }
                candidates.push(MiningCandidate {
                    rule,
                    entailed,
                    satisfying_assignments,
                    first_counterexample_assignment,
                });
            }
        }
    }
    Ok(Mining {
        library: FrozenLibrary { rules },
        candidates,
        work: meter.work,
    })
}

fn mapped(lit: i32, mapping: &[i32; 2], meter: &mut Meter) -> Result<i32, Failure> {
    meter.tick(Event::LiteralRead)?;
    meter.tick(Event::LiteralRead)?;
    let result = mapping[(lit.unsigned_abs() - 1) as usize];
    if lit > 0 {
        Ok(result)
    } else {
        result.checked_neg().ok_or(Failure::CounterOverflow)
    }
}

fn match_rule(
    rule: &Rule,
    first: &[i32],
    second: &[i32],
    swap: bool,
    meter: &mut Meter,
) -> Result<Option<i32>, Failure> {
    meter.tick(Event::Rule)?;
    let mut mapping = [0_i32; 2];
    for (position, &source) in rule.premises[0].iter().enumerate() {
        meter.tick(Event::LiteralRead)?;
        meter.tick(Event::LiteralRead)?;
        let target = first[if swap { 1 - position } else { position }];
        meter.tick(Event::LiteralWrite)?;
        mapping[(source.unsigned_abs() - 1) as usize] = if source > 0 {
            target
        } else {
            target.checked_neg().ok_or(Failure::CounterOverflow)?
        };
    }
    if mapping[0].unsigned_abs() == mapping[1].unsigned_abs() {
        return Ok(None);
    }
    let a = mapped(rule.premises[1][0], &mapping, meter)?;
    let b = mapped(rule.premises[1][1], &mapping, meter)?;
    meter.tick(Event::LiteralRead)?;
    meter.tick(Event::LiteralRead)?;
    if (second[0] == a && second[1] == b) || (second[0] == b && second[1] == a) {
        Ok(Some(mapped(rule.conclusion, &mapping, meter)?))
    } else {
        Ok(None)
    }
}

pub(crate) fn preprocess(
    input: &Cnf,
    variables: u32,
    library: &FrozenLibrary,
    meter: &mut Meter,
) -> Result<(Cnf, Vec<i32>), Failure> {
    validate(input, variables, meter)?;
    let mut output = copy(input, meter)?;
    let mut binary = Vec::new();
    for (index, clause) in input.iter().enumerate() {
        meter.tick(Event::ClauseRead)?;
        if clause.len() == 2 {
            meter.tick(Event::LiteralRead)?;
            meter.tick(Event::LiteralRead)?;
            if clause[0].unsigned_abs() != clause[1].unsigned_abs() {
                // One stored index is accounted as a scalar write.
                meter.tick(Event::LiteralWrite)?;
                binary.push(index);
            }
        }
    }
    let mut derived = Vec::new();
    for i in 0..binary.len() {
        for j in (i + 1)..binary.len() {
            meter.tick(Event::Pair)?;
            meter.tick(Event::ClauseRead)?;
            meter.tick(Event::ClauseRead)?;
            let first = &input[binary[i]];
            let second = &input[binary[j]];
            for rule in library.rules() {
                for swap in [false, true] {
                    if let Some(lit) = match_rule(rule, first, second, swap, meter)? {
                        let mut already = false;
                        for &known in &derived {
                            meter.tick(Event::LiteralRead)?;
                            if known == lit {
                                already = true;
                                break;
                            }
                        }
                        if !already {
                            meter.tick(Event::LiteralWrite)?;
                            derived.push(lit);
                            meter.tick(Event::ClauseWrite)?;
                            meter.tick(Event::LiteralWrite)?;
                            output.push(vec![lit]);
                        }
                    }
                }
            }
        }
    }
    Ok((output, derived))
}

fn restrict(input: &Cnf, chosen: i32, meter: &mut Meter) -> Result<Cnf, Failure> {
    meter.tick(Event::Assignment)?;
    let opposite = chosen.checked_neg().ok_or(Failure::CounterOverflow)?;
    let mut output = Vec::new();
    for clause in input {
        meter.tick(Event::ClauseRead)?;
        let mut satisfied = false;
        for &lit in clause {
            meter.tick(Event::LiteralRead)?;
            if lit == chosen {
                satisfied = true;
                break;
            }
        }
        if satisfied {
            continue;
        }
        meter.tick(Event::ClauseWrite)?;
        let mut out = Vec::new();
        for &lit in clause {
            meter.tick(Event::LiteralRead)?;
            if lit != opposite {
                meter.tick(Event::LiteralWrite)?;
                out.push(lit);
            }
        }
        output.push(out);
    }
    Ok(output)
}

fn dpll(mut input: Cnf, meter: &mut Meter) -> Result<bool, Failure> {
    meter.tick(Event::Search)?;
    loop {
        meter.tick(Event::Formula)?;
        if input.is_empty() {
            return Ok(true);
        }
        let mut first_unit = None;
        for clause in &input {
            meter.tick(Event::ClauseRead)?;
            if clause.is_empty() {
                return Ok(false);
            }
            if clause.len() == 1 && first_unit.is_none() {
                meter.tick(Event::LiteralRead)?;
                first_unit = Some(clause[0]);
            }
        }
        if let Some(lit) = first_unit {
            input = restrict(&input, lit, meter)?;
        } else {
            break;
        }
    }
    let mut variable = u32::MAX;
    for clause in &input {
        meter.tick(Event::ClauseRead)?;
        for &lit in clause {
            meter.tick(Event::LiteralRead)?;
            variable = variable.min(lit.unsigned_abs());
        }
    }
    let positive = i32::try_from(variable).map_err(|_| Failure::InvalidInput)?;
    if dpll(restrict(&input, positive, meter)?, meter)? {
        return Ok(true);
    }
    dpll(restrict(&input, -positive, meter)?, meter)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Sat,
    Unsat,
    Unknown,
}

impl Outcome {
    fn name(&self) -> &'static str {
        match self {
            Self::Sat => "sat",
            Self::Unsat => "unsat",
            Self::Unknown => "unknown-budget",
        }
    }
    pub fn agrees(&self, label: bool) -> bool {
        match self {
            Self::Sat => label,
            Self::Unsat => !label,
            Self::Unknown => true,
        }
    }
}

pub struct Arm {
    pub outcome: Outcome,
    pub preprocessing: Work,
    pub residual: Work,
    pub derived_units: Vec<i32>,
}

impl Arm {
    pub fn total_work(&self) -> u64 {
        self.preprocessing
            .work_units
            .checked_add(self.residual.work_units)
            .expect("shared budget bounds sum")
    }
    pub(crate) fn json(&self) -> String {
        format!("{{\"outcome\":\"{}\",\"preprocessing\":{},\"residual\":{},\"total_work_units\":{},\"derived_units\":{:?}}}",
            self.outcome.name(), self.preprocessing.json(), self.residual.json(), self.total_work(), self.derived_units)
    }
}

/// Both modes call precisely the same DPLL routine. A transfer arm's matching
/// work consumes its total budget before the common solver starts.
pub fn solve(
    input: &Cnf,
    variables: u32,
    library: Option<&FrozenLibrary>,
    budget: u64,
) -> Result<Arm, Failure> {
    let mut preparation = Meter::new(budget);
    let mut derived_units = Vec::new();
    let processed = if let Some(library) = library {
        match preprocess(input, variables, library, &mut preparation) {
            Ok((formula, units)) => {
                derived_units = units;
                Some(formula)
            }
            Err(Failure::Budget) => {
                return Ok(Arm {
                    outcome: Outcome::Unknown,
                    preprocessing: preparation.work,
                    residual: Work::default(),
                    derived_units,
                })
            }
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    let remaining = budget
        .checked_sub(preparation.work.work_units)
        .ok_or(Failure::CounterOverflow)?;
    let mut residual = Meter::new(remaining);
    let source = processed.as_ref().unwrap_or(input);
    let result = (|| {
        validate(source, variables, &mut residual)?;
        let formula = copy(source, &mut residual)?;
        dpll(formula, &mut residual)
    })();
    let outcome = match result {
        Ok(true) => Outcome::Sat,
        Ok(false) => Outcome::Unsat,
        Err(Failure::Budget) => Outcome::Unknown,
        Err(error) => return Err(error),
    };
    Ok(Arm {
        outcome,
        preprocessing: preparation.work,
        residual: residual.work,
        derived_units,
    })
}

#[derive(Clone, Debug)]
pub struct Case {
    pub id: String,
    pub family: &'static str,
    pub variables: u32,
    pub input: Cnf,
    pub seed: Option<u64>,
    pub density: Option<u32>,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn generated(variables: u32, density: u32, seed: u64, mixed: bool) -> Cnf {
    let mut rng = Rng(seed);
    let mut formula = Vec::new();
    for _ in 0..variables * density {
        let width = if mixed && rng.next() & 1 == 0 { 2 } else { 3 };
        let mut clause: Vec<i32> = Vec::new();
        while clause.len() < width {
            let variable = (rng.next() % u64::from(variables) + 1) as i32;
            if clause.iter().any(|lit| lit.abs() == variable) {
                continue;
            }
            let literal = if rng.next() & 1 == 0 {
                variable
            } else {
                -variable
            };
            clause.push(literal);
        }
        formula.push(clause);
    }
    formula
}

pub fn corpus() -> Vec<Case> {
    let mut cases = Vec::new();
    for seed in SEEDS {
        for variables in [6, 8, 10, 12] {
            for density in [2, 4, 6] {
                for mixed in [false, true] {
                    let family = if mixed { "random-mixed" } else { "random-3cnf" };
                    cases.push(Case {
                        id: format!("{family}-n{variables}-d{density}-s{seed}"),
                        family,
                        variables,
                        input: generated(variables, density, seed, mixed),
                        seed: Some(seed),
                        density: Some(density),
                    });
                }
            }
        }
    }
    for variables in [6, 8, 10, 12] {
        for positive in [true, false] {
            let family = if positive {
                "forced-true"
            } else {
                "forced-false"
            };
            let input = (1..=variables / 2)
                .flat_map(|i| {
                    let x = (2 * i - 1) as i32 * if positive { 1 } else { -1 };
                    let y = (2 * i) as i32;
                    [vec![x, y], vec![x, -y]]
                })
                .collect();
            cases.push(Case {
                id: format!("{family}-n{variables}"),
                family,
                variables,
                input,
                seed: None,
                density: None,
            });
        }
    }
    for variables in 5..=12 {
        let input = (1..=variables)
            .flat_map(|i| {
                let a = i as i32;
                let b = (i % variables + 1) as i32;
                [vec![a, b], vec![-a, -b]]
            })
            .collect();
        cases.push(Case {
            id: format!("parity-cycle-n{variables}"),
            family: "parity-cycle",
            variables,
            input,
            seed: None,
            density: None,
        });
    }
    for (pigeons, holes) in [(3, 2), (4, 3)] {
        let mut input: Cnf = (0..pigeons)
            .map(|p| (1..=holes).map(|h| p * holes + h).collect())
            .collect();
        for h in 1..=holes {
            for a in 0..pigeons {
                for b in a + 1..pigeons {
                    input.push(vec![-(a * holes + h), -(b * holes + h)]);
                }
            }
        }
        cases.push(Case {
            id: format!("pigeonhole-{pigeons}-{holes}"),
            family: "pigeonhole",
            variables: (pigeons * holes) as u32,
            input,
            seed: None,
            density: None,
        });
    }
    for variables in [6, 8, 10, 12] {
        let a = variables as i32 - 2;
        let b = variables as i32 - 1;
        let c = variables as i32;
        let input = (0..8)
            .map(|signs| {
                vec![
                    if signs & 1 == 0 { a } else { -a },
                    if signs & 2 == 0 { b } else { -b },
                    if signs & 4 == 0 { c } else { -c },
                ]
            })
            .collect();
        cases.push(Case {
            id: format!("ternary-cube-n{variables}"),
            family: "ternary-cube",
            variables,
            input,
            seed: None,
            density: None,
        });
        let forced = if variables % 4 == 0 { c } else { -c };
        let input = vec![
            vec![forced, a, b],
            vec![forced, a, -b],
            vec![forced, -a, b],
            vec![forced, -a, -b],
        ];
        cases.push(Case {
            id: format!("hidden-backbone-n{variables}"),
            family: "hidden-backbone",
            variables,
            input,
            seed: None,
            density: None,
        });
    }
    cases
}

pub struct Observation {
    pub case: Case,
    pub reference_sat: bool,
    pub reference_work: Work,
    pub baseline: Arm,
    pub transfer: Arm,
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub cases: u64,
    pub baseline_work_units: u64,
    pub transfer_preprocessing_work_units: u64,
    pub transfer_residual_work_units: u64,
    pub transfer_work_units: u64,
    pub reference_work_units: u64,
    pub derived_units: u64,
    pub baseline_search_nodes: u64,
    pub transfer_search_nodes: u64,
    pub baseline_unknown: u64,
    pub transfer_unknown: u64,
    pub both_complete: u64,
    pub transfer_better: u64,
    pub tied: u64,
    pub transfer_worse: u64,
}

impl Summary {
    fn include(&mut self, row: &Observation) -> Result<(), Failure> {
        fn add(a: &mut u64, b: u64) -> Result<(), Failure> {
            *a = a.checked_add(b).ok_or(Failure::CounterOverflow)?;
            Ok(())
        }
        add(&mut self.cases, 1)?;
        add(&mut self.baseline_work_units, row.baseline.total_work())?;
        add(&mut self.transfer_work_units, row.transfer.total_work())?;
        add(
            &mut self.transfer_preprocessing_work_units,
            row.transfer.preprocessing.work_units,
        )?;
        add(
            &mut self.transfer_residual_work_units,
            row.transfer.residual.work_units,
        )?;
        add(
            &mut self.reference_work_units,
            row.reference_work.work_units,
        )?;
        add(
            &mut self.derived_units,
            row.transfer.derived_units.len() as u64,
        )?;
        add(
            &mut self.baseline_search_nodes,
            row.baseline.residual.search_nodes,
        )?;
        add(
            &mut self.transfer_search_nodes,
            row.transfer.residual.search_nodes,
        )?;
        if row.baseline.outcome == Outcome::Unknown {
            add(&mut self.baseline_unknown, 1)?;
        }
        if row.transfer.outcome == Outcome::Unknown {
            add(&mut self.transfer_unknown, 1)?;
        }
        if row.baseline.outcome != Outcome::Unknown && row.transfer.outcome != Outcome::Unknown {
            add(&mut self.both_complete, 1)?;
            match row.transfer.total_work().cmp(&row.baseline.total_work()) {
                std::cmp::Ordering::Less => add(&mut self.transfer_better, 1)?,
                std::cmp::Ordering::Equal => add(&mut self.tied, 1)?,
                std::cmp::Ordering::Greater => add(&mut self.transfer_worse, 1)?,
            }
        }
        Ok(())
    }
    fn json(&self) -> String {
        format!(concat!("{{\"cases\":{},\"baseline_work_units\":{},\"transfer_preprocessing_work_units\":{},",
            "\"transfer_residual_work_units\":{},\"transfer_work_units\":{},\"reference_work_units\":{},",
            "\"derived_units\":{},\"baseline_search_nodes\":{},\"transfer_search_nodes\":{},",
            "\"baseline_unknown\":{},\"transfer_unknown\":{},\"both_complete\":{},",
            "\"transfer_better\":{},\"tied\":{},\"transfer_worse\":{}}}"), self.cases, self.baseline_work_units,
            self.transfer_preprocessing_work_units, self.transfer_residual_work_units, self.transfer_work_units,
            self.reference_work_units, self.derived_units, self.baseline_search_nodes, self.transfer_search_nodes,
            self.baseline_unknown, self.transfer_unknown, self.both_complete, self.transfer_better, self.tied, self.transfer_worse)
    }
}

pub struct Experiment {
    pub mining: Mining,
    pub observations: Vec<Observation>,
    pub summary: Summary,
    pub families: std::collections::BTreeMap<&'static str, Summary>,
}

pub fn experiment() -> Result<Experiment, Failure> {
    let mining = mine()?;
    // The actual immutable rule objects exist before any held-out inputs or labels.
    let cases = corpus();
    let mut observations = Vec::new();
    let mut summary = Summary::default();
    let mut families = std::collections::BTreeMap::<&'static str, Summary>::new();
    for case in cases {
        let (reference_sat, reference_work) = reference(&case.input, case.variables)?;
        let baseline = solve(&case.input, case.variables, None, ARM_BUDGET)?;
        let transfer = solve(
            &case.input,
            case.variables,
            Some(&mining.library),
            ARM_BUDGET,
        )?;
        if !baseline.outcome.agrees(reference_sat) || !transfer.outcome.agrees(reference_sat) {
            return Err(Failure::InvalidInput);
        }
        let row = Observation {
            case,
            reference_sat,
            reference_work,
            baseline,
            transfer,
        };
        summary.include(&row)?;
        families.entry(row.case.family).or_default().include(&row)?;
        observations.push(row);
    }
    Ok(Experiment {
        mining,
        observations,
        summary,
        families,
    })
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\u{20}' => {
                write!(&mut out, "\\u{:04x}", c as u32).expect("String writing");
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn json(result: &Experiment) -> Result<String, Failure> {
    let acquisition_plus_online = result
        .mining
        .work
        .work_units
        .checked_add(result.summary.transfer_work_units)
        .ok_or(Failure::CounterOverflow)?;
    let mut output = format!(concat!("{{\n\"schema_version\":1,\n\"experiment\":\"clue-transfer-v1\",\n",
        "\"status\":\"bounded-tested\",\n\"protocol_sha256\":\"{}\",\n",
        "\"proof_status\":\"informal-general-argument-not-kernel-checked\",\n",
        "\"training\":{{\"variables\":2,\"premise_pairs\":6,\"conclusions_per_pair\":4,\"candidate_implications\":24,",
        "\"assignments_per_candidate\":4,\"accepted_rules\":{},\"acquisition\":{},\"candidates\":["),
        PROTOCOL_SHA256, result.mining.library.rules.len(), result.mining.work.json());
    for (index, candidate) in result.mining.candidates.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        let counterexample = candidate
            .first_counterexample_assignment
            .map_or("null".into(), |x| x.to_string());
        write!(&mut output, "{{\"rule\":{},\"entailed\":{},\"satisfying_assignments\":{},\"first_counterexample_assignment\":{}}}",
            candidate.rule.json(), candidate.entailed, candidate.satisfying_assignments, counterexample).expect("String writing");
    }
    let rules: Vec<_> = result.mining.library.rules.iter().map(Rule::json).collect();
    write!(&mut output, concat!("]}},\n\"frozen_library\":{{\"rules\":[{}],\"rule_instances\":{},\"schemas_up_to_signed_renaming\":1,",
        "\"schema\":\"known-binary-resolution-to-unit\",\"frozen_before_heldout\":true}},\n",
        "\"corpus\":{{\"generator\":\"xorshift64-v1\",\"seeds\":{:?},\"cases\":122,\"max_variables\":12}},\n",
        "\"arm_budget_work_units\":{},\n\"observations\":["), rules.join(","), rules.len(), SEEDS, ARM_BUDGET).expect("String writing");
    for (index, row) in result.observations.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        let seed = row.case.seed.map_or("null".into(), |x| x.to_string());
        let density = row.case.density.map_or("null".into(), |x| x.to_string());
        let used: std::collections::BTreeSet<_> = row
            .case
            .input
            .iter()
            .flatten()
            .map(|lit| lit.unsigned_abs())
            .collect();
        write!(&mut output, concat!("\n{{\"id\":{},\"family\":{},\"variables\":{},\"used_variables\":{},\"seed\":{},\"density\":{},",
            "\"input\":{:?},\"reference_sat\":{},\"reference_work\":{},\"baseline\":{},\"transfer\":{}}}"),
            quote(&row.case.id), quote(row.case.family), row.case.variables, used.len(), seed, density, row.case.input,
            row.reference_sat, row.reference_work.json(), row.baseline.json(), row.transfer.json()).expect("String writing");
    }
    write!(
        &mut output,
        "\n],\n\"summary\":{},\n\"acquisition_plus_transfer_work_units\":{},\n\"families\":{{",
        result.summary.json(),
        acquisition_plus_online
    )
    .expect("String writing");
    for (index, (family, summary)) in result.families.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(&mut output, "{}:{}", quote(family), summary.json()).expect("String writing");
    }
    output.push_str(concat!("},\n\"limitations\":[\"Known resolution rule mined in a hand-scoped grammar\",",
        "\"Finite nonrepresentative corpus of at most 12 declared variables\",",
        "\"Ternary-cube and hidden-backbone controls use unused declared-variable padding; not scaling evidence\",",
        "\"One root-only learned preprocessing pass; no learned closure inside search\",",
        "\"Common DPLL is a transparent toy baseline, not a competitive SAT solver\",",
        "\"Event costs include declared traversals, copies and matching; not allocator overhead or proved bit steps\",",
        "\"Oracle labeling work is separate from algorithm runtime\",",
        "\"No evolutionary advantage, novel theorem or P versus NP result\"]\n}\n"));
    Ok(output)
}
