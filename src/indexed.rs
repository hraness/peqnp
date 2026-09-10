//! A metered sorted-array implementation of the frozen binary resolution schema.
//! The index changes matching cost, not the rule library or derived-unit order.

use crate::transfer::{self, Arm, Event, Failure, FrozenLibrary, Meter, Outcome, Rule, Work};
use crate::Cnf;
use std::cmp::Ordering;
use std::fmt::Write;

pub const ARM_BUDGET: u64 = 1_000_000;
pub const SEEDS: [u64; 4] = [1201, 5179, 65539, 104729];
// SHA-256 of experiments/indexed-transfer-protocol.md at its freeze commit 205f774.
pub const PROTOCOL_SHA256: &str =
    "8e3239abca6796e86dc4463b51bdbfb081a1f3c33e14d8bc632e31332910a043";

#[derive(Clone, Debug)]
pub struct CompiledLibrary {
    enabled: bool,
    source_rule_instances: usize,
}

impl CompiledLibrary {
    pub fn source_rule_instances(&self) -> usize {
        self.source_rule_instances
    }
    pub fn schemas(&self) -> usize {
        usize::from(self.enabled)
    }
}

pub struct Compilation {
    pub library: CompiledLibrary,
    pub work: Work,
}

fn ticks(meter: &mut Meter, event: Event, count: usize) -> Result<(), Failure> {
    for _ in 0..count {
        meter.tick(event)?;
    }
    Ok(())
}

fn increment(value: &mut u64) -> Result<(), Failure> {
    *value = value.checked_add(1).ok_or(Failure::CounterOverflow)?;
    Ok(())
}

fn check_rule(rule: &Rule, meter: &mut Meter) -> Result<(), Failure> {
    meter.tick(Event::Rule)?;
    ticks(meter, Event::ClauseRead, 2)?;
    ticks(meter, Event::LiteralRead, 5)?;
    if rule.conclusion == 0
        || rule.conclusion.unsigned_abs() > 2
        || rule
            .premises
            .iter()
            .flatten()
            .any(|&lit| lit == 0 || lit.unsigned_abs() > 2)
    {
        return Err(Failure::InvalidInput);
    }
    let mut pivots = [0; 2];
    for (index, clause) in rule.premises.iter().enumerate() {
        meter.tick(Event::Pair)?;
        ticks(meter, Event::LiteralRead, 3)?;
        if clause[0].unsigned_abs() == clause[1].unsigned_abs() {
            return Err(Failure::InvalidInput);
        }
        let pivot = if clause[0] == rule.conclusion {
            clause[1]
        } else if clause[1] == rule.conclusion {
            clause[0]
        } else {
            return Err(Failure::InvalidInput);
        };
        meter.tick(Event::LiteralWrite)?;
        pivots[index] = pivot;
    }
    meter.tick(Event::Pair)?;
    ticks(meter, Event::LiteralRead, 2)?;
    if pivots[0].checked_neg() != Some(pivots[1]) {
        return Err(Failure::InvalidInput);
    }
    Ok(())
}

/// Compile only the schema actually justified by each supplied rule object.
/// Compilation is separate one-time work, not an online reference oracle.
pub fn compile(library: &FrozenLibrary, budget: u64) -> Result<Compilation, Failure> {
    let mut meter = Meter::new(budget);
    let mut enabled = false;
    for rule in library.rules() {
        check_rule(rule, &mut meter)?;
        meter.tick(Event::LiteralRead)?; // Look up the already-emitted schema flag.
        if !enabled {
            meter.tick(Event::LiteralWrite)?;
            enabled = true;
        }
    }
    Ok(Compilation {
        library: CompiledLibrary {
            enabled,
            source_rule_instances: library.rules().len(),
        },
        work: meter.work,
    })
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexStats {
    pub entries: u64,
    pub key_comparisons: u64,
    pub group_lookups: u64,
    pub entry_swaps: u64,
    pub peak_stored_scalar_cells: u64,
}

impl IndexStats {
    fn storage(&mut self, entries: usize, witnesses: usize, units: usize) -> Result<(), Failure> {
        let cells = entries
            .checked_add(witnesses)
            .and_then(|n| n.checked_mul(3))
            .and_then(|n| n.checked_add(units))
            .ok_or(Failure::CounterOverflow)?;
        self.peak_stored_scalar_cells = self
            .peak_stored_scalar_cells
            .max(u64::try_from(cells).map_err(|_| Failure::CounterOverflow)?);
        Ok(())
    }
    fn json(&self) -> String {
        format!("{{\"entries\":{},\"key_comparisons\":{},\"group_lookups\":{},\"entry_swaps\":{},\"peak_stored_scalar_cells\":{}}}",
            self.entries, self.key_comparisons, self.group_lookups, self.entry_swaps, self.peak_stored_scalar_cells)
    }
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    shared: i32,
    pivot: i32,
    clause: usize,
}
#[derive(Clone, Copy, Debug)]
struct Witness {
    unit: i32,
    first: usize,
    second: usize,
}

fn comparison(meter: &mut Meter, stats: &mut IndexStats) -> Result<(), Failure> {
    meter.tick(Event::Pair)?;
    increment(&mut stats.key_comparisons)
}

fn entry_order(
    a: &Entry,
    b: &Entry,
    meter: &mut Meter,
    stats: &mut IndexStats,
) -> Result<Ordering, Failure> {
    comparison(meter, stats)?;
    ticks(meter, Event::LiteralRead, 2)?;
    let shared = a.shared.cmp(&b.shared);
    if shared != Ordering::Equal {
        return Ok(shared);
    }
    ticks(meter, Event::LiteralRead, 2)?;
    let pivot = a.pivot.unsigned_abs().cmp(&b.pivot.unsigned_abs());
    if pivot != Ordering::Equal {
        return Ok(pivot);
    }
    ticks(meter, Event::LiteralRead, 2)?;
    Ok(a.clause.cmp(&b.clause))
}

fn witness_order(
    a: &Witness,
    b: &Witness,
    meter: &mut Meter,
    stats: &mut IndexStats,
) -> Result<Ordering, Failure> {
    comparison(meter, stats)?;
    ticks(meter, Event::LiteralRead, 2)?;
    let first = a.first.cmp(&b.first);
    if first != Ordering::Equal {
        return Ok(first);
    }
    ticks(meter, Event::LiteralRead, 2)?;
    Ok(a.second.cmp(&b.second))
}

type Compare<T> = fn(&T, &T, &mut Meter, &mut IndexStats) -> Result<Ordering, Failure>;

fn swap_three_scalars<T>(
    values: &mut [T],
    a: usize,
    b: usize,
    meter: &mut Meter,
    stats: &mut IndexStats,
) -> Result<(), Failure> {
    // Both private element types have exactly three logical scalar fields.
    ticks(meter, Event::LiteralRead, 6)?;
    ticks(meter, Event::LiteralWrite, 6)?;
    values.swap(a, b);
    increment(&mut stats.entry_swaps)
}

fn sift<T>(
    values: &mut [T],
    mut root: usize,
    end: usize,
    compare: Compare<T>,
    meter: &mut Meter,
    stats: &mut IndexStats,
) -> Result<(), Failure> {
    loop {
        let left = root
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or(Failure::CounterOverflow)?;
        if left >= end {
            return Ok(());
        }
        let mut child = left;
        let right = left.checked_add(1).ok_or(Failure::CounterOverflow)?;
        if right < end && compare(&values[left], &values[right], meter, stats)? == Ordering::Less {
            child = right;
        }
        if compare(&values[root], &values[child], meter, stats)? != Ordering::Less {
            return Ok(());
        }
        swap_three_scalars(values, root, child, meter, stats)?;
        root = child;
    }
}

fn heapsort<T>(
    values: &mut [T],
    compare: Compare<T>,
    meter: &mut Meter,
    stats: &mut IndexStats,
) -> Result<(), Failure> {
    meter.tick(Event::Formula)?;
    let len = values.len();
    for root in (0..len / 2).rev() {
        sift(values, root, len, compare, meter, stats)?;
    }
    for end in (1..len).rev() {
        swap_three_scalars(values, 0, end, meter, stats)?;
        sift(values, 0, end, compare, meter, stats)?;
    }
    Ok(())
}

fn same_group(
    a: &Entry,
    b: &Entry,
    meter: &mut Meter,
    stats: &mut IndexStats,
) -> Result<bool, Failure> {
    increment(&mut stats.group_lookups)?;
    comparison(meter, stats)?;
    ticks(meter, Event::LiteralRead, 2)?;
    if a.shared != b.shared {
        return Ok(false);
    }
    ticks(meter, Event::LiteralRead, 2)?;
    Ok(a.pivot.unsigned_abs() == b.pivot.unsigned_abs())
}

fn retain_witness(
    best: &mut Option<Witness>,
    proposal: Witness,
    meter: &mut Meter,
    stats: &mut IndexStats,
) -> Result<(), Failure> {
    meter.tick(Event::LiteralRead)?;
    if let Some(known) = best {
        if witness_order(&proposal, known, meter, stats)? != Ordering::Less {
            return Ok(());
        }
    }
    ticks(meter, Event::LiteralRead, 3)?;
    ticks(meter, Event::LiteralWrite, 3)?;
    *best = Some(proposal);
    Ok(())
}

fn preprocess(
    input: &Cnf,
    variables: u32,
    library: &CompiledLibrary,
    meter: &mut Meter,
    stats: &mut IndexStats,
) -> Result<(Cnf, Vec<i32>), Failure> {
    transfer::validate(input, variables, meter)?;
    let mut output = transfer::copy(input, meter)?;
    meter.tick(Event::LiteralRead)?;
    if !library.enabled {
        return Ok((output, Vec::new()));
    }
    let mut entries = Vec::new();
    for (index, clause) in input.iter().enumerate() {
        meter.tick(Event::ClauseRead)?;
        if clause.len() != 2 {
            continue;
        }
        ticks(meter, Event::LiteralRead, 2)?;
        if clause[0].unsigned_abs() == clause[1].unsigned_abs() {
            continue;
        }
        for position in [0, 1] {
            ticks(meter, Event::LiteralRead, 2)?;
            ticks(meter, Event::LiteralWrite, 3)?;
            entries.push(Entry {
                shared: clause[position],
                pivot: clause[1 - position],
                clause: index,
            });
            increment(&mut stats.entries)?;
            stats.storage(entries.len(), 0, 0)?;
        }
    }
    heapsort(&mut entries, entry_order, meter, stats)?;
    let mut witnesses = Vec::new();
    let mut start = 0;
    while start < entries.len() {
        meter.tick(Event::Formula)?;
        meter.tick(Event::LiteralRead)?;
        meter.tick(Event::LiteralWrite)?;
        let shared = entries[start].shared;
        let mut best = None;
        while start < entries.len() {
            comparison(meter, stats)?;
            ticks(meter, Event::LiteralRead, 2)?;
            if entries[start].shared != shared {
                break;
            }
            let mut end = start;
            let mut positive = None;
            let mut negative = None;
            while end < entries.len() && same_group(&entries[start], &entries[end], meter, stats)? {
                ticks(meter, Event::LiteralRead, 2)?;
                let selected = if entries[end].pivot > 0 {
                    &mut positive
                } else {
                    &mut negative
                };
                meter.tick(Event::LiteralRead)?;
                // Entries in each group are ordered by original clause index.
                if selected.is_none() {
                    meter.tick(Event::LiteralWrite)?;
                    *selected = Some(entries[end].clause);
                }
                end = end.checked_add(1).ok_or(Failure::CounterOverflow)?;
            }
            ticks(meter, Event::LiteralRead, 2)?;
            if let (Some(a), Some(b)) = (positive, negative) {
                ticks(meter, Event::LiteralRead, 3)?;
                ticks(meter, Event::LiteralWrite, 3)?;
                let proposal = Witness {
                    unit: shared,
                    first: a.min(b),
                    second: a.max(b),
                };
                retain_witness(&mut best, proposal, meter, stats)?;
            }
            start = end;
        }
        meter.tick(Event::LiteralRead)?;
        if let Some(witness) = best {
            ticks(meter, Event::LiteralRead, 3)?;
            ticks(meter, Event::LiteralWrite, 3)?;
            witnesses.push(witness);
            stats.storage(entries.len(), witnesses.len(), 0)?;
        }
    }
    heapsort(&mut witnesses, witness_order, meter, stats)?;
    let mut units = Vec::new();
    for witness in &witnesses {
        meter.tick(Event::LiteralRead)?;
        meter.tick(Event::LiteralWrite)?;
        units.push(witness.unit);
        meter.tick(Event::ClauseWrite)?;
        meter.tick(Event::LiteralWrite)?;
        output.push(vec![witness.unit]);
        stats.storage(entries.len(), witnesses.len(), units.len())?;
    }
    Ok((output, units))
}

pub struct IndexedArm {
    pub arm: Arm,
    pub index: IndexStats,
}

impl IndexedArm {
    fn json(&self) -> String {
        format!(
            "{{\"arm\":{},\"index\":{}}}",
            self.arm.json(),
            self.index.json()
        )
    }
}

pub fn solve(
    input: &Cnf,
    variables: u32,
    library: &CompiledLibrary,
    budget: u64,
) -> Result<IndexedArm, Failure> {
    let mut meter = Meter::new(budget);
    let mut index = IndexStats::default();
    let (processed, units) = match preprocess(input, variables, library, &mut meter, &mut index) {
        Ok(result) => result,
        Err(Failure::Budget) => {
            return Ok(IndexedArm {
                arm: Arm {
                    outcome: Outcome::Unknown,
                    preprocessing: meter.work,
                    residual: Work::default(),
                    derived_units: Vec::new(),
                },
                index,
            })
        }
        Err(error) => return Err(error),
    };
    let remaining = budget
        .checked_sub(meter.work.work_units)
        .ok_or(Failure::CounterOverflow)?;
    let residual = transfer::solve(&processed, variables, None, remaining)?;
    Ok(IndexedArm {
        arm: Arm {
            outcome: residual.outcome,
            preprocessing: meter.work,
            residual: residual.residual,
            derived_units: units,
        },
        index,
    })
}

#[derive(Clone, Debug)]
pub struct Case {
    pub id: String,
    pub family: &'static str,
    pub variables: u32,
    pub input: Cnf,
    pub seed: Option<u64>,
    pub effective_seed: Option<u64>,
    pub density: Option<u32>,
    pub repetitions: Option<u32>,
    pub target_sign: Option<i32>,
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

fn generated(variables: u32, density: u32, effective_seed: u64, tag: u64) -> Cnf {
    let mut rng = Rng(effective_seed);
    let mut formula = Vec::new();
    for _ in 0..variables * density {
        let width = match tag {
            1 => 2,
            2 => 3,
            _ => {
                if rng.next() & 1 == 0 {
                    2
                } else {
                    3
                }
            }
        };
        let mut clause: Vec<i32> = Vec::new();
        while clause.len() < width {
            let variable = (rng.next() % u64::from(variables) + 1) as i32;
            if clause
                .iter()
                .any(|lit| lit.unsigned_abs() == variable as u32)
            {
                continue;
            }
            clause.push(if rng.next() & 1 == 0 {
                variable
            } else {
                -variable
            });
        }
        formula.push(clause);
    }
    formula
}

/// The fixed evaluation set. Do not evaluate or tune it before protocol freeze.
pub fn corpus() -> Result<Vec<Case>, Failure> {
    let mut cases = Vec::new();
    for seed in SEEDS {
        for variables in [6, 8, 10, 12] {
            for density in [2, 4, 6] {
                for (tag, family) in [(1, "random-2cnf"), (2, "random-3cnf"), (3, "random-mixed")] {
                    let effective_seed = seed
                        ^ (u64::from(variables) << 32)
                        ^ (u64::from(density) << 40)
                        ^ (tag << 48);
                    cases.push(Case {
                        id: format!("{family}-n{variables}-d{density}-s{seed}"),
                        family,
                        variables,
                        input: generated(variables, density, effective_seed, tag),
                        seed: Some(seed),
                        effective_seed: Some(effective_seed),
                        density: Some(density),
                        repetitions: None,
                        target_sign: None,
                    });
                }
            }
        }
    }
    for variables in [6, 8, 10, 12] {
        for sign in [1, -1] {
            for repetitions in [1, 8] {
                let shared = variables as i32 * sign;
                let mut input = Vec::new();
                for _ in 0..repetitions {
                    for pivot in 1..variables as i32 {
                        input.push(vec![shared, pivot]);
                        input.push(vec![shared, -pivot]);
                    }
                }
                let family = if sign > 0 {
                    "duplicate-star-positive"
                } else {
                    "duplicate-star-negative"
                };
                cases.push(Case {
                    id: format!("{family}-n{variables}-r{repetitions}"),
                    family,
                    variables,
                    input,
                    seed: None,
                    effective_seed: None,
                    density: None,
                    repetitions: Some(repetitions),
                    target_sign: Some(sign),
                });
            }
        }
    }
    let old = transfer::corpus();
    for (index, case) in cases.iter().enumerate() {
        if old
            .iter()
            .any(|previous| previous.variables == case.variables && previous.input == case.input)
            || cases[..index].iter().any(|previous| {
                previous.variables == case.variables && previous.input == case.input
            })
        {
            return Err(Failure::InvalidInput);
        }
    }
    Ok(cases)
}

pub struct Observation {
    pub case: Case,
    pub reference_sat: bool,
    pub reference_work: Work,
    pub baseline: Arm,
    pub generic: Arm,
    pub indexed: IndexedArm,
}

#[derive(Clone, Debug, Default)]
pub struct ArmSummary {
    pub preprocessing_work_units: u64,
    pub residual_work_units: u64,
    pub total_work_units: u64,
    pub search_nodes: u64,
    pub derived_units: u64,
    pub complete: u64,
    pub unknown: u64,
}

fn add(total: &mut u64, amount: u64) -> Result<(), Failure> {
    *total = total.checked_add(amount).ok_or(Failure::CounterOverflow)?;
    Ok(())
}

impl ArmSummary {
    fn include(&mut self, arm: &Arm) -> Result<(), Failure> {
        add(
            &mut self.preprocessing_work_units,
            arm.preprocessing.work_units,
        )?;
        add(&mut self.residual_work_units, arm.residual.work_units)?;
        add(&mut self.total_work_units, arm.total_work())?;
        add(&mut self.search_nodes, arm.residual.search_nodes)?;
        add(
            &mut self.derived_units,
            u64::try_from(arm.derived_units.len()).map_err(|_| Failure::CounterOverflow)?,
        )?;
        if arm.outcome == Outcome::Unknown {
            increment(&mut self.unknown)?;
        } else {
            increment(&mut self.complete)?;
        }
        Ok(())
    }
    fn json(&self) -> String {
        format!("{{\"preprocessing_work_units\":{},\"residual_work_units\":{},\"total_work_units\":{},\"search_nodes\":{},\"derived_units\":{},\"complete\":{},\"unknown\":{}}}",
            self.preprocessing_work_units, self.residual_work_units, self.total_work_units, self.search_nodes, self.derived_units, self.complete, self.unknown)
    }
}

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
    fn include(&mut self, left: &Arm, right: &Arm) -> Result<(), Failure> {
        match (
            left.outcome != Outcome::Unknown,
            right.outcome != Outcome::Unknown,
        ) {
            (true, true) => {
                increment(&mut self.both_complete)?;
                increment(match left.total_work().cmp(&right.total_work()) {
                    Ordering::Less => &mut self.left_better,
                    Ordering::Equal => &mut self.tied,
                    Ordering::Greater => &mut self.left_worse,
                })?;
            }
            (true, false) => increment(&mut self.left_complete_only)?,
            (false, true) => increment(&mut self.right_complete_only)?,
            (false, false) => increment(&mut self.both_unknown)?,
        }
        Ok(())
    }
    fn json(&self) -> String {
        format!("{{\"both_complete\":{},\"left_better\":{},\"tied\":{},\"left_worse\":{},\"left_complete_only\":{},\"right_complete_only\":{},\"both_unknown\":{}}}",
            self.both_complete, self.left_better, self.tied, self.left_worse, self.left_complete_only, self.right_complete_only, self.both_unknown)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub cases: u64,
    pub reference_work_units: u64,
    pub baseline: ArmSummary,
    pub generic: ArmSummary,
    pub indexed: ArmSummary,
    pub generic_vs_baseline: Comparison,
    pub indexed_vs_baseline: Comparison,
    pub indexed_vs_generic: Comparison,
}

impl Summary {
    fn include(&mut self, row: &Observation) -> Result<(), Failure> {
        increment(&mut self.cases)?;
        add(
            &mut self.reference_work_units,
            row.reference_work.work_units,
        )?;
        self.baseline.include(&row.baseline)?;
        self.generic.include(&row.generic)?;
        self.indexed.include(&row.indexed.arm)?;
        self.generic_vs_baseline
            .include(&row.generic, &row.baseline)?;
        self.indexed_vs_baseline
            .include(&row.indexed.arm, &row.baseline)?;
        self.indexed_vs_generic
            .include(&row.indexed.arm, &row.generic)?;
        Ok(())
    }
    fn json(&self) -> String {
        format!("{{\"cases\":{},\"reference_work_units\":{},\"arms\":{{\"baseline\":{},\"generic\":{},\"indexed\":{}}},\"comparisons\":{{\"generic_vs_baseline\":{},\"indexed_vs_baseline\":{},\"indexed_vs_generic\":{}}}}}",
            self.cases, self.reference_work_units, self.baseline.json(), self.generic.json(), self.indexed.json(),
            self.generic_vs_baseline.json(), self.indexed_vs_baseline.json(), self.indexed_vs_generic.json())
    }
}

pub struct Experiment {
    pub mining: transfer::Mining,
    pub compilation: Compilation,
    pub observations: Vec<Observation>,
    pub summary: Summary,
    pub families: std::collections::BTreeMap<&'static str, Summary>,
}

/// An arm exhausted inside preprocessing reports an unknown outcome, no
/// residual work, and no derived units. Every other signature means its
/// preprocessing phase completed, so its processed formula is comparable.
fn preprocessing_completed(arm: &Arm) -> bool {
    arm.outcome != Outcome::Unknown || arm.residual.work_units > 0 || !arm.derived_units.is_empty()
}

/// The protocol's equivalence label for one case: both processed formulas and
/// residual counters checked, only the formulas checked, or not comparable.
fn equivalence(generic: &Arm, indexed: &Arm) -> &'static str {
    if generic.outcome != Outcome::Unknown && indexed.outcome != Outcome::Unknown {
        "complete-arms-exact"
    } else if preprocessing_completed(generic) && preprocessing_completed(indexed) {
        "preprocessing-exact"
    } else {
        "not-comparable-budget"
    }
}

/// Run only after the protocol is committed. The library is immutable before
/// any held-out input is constructed or independently labeled.
pub fn experiment() -> Result<Experiment, Failure> {
    if PROTOCOL_SHA256.len() != 64 {
        return Err(Failure::InvalidInput);
    }
    let mining = transfer::mine()?;
    let compilation = compile(&mining.library, ARM_BUDGET)?;
    let cases = corpus()?;
    let mut observations = Vec::new();
    let mut summary = Summary::default();
    let mut families = std::collections::BTreeMap::<&'static str, Summary>::new();
    for case in cases {
        let (reference_sat, reference_work) = transfer::reference(&case.input, case.variables)?;
        let baseline = transfer::solve(&case.input, case.variables, None, ARM_BUDGET)?;
        let generic = transfer::solve(
            &case.input,
            case.variables,
            Some(&mining.library),
            ARM_BUDGET,
        )?;
        let indexed = solve(
            &case.input,
            case.variables,
            &compilation.library,
            ARM_BUDGET,
        )?;
        for arm in [&baseline, &generic, &indexed.arm] {
            if !arm.outcome.agrees(reference_sat) || arm.total_work() > ARM_BUDGET {
                return Err(Failure::InvalidInput);
            }
        }
        // Both processed formulas are the same metered copy of the input followed
        // by the derived units in order, so equal unit lists are equal formulas.
        match equivalence(&generic, &indexed.arm) {
            "complete-arms-exact"
                if generic.outcome != indexed.arm.outcome
                    || generic.derived_units != indexed.arm.derived_units
                    || generic.residual != indexed.arm.residual =>
            {
                return Err(Failure::InvalidInput);
            }
            "preprocessing-exact" if generic.derived_units != indexed.arm.derived_units => {
                return Err(Failure::InvalidInput);
            }
            _ => {}
        }
        let row = Observation {
            case,
            reference_sat,
            reference_work,
            baseline,
            generic,
            indexed,
        };
        summary.include(&row)?;
        families.entry(row.case.family).or_default().include(&row)?;
        observations.push(row);
    }
    Ok(Experiment {
        mining,
        compilation,
        observations,
        summary,
        families,
    })
}

fn quoted(value: &str) -> String {
    let mut result = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c < ' ' => {
                write!(result, "\\u{:04x}", c as u32).expect("string write");
            }
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

fn optional<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

/// Deterministic artifact serialization. Diagnostic/report construction is
/// outside all compared runtime meters.
pub fn json(result: &Experiment) -> String {
    let mut output = format!(concat!(
        "{{\n\"schema_version\":1,\"experiment\":\"indexed-transfer-v1\",\"status\":\"bounded-tested\",",
        "\"proof_status\":\"no-formal-proof\",\"protocol_sha256\":{},\"arm_budget\":{},\n",
        "\"counter_model\":\"transfer-v1-events-with-metered-array-index\",\n",
        "\"limitations\":[\"Finite nonrepresentative corpus; duplicate controls are engineered.\",",
        "\"Declared scalar events exclude allocator internals, loop-control arithmetic, and report construction; not elapsed time or a bit-cost proof.\",",
        "\"Peak index cells cover explicit entry, witness, and derived-unit arrays, not whole-process memory.\",",
        "\"Unknown arms are excluded from cost wins; total workload sums still include their spent budgets.\",",
        "\"The same known resolution schema is used; no new SAT completeness or P versus NP result.\"],\n",
        "\"frozen_library\":{{\"source_rule_instances\":{},\"compiled_schemas\":{},\"rules\":["),
        quoted(PROTOCOL_SHA256), ARM_BUDGET, result.compilation.library.source_rule_instances(), result.compilation.library.schemas());
    for (index, rule) in result.mining.library.rules().iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&rule.json());
    }
    write!(
        output,
        "]}},\n\"setup\":{{\"acquisition\":{},\"compilation\":{}}},\n",
        result.mining.work.json(),
        result.compilation.work.json()
    )
    .expect("string write");
    output.push_str("\"corpus\":{\"cases\":160,\"seeds\":[1201,5179,65539,104729],\"variables\":[6,8,10,12],\"densities\":[2,4,6],\"random_cases\":144,\"duplicate_controls\":16,\"exact_v1_overlap\":0},\n\"observations\":[\n");
    for (index, row) in result.observations.iter().enumerate() {
        if index > 0 {
            output.push_str(",\n");
        }
        let used = row
            .case
            .input
            .iter()
            .flatten()
            .map(|lit| lit.unsigned_abs())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let equivalence = equivalence(&row.generic, &row.indexed.arm);
        write!(output, concat!("{{\"id\":{},\"family\":{},\"variables\":{},\"used_variables\":{},\"seed\":{},",
            "\"effective_seed\":{},\"density\":{},\"repetitions\":{},\"target_sign\":{},\"input\":{:?},",
            "\"reference_sat\":{},\"reference_work\":{},\"baseline\":{},\"generic\":{},\"indexed\":{},\"generic_indexed_equivalence\":{}}}"),
            quoted(&row.case.id), quoted(row.case.family), row.case.variables, used, optional(row.case.seed),
            row.case.effective_seed.map_or_else(|| "null".into(), |seed| quoted(&seed.to_string())),
            optional(row.case.density), optional(row.case.repetitions), optional(row.case.target_sign), row.case.input,
            row.reference_sat, row.reference_work.json(), row.baseline.json(), row.generic.json(), row.indexed.json(), quoted(equivalence)).expect("string write");
    }
    write!(output, "\n],\n\"summary\":{},\n\"setup_plus_online\":{{\"generic_work_units\":{},\"indexed_work_units\":{}}},\n\"families\":{{",
        result.summary.json(), result.mining.work.work_units.checked_add(result.summary.generic.total_work_units).expect("bounded corpus sum"),
        result.mining.work.work_units.checked_add(result.compilation.work.work_units).and_then(|n| n.checked_add(result.summary.indexed.total_work_units)).expect("bounded corpus sum")).expect("string write");
    for (index, (family, summary)) in result.families.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write!(output, "{}:{}", quoted(family), summary.json()).expect("string write");
    }
    output.push_str("}\n}\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiler_rejects_unsupported_or_unsound_rule_shapes() {
        for rule in [
            Rule {
                premises: [[1, 2], [1, -2]],
                conclusion: 2,
            },
            Rule {
                premises: [[1, 2], [1, 2]],
                conclusion: 1,
            },
            Rule {
                premises: [[1, -1], [1, -2]],
                conclusion: 1,
            },
            Rule {
                premises: [[1, 0], [1, 0]],
                conclusion: 1,
            },
            Rule {
                premises: [[3, 4], [3, -4]],
                conclusion: 3,
            },
        ] {
            assert_eq!(
                check_rule(&rule, &mut Meter::new(1000)),
                Err(Failure::InvalidInput)
            );
        }
    }
}
