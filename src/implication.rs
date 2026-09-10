//! Known 2-SAT reasoning, with separately metered decision certificates and
//! complete backbone extraction. The truth-table reference is never an input
//! to graph construction or traversal. See the frozen experiment protocol.

use crate::transfer::{self, Event, Failure, FrozenLibrary, Meter, Outcome, Work};
use crate::Cnf;
use std::collections::BTreeMap;
use std::fmt::Write;

// SHA-256 of experiments/implication-protocol.md at its freeze commit 205f774.
pub const PROTOCOL_SHA256: &str =
    "5b957bb778acd98fa22cd40e8c4bd0f46c87ec516bea42b68736c70f44352ec2";
pub const ARM_BUDGET: u64 = 1_000_000;
pub const SEEDS: [u64; 4] = [2027, 8191, 131071, 999983];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub nodes: Vec<i32>,
    pub clauses: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Certificate {
    Sat(Vec<bool>),
    EmptyClause(usize),
    OppositePaths {
        variable: i32,
        positive_to_negative: Path,
        negative_to_positive: Path,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clue {
    pub literal: i32,
    pub path: Path,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackboneStatus {
    Complete,
    NotDefinedUnsat,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct GraphArm {
    pub decision: Outcome,
    pub certificate: Option<Certificate>,
    pub backbone_status: BackboneStatus,
    pub clues: Vec<Clue>,
    pub construction: Work,
    pub scc: Work,
    pub decision_certificate: Work,
    pub backbone: Work,
}

impl GraphArm {
    pub fn decision_work(&self) -> u64 {
        self.construction.work_units + self.scc.work_units + self.decision_certificate.work_units
    }
    pub fn total_work(&self) -> u64 {
        self.decision_work() + self.backbone.work_units
    }
}

// Phase snapshots are monotone components of the same checked meter.
pub(crate) fn difference(after: &Work, before: &Work) -> Work {
    Work {
        work_units: after.work_units - before.work_units,
        formula_checks: after.formula_checks - before.formula_checks,
        clause_reads: after.clause_reads - before.clause_reads,
        literal_reads: after.literal_reads - before.literal_reads,
        clause_writes: after.clause_writes - before.clause_writes,
        literal_writes: after.literal_writes - before.literal_writes,
        assignments: after.assignments - before.assignments,
        pair_checks: after.pair_checks - before.pair_checks,
        rule_attempts: after.rule_attempts - before.rule_attempts,
        search_nodes: after.search_nodes - before.search_nodes,
    }
}

pub(crate) fn ticks(m: &mut Meter, event: Event, count: usize) -> Result<(), Failure> {
    for _ in 0..count {
        m.tick(event)?;
    }
    Ok(())
}

pub(crate) fn node(lit: i32) -> usize {
    2 * (lit.unsigned_abs() as usize - 1) + usize::from(lit < 0)
}
pub(crate) fn literal(vertex: usize) -> i32 {
    let v = (vertex / 2 + 1) as i32;
    if vertex & 1 == 0 {
        v
    } else {
        -v
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Edge {
    pub(crate) to: usize,
    pub(crate) clause: usize,
}
pub(crate) struct Graph {
    pub(crate) forward: Vec<Vec<Edge>>,
    pub(crate) reverse: Vec<Vec<Edge>>,
    pub(crate) empty: Option<usize>,
}

/// Whole-formula validation with a declared width bound: the 2-CNF arms use
/// width two; the fragment arm admits width three and reads every clause.
fn validate_width(input: &Cnf, n: u32, max_width: usize, m: &mut Meter) -> Result<(), Failure> {
    m.tick(Event::Formula)?;
    transfer::validate(input, n, m)?;
    for clause in input {
        m.tick(Event::ClauseRead)?;
        if clause.len() > max_width {
            return Err(Failure::InvalidInput);
        }
    }
    Ok(())
}

fn validate_binary(input: &Cnf, n: u32, m: &mut Meter) -> Result<(), Failure> {
    validate_width(input, n, 2, m)
}

fn edge(
    graph: &mut Graph,
    from: usize,
    to: usize,
    clause: usize,
    m: &mut Meter,
) -> Result<(), Failure> {
    // Access two buckets, append two edge records, and store two fields each.
    ticks(m, Event::ClauseRead, 2)?;
    ticks(m, Event::ClauseWrite, 2)?;
    ticks(m, Event::LiteralWrite, 4)?;
    graph.forward[from].push(Edge { to, clause });
    graph.reverse[to].push(Edge { to: from, clause });
    Ok(())
}

fn build(input: &Cnf, n: u32, m: &mut Meter) -> Result<Graph, Failure> {
    validate_binary(input, n, m)?;
    construct(input, n, m)
}

/// The implication graph of the binary fragment of a width-at-most-three
/// formula. Every clause is read once; edges keep their original clause
/// indices; ternary clauses contribute no edge.
pub(crate) fn build_fragment(input: &Cnf, n: u32, m: &mut Meter) -> Result<Graph, Failure> {
    validate_width(input, n, 3, m)?;
    construct(input, n, m)
}

fn construct(input: &Cnf, n: u32, m: &mut Meter) -> Result<Graph, Failure> {
    ticks(m, Event::ClauseWrite, 2)?;
    let mut graph = Graph {
        forward: Vec::new(),
        reverse: Vec::new(),
        empty: None,
    };
    m.tick(Event::LiteralWrite)?;
    for _ in 0..2 * n {
        ticks(m, Event::ClauseWrite, 2)?;
        graph.forward.push(Vec::new());
        graph.reverse.push(Vec::new());
    }
    for (index, clause) in input.iter().enumerate() {
        m.tick(Event::ClauseRead)?;
        match clause.as_slice() {
            [] => {
                m.tick(Event::LiteralRead)?;
                if graph.empty.is_none() {
                    m.tick(Event::LiteralWrite)?;
                    graph.empty = Some(index);
                }
            }
            [a] => {
                m.tick(Event::LiteralRead)?;
                edge(&mut graph, node(-a), node(*a), index, m)?;
            }
            [a, b] => {
                ticks(m, Event::LiteralRead, 2)?;
                edge(&mut graph, node(-a), node(*b), index, m)?;
                edge(&mut graph, node(-b), node(*a), index, m)?;
            }
            [_, _, _] => (),
            _ => unreachable!("validated clause width"),
        }
    }
    Ok(graph)
}

pub(crate) fn array<T: Copy>(len: usize, value: T, m: &mut Meter) -> Result<Vec<T>, Failure> {
    m.tick(Event::ClauseWrite)?;
    ticks(m, Event::LiteralWrite, len)?;
    Ok(vec![value; len])
}

pub(crate) fn components(g: &Graph, m: &mut Meter) -> Result<Vec<usize>, Failure> {
    let size = g.forward.len();
    let mut seen = array(size, false, m)?;
    ticks(m, Event::ClauseWrite, 2)?;
    let mut finish = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for start in 0..size {
        m.tick(Event::LiteralRead)?;
        if seen[start] {
            continue;
        }
        m.tick(Event::LiteralWrite)?;
        seen[start] = true;
        ticks(m, Event::LiteralWrite, 2)?;
        stack.push((start, 0));
        while let Some((v, next)) = stack.last().copied() {
            ticks(m, Event::LiteralRead, 2)?;
            m.tick(Event::ClauseRead)?;
            if next == g.forward[v].len() {
                ticks(m, Event::LiteralRead, 2)?;
                stack.pop();
                m.tick(Event::LiteralWrite)?;
                finish.push(v);
                continue;
            }
            m.tick(Event::LiteralWrite)?;
            stack.last_mut().expect("nonempty").1 += 1;
            ticks(m, Event::LiteralRead, 2)?;
            let e = g.forward[v][next];
            m.tick(Event::LiteralRead)?;
            if !seen[e.to] {
                m.tick(Event::LiteralWrite)?;
                seen[e.to] = true;
                ticks(m, Event::LiteralWrite, 2)?;
                stack.push((e.to, 0));
            }
        }
    }
    let mut component = array(size, usize::MAX, m)?;
    m.tick(Event::ClauseWrite)?;
    let mut pending = Vec::new();
    let mut index = 0;
    for &start in finish.iter().rev() {
        ticks(m, Event::LiteralRead, 2)?;
        if component[start] != usize::MAX {
            continue;
        }
        m.tick(Event::LiteralWrite)?;
        component[start] = index;
        m.tick(Event::LiteralWrite)?;
        pending.push(start);
        while let Some(v) = pending.pop() {
            m.tick(Event::LiteralRead)?;
            m.tick(Event::Search)?;
            m.tick(Event::ClauseRead)?;
            // Reverse push order produces adjacency insertion order on pop.
            for e in g.reverse[v].iter().rev() {
                ticks(m, Event::LiteralRead, 2)?;
                m.tick(Event::LiteralRead)?;
                if component[e.to] == usize::MAX {
                    m.tick(Event::LiteralWrite)?;
                    component[e.to] = index;
                    m.tick(Event::LiteralWrite)?;
                    pending.push(e.to);
                }
            }
        }
        index += 1;
    }
    Ok(component)
}

fn path(g: &Graph, from: usize, to: usize, m: &mut Meter) -> Result<Option<Path>, Failure> {
    let size = g.forward.len();
    let mut predecessor = array(size, usize::MAX, m)?;
    let mut clause = array(size, usize::MAX, m)?;
    m.tick(Event::LiteralWrite)?;
    predecessor[from] = from;
    m.tick(Event::ClauseWrite)?;
    let mut stack = Vec::new();
    ticks(m, Event::LiteralWrite, 2)?;
    stack.push((from, 0));
    let mut found = from == to;
    while !found {
        let Some((v, next)) = stack.last().copied() else {
            break;
        };
        ticks(m, Event::LiteralRead, 2)?;
        m.tick(Event::ClauseRead)?;
        if next == g.forward[v].len() {
            ticks(m, Event::LiteralRead, 2)?;
            stack.pop();
            continue;
        }
        m.tick(Event::LiteralWrite)?;
        stack.last_mut().expect("nonempty").1 += 1;
        ticks(m, Event::LiteralRead, 2)?;
        let e = g.forward[v][next];
        m.tick(Event::LiteralRead)?;
        if predecessor[e.to] != usize::MAX {
            continue;
        }
        m.tick(Event::Search)?;
        ticks(m, Event::LiteralWrite, 2)?;
        predecessor[e.to] = v;
        clause[e.to] = e.clause;
        found = e.to == to;
        if !found {
            ticks(m, Event::LiteralWrite, 2)?;
            stack.push((e.to, 0));
        }
    }
    if !found {
        return Ok(None);
    }
    ticks(m, Event::ClauseWrite, 4)?;
    let mut reversed_nodes = Vec::new();
    let mut reversed_clauses = Vec::new();
    let mut v = to;
    loop {
        m.tick(Event::LiteralWrite)?;
        reversed_nodes.push(literal(v));
        if v == from {
            break;
        }
        ticks(m, Event::LiteralRead, 2)?;
        m.tick(Event::LiteralWrite)?;
        reversed_clauses.push(clause[v]);
        v = predecessor[v];
    }
    let mut nodes = Vec::new();
    let mut clauses = Vec::new();
    for &item in reversed_nodes.iter().rev() {
        m.tick(Event::LiteralRead)?;
        m.tick(Event::LiteralWrite)?;
        nodes.push(item);
    }
    for &item in reversed_clauses.iter().rev() {
        m.tick(Event::LiteralRead)?;
        m.tick(Event::LiteralWrite)?;
        clauses.push(item);
    }
    Ok(Some(Path { nodes, clauses }))
}

pub(crate) fn decision_certificate(
    g: &Graph,
    component: &[usize],
    n: u32,
    m: &mut Meter,
) -> Result<Certificate, Failure> {
    m.tick(Event::LiteralRead)?;
    if let Some(index) = g.empty {
        m.tick(Event::LiteralWrite)?;
        return Ok(Certificate::EmptyClause(index));
    }
    for v in 1..=n as i32 {
        ticks(m, Event::LiteralRead, 2)?;
        if component[node(v)] == component[node(-v)] {
            let positive_to_negative = path(g, node(v), node(-v), m)?.expect("same SCC has path");
            let negative_to_positive = path(g, node(-v), node(v), m)?.expect("same SCC has path");
            m.tick(Event::LiteralWrite)?;
            return Ok(Certificate::OppositePaths {
                variable: v,
                positive_to_negative,
                negative_to_positive,
            });
        }
    }
    m.tick(Event::ClauseWrite)?;
    let mut values = Vec::new();
    for v in 1..=n as i32 {
        ticks(m, Event::LiteralRead, 2)?;
        m.tick(Event::LiteralWrite)?;
        values.push(component[node(v)] > component[node(-v)]);
    }
    Ok(Certificate::Sat(values))
}

pub(crate) fn extract(g: &Graph, n: u32, m: &mut Meter) -> Result<Vec<Clue>, Failure> {
    m.tick(Event::ClauseWrite)?;
    let mut clues = Vec::new();
    for v in 1..=n as i32 {
        for l in [v, -v] {
            m.tick(Event::Rule)?;
            if let Some(path) = path(g, node(-l), node(l), m)? {
                m.tick(Event::ClauseWrite)?;
                m.tick(Event::LiteralWrite)?;
                clues.push(Clue { literal: l, path });
            }
        }
    }
    Ok(clues)
}

/// A single budget covers every graph phase. No oracle is used here.
pub fn analyze(input: &Cnf, n: u32, budget: u64) -> Result<GraphArm, Failure> {
    let mut meter = Meter::new(budget);
    let mut arm = GraphArm {
        decision: Outcome::Unknown,
        certificate: None,
        backbone_status: BackboneStatus::Unknown,
        clues: Vec::new(),
        construction: Work::default(),
        scc: Work::default(),
        decision_certificate: Work::default(),
        backbone: Work::default(),
    };
    let result = build(input, n, &mut meter);
    arm.construction = meter.work.clone();
    let g = match result {
        Ok(g) => g,
        Err(Failure::Budget) => return Ok(arm),
        Err(e) => return Err(e),
    };
    let before = meter.work.clone();
    let result = components(&g, &mut meter);
    arm.scc = difference(&meter.work, &before);
    let component = match result {
        Ok(c) => c,
        Err(Failure::Budget) => return Ok(arm),
        Err(e) => return Err(e),
    };
    let before = meter.work.clone();
    let result = decision_certificate(&g, &component, n, &mut meter);
    arm.decision_certificate = difference(&meter.work, &before);
    let certificate = match result {
        Ok(c) => c,
        Err(Failure::Budget) => return Ok(arm),
        Err(e) => return Err(e),
    };
    arm.decision = if matches!(certificate, Certificate::Sat(_)) {
        Outcome::Sat
    } else {
        Outcome::Unsat
    };
    arm.certificate = Some(certificate);
    if arm.decision == Outcome::Unsat {
        arm.backbone_status = BackboneStatus::NotDefinedUnsat;
        return Ok(arm);
    }
    let before = meter.work.clone();
    let result = extract(&g, n, &mut meter);
    arm.backbone = difference(&meter.work, &before);
    match result {
        Ok(clues) => {
            arm.clues = clues;
            arm.backbone_status = BackboneStatus::Complete;
        }
        Err(Failure::Budget) => (),
        Err(e) => return Err(e),
    }
    Ok(arm)
}

#[derive(Clone, Debug)]
pub struct Reference {
    pub model_count: u64,
    pub backbone: Option<Vec<i32>>,
    pub work: Work,
}

/// Full direct evaluation, independent of implication/SCC representation.
pub fn reference(input: &Cnf, n: u32) -> Result<Reference, Failure> {
    enumerate(input, n, 2)
}

/// The same direct evaluator admitting clauses of width at most three.
pub(crate) fn reference_wide(input: &Cnf, n: u32) -> Result<Reference, Failure> {
    enumerate(input, n, 3)
}

fn enumerate(input: &Cnf, n: u32, max_width: usize) -> Result<Reference, Failure> {
    let mut m = Meter::new(u64::MAX);
    validate_width(input, n, max_width, &mut m)?;
    let mut all_true = array(n as usize, true, &mut m)?;
    let mut all_false = array(n as usize, true, &mut m)?;
    let mut count = 0_u64;
    for assignment in 0..(1_u64 << n) {
        m.tick(Event::Assignment)?;
        m.tick(Event::Formula)?;
        let mut satisfied = true;
        for clause in input {
            m.tick(Event::ClauseRead)?;
            let mut clause_true = false;
            for &lit in clause {
                m.tick(Event::LiteralRead)?;
                if (((assignment >> (lit.unsigned_abs() - 1)) & 1) != 0) == (lit > 0) {
                    clause_true = true;
                    break;
                }
            }
            if !clause_true {
                satisfied = false;
                break;
            }
        }
        if satisfied {
            count = count.checked_add(1).ok_or(Failure::CounterOverflow)?;
            for v in 0..n as usize {
                ticks(&mut m, Event::LiteralRead, 2)?;
                ticks(&mut m, Event::LiteralWrite, 2)?;
                let value = (assignment >> v) & 1 != 0;
                all_true[v] &= value;
                all_false[v] &= !value;
            }
        }
    }
    let backbone = if count == 0 {
        None
    } else {
        m.tick(Event::ClauseWrite)?;
        let mut units = Vec::new();
        for v in 0..n as usize {
            ticks(&mut m, Event::LiteralRead, 2)?;
            if all_true[v] {
                m.tick(Event::LiteralWrite)?;
                units.push(v as i32 + 1);
            }
            if all_false[v] {
                m.tick(Event::LiteralWrite)?;
                units.push(-(v as i32 + 1));
            }
        }
        Some(units)
    };
    Ok(Reference {
        model_count: count,
        backbone,
        work: m.work,
    })
}

fn raw_path(
    input: &Cnf,
    n: u32,
    witness: &Path,
    from: i32,
    to: i32,
    m: &mut Meter,
) -> Result<bool, Failure> {
    m.tick(Event::ClauseRead)?;
    if witness.nodes.len() != witness.clauses.len() + 1
        || witness.nodes.first() != Some(&from)
        || witness.nodes.last() != Some(&to)
    {
        return Ok(false);
    }
    ticks(m, Event::LiteralRead, 2)?;
    for &lit in &witness.nodes {
        m.tick(Event::LiteralRead)?;
        if lit == 0 || lit == i32::MIN || lit.unsigned_abs() > n {
            return Ok(false);
        }
    }
    for (i, &index) in witness.clauses.iter().enumerate() {
        ticks(m, Event::LiteralRead, 3)?;
        let a = witness.nodes[i];
        let b = witness.nodes[i + 1];
        m.tick(Event::ClauseRead)?;
        let Some(clause) = input.get(index) else {
            return Ok(false);
        };
        let valid = match clause.as_slice() {
            [x] => {
                m.tick(Event::LiteralRead)?;
                a == -*x && b == *x
            }
            [x, y] => {
                ticks(m, Event::LiteralRead, 2)?;
                (a == -*x && b == *y) || (a == -*y && b == *x)
            }
            _ => false,
        };
        if !valid {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Check only raw input and supplied evidence. No graph, SCC or truth-table call.
pub fn check_certificate(
    input: &Cnf,
    n: u32,
    certificate: &Certificate,
) -> Result<(bool, Work), Failure> {
    certificate_check(input, n, certificate, 2)
}

/// The same raw checker on a width-at-most-three formula. A SAT certificate
/// is an assignment of the binary fragment, so only clauses of width at most
/// two are required to be satisfied; path certificates cite clauses by their
/// original index and a cited ternary clause is rejected.
pub(crate) fn check_fragment_certificate(
    input: &Cnf,
    n: u32,
    certificate: &Certificate,
) -> Result<(bool, Work), Failure> {
    certificate_check(input, n, certificate, 3)
}

fn certificate_check(
    input: &Cnf,
    n: u32,
    certificate: &Certificate,
    max_width: usize,
) -> Result<(bool, Work), Failure> {
    let mut m = Meter::new(u64::MAX);
    validate_width(input, n, max_width, &mut m)?;
    let valid = match certificate {
        Certificate::Sat(values) => {
            if values.len() != n as usize {
                false
            } else {
                let mut valid = true;
                m.tick(Event::Formula)?;
                for clause in input {
                    m.tick(Event::ClauseRead)?;
                    if clause.len() > 2 {
                        continue;
                    }
                    let mut sat = false;
                    for &lit in clause {
                        ticks(&mut m, Event::LiteralRead, 2)?;
                        if values[(lit.unsigned_abs() - 1) as usize] == (lit > 0) {
                            sat = true;
                            break;
                        }
                    }
                    if !sat {
                        valid = false;
                        break;
                    }
                }
                valid
            }
        }
        Certificate::EmptyClause(index) => {
            m.tick(Event::ClauseRead)?;
            input.get(*index).is_some_and(Vec::is_empty)
        }
        Certificate::OppositePaths {
            variable,
            positive_to_negative,
            negative_to_positive,
        } => {
            *variable > 0
                && variable.unsigned_abs() <= n
                && raw_path(
                    input,
                    n,
                    positive_to_negative,
                    *variable,
                    -*variable,
                    &mut m,
                )?
                && raw_path(
                    input,
                    n,
                    negative_to_positive,
                    -*variable,
                    *variable,
                    &mut m,
                )?
        }
    };
    Ok((valid, m.work))
}

pub fn check_clue(input: &Cnf, n: u32, clue: &Clue) -> Result<(bool, Work), Failure> {
    clue_check(input, n, clue, 2)
}

/// The raw clue checker on a width-at-most-three formula; the cited clauses
/// must be units or binary clauses of the input at their original indices.
pub(crate) fn check_fragment_clue(
    input: &Cnf,
    n: u32,
    clue: &Clue,
) -> Result<(bool, Work), Failure> {
    clue_check(input, n, clue, 3)
}

fn clue_check(input: &Cnf, n: u32, clue: &Clue, max_width: usize) -> Result<(bool, Work), Failure> {
    let mut m = Meter::new(u64::MAX);
    validate_width(input, n, max_width, &mut m)?;
    if clue.literal == 0 || clue.literal == i32::MIN || clue.literal.unsigned_abs() > n {
        return Ok((false, m.work));
    }
    let valid = raw_path(input, n, &clue.path, -clue.literal, clue.literal, &mut m)?;
    Ok((valid, m.work))
}

#[derive(Clone, Debug)]
pub struct LocalArm {
    pub complete: bool,
    pub units: Vec<i32>,
    pub work: Work,
}
pub fn local(
    input: &Cnf,
    n: u32,
    library: &FrozenLibrary,
    budget: u64,
) -> Result<LocalArm, Failure> {
    let mut m = Meter::new(budget);
    // The actual existing preprocessor owns validation/copying and its meter.
    match transfer::preprocess(input, n, library, &mut m) {
        Ok((_, units)) => Ok(LocalArm {
            complete: true,
            units,
            work: m.work,
        }),
        Err(Failure::Budget) => Ok(LocalArm {
            complete: false,
            units: Vec::new(),
            work: m.work,
        }),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Fixed 120-case corpus, enumerated in protocol order.

pub const CHAIN_LENGTHS: [u32; 6] = [2, 3, 5, 7, 9, 11];
pub const IMMEDIATE_SIZES: [u32; 6] = [2, 4, 6, 8, 10, 12];
pub const PARITY_SIZES: std::ops::RangeInclusive<u32> = 3..=12;
pub const RANDOM_VARIABLES: [u32; 4] = [6, 8, 10, 12];
pub const DENSITIES: [u32; 3] = [1, 2, 4];
pub const CORPUS_CASES: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Forward,
    Reversed,
}

impl Order {
    fn name(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Reversed => "reversed",
        }
    }
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
    pub chain_length: Option<u32>,
    pub sign: Option<i32>,
    pub order: Option<Order>,
}

fn sign_name(sign: i32) -> &'static str {
    if sign > 0 {
        "pos"
    } else {
        "neg"
    }
}

/// `F_k` over x=1 and y_i=i+1; the broken variant drops link i=floor(k/2).
pub(crate) fn chain(k: u32, broken: bool) -> Cnf {
    let y = |i: u32| (i + 1) as i32;
    let mut clauses = vec![vec![1, y(1)]];
    for i in 1..k {
        if broken && i == k / 2 {
            continue;
        }
        clauses.push(vec![-y(i), y(i + 1)]);
    }
    clauses.push(vec![-y(k), 1]);
    clauses
}

/// Negate every occurrence of x for the negative sign; keep literal order in
/// each clause; reverse the clause list for the reversed order.
pub(crate) fn transformed(clauses: &Cnf, sign: i32, order: Order) -> Cnf {
    let mut result: Cnf = clauses
        .iter()
        .map(|clause| {
            clause
                .iter()
                .map(|&lit| {
                    if lit.unsigned_abs() == 1 {
                        lit * sign
                    } else {
                        lit
                    }
                })
                .collect()
        })
        .collect();
    if order == Order::Reversed {
        result.reverse();
    }
    result
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

pub fn effective_seed(base_seed: u64, variables: u32, density: u32) -> u64 {
    base_seed ^ (u64::from(variables) << 32) ^ (u64::from(density) << 16)
}

/// Unconditioned pseudorandom 2-CNF with the protocol's exact draw procedure.
fn random(variables: u32, density: u32, seed: u64) -> Cnf {
    let mut rng = Rng(seed);
    let mut formula = Vec::new();
    for _ in 0..variables * density {
        let first = (rng.next() % u64::from(variables) + 1) as i32;
        let first = if rng.next() & 1 == 0 { first } else { -first };
        let second = loop {
            let candidate = (rng.next() % u64::from(variables) + 1) as i32;
            if candidate != first.abs() {
                break candidate;
            }
        };
        let second = if rng.next() & 1 == 0 { second } else { -second };
        formula.push(vec![first, second]);
    }
    formula
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
        chain_length: None,
        sign: None,
        order: None,
    }
}

/// The fixed evaluation set in protocol enumeration order.
pub fn corpus() -> Vec<Case> {
    let mut cases = Vec::new();
    for (family, broken) in [("long-explanation", false), ("broken-explanation", true)] {
        for k in CHAIN_LENGTHS {
            for sign in [1, -1] {
                for order in [Order::Forward, Order::Reversed] {
                    let mut case = plain(
                        format!("{family}-k{k}-{}-{}", sign_name(sign), order.name()),
                        family,
                        k + 1,
                        transformed(&chain(k, broken), sign, order),
                    );
                    case.chain_length = Some(k);
                    case.sign = Some(sign);
                    case.order = Some(order);
                    cases.push(case);
                }
            }
        }
    }
    for n in IMMEDIATE_SIZES {
        for sign in [1, -1] {
            let input = (1..=n / 2)
                .flat_map(|i| {
                    let odd = sign * (2 * i as i32 - 1);
                    let even = 2 * i as i32;
                    [vec![odd, even], vec![odd, -even]]
                })
                .collect();
            let mut case = plain(
                format!("immediate-pattern-n{n}-{}", sign_name(sign)),
                "immediate-pattern",
                n,
                input,
            );
            case.sign = Some(sign);
            cases.push(case);
        }
    }
    for n in PARITY_SIZES {
        let input = (1..=n)
            .flat_map(|i| {
                let a = i as i32;
                let b = (i % n + 1) as i32;
                [vec![a, b], vec![-a, -b]]
            })
            .collect();
        cases.push(plain(
            format!("parity-cycle-n{n}"),
            "parity-cycle",
            n,
            input,
        ));
    }
    cases.push(plain(
        "empty-formula".into(),
        "empty-control",
        0,
        Vec::new(),
    ));
    cases.push(plain(
        "empty-clause".into(),
        "empty-control",
        0,
        vec![Vec::new()],
    ));
    for seed in SEEDS {
        for n in RANDOM_VARIABLES {
            for d in DENSITIES {
                let effective = effective_seed(seed, n, d);
                let mut case = plain(
                    format!("random-n{n}-d{d}-s{seed}"),
                    "random-2cnf",
                    n,
                    random(n, d, effective),
                );
                case.seed = Some(seed);
                case.effective_seed = Some(effective);
                case.density = Some(d);
                cases.push(case);
            }
        }
    }
    cases
}

// ---------------------------------------------------------------------------
// Experiment: reference, three arms, raw checkers, acceptance, summaries.

pub(crate) fn add(total: &mut u64, amount: u64) -> Result<(), Failure> {
    *total = total.checked_add(amount).ok_or(Failure::CounterOverflow)?;
    Ok(())
}

pub(crate) fn increment(total: &mut u64) -> Result<(), Failure> {
    add(total, 1)
}

pub(crate) fn count(len: usize) -> Result<u64, Failure> {
    u64::try_from(len).map_err(|_| Failure::CounterOverflow)
}

pub(crate) fn accumulate(total: &mut Work, part: &Work) -> Result<(), Failure> {
    add(&mut total.work_units, part.work_units)?;
    add(&mut total.formula_checks, part.formula_checks)?;
    add(&mut total.clause_reads, part.clause_reads)?;
    add(&mut total.literal_reads, part.literal_reads)?;
    add(&mut total.clause_writes, part.clause_writes)?;
    add(&mut total.literal_writes, part.literal_writes)?;
    add(&mut total.assignments, part.assignments)?;
    add(&mut total.pair_checks, part.pair_checks)?;
    add(&mut total.rule_attempts, part.rule_attempts)?;
    add(&mut total.search_nodes, part.search_nodes)?;
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct Checks {
    pub certificate_valid: Option<bool>,
    pub clues_checked: u64,
    pub clues_valid: u64,
    pub work: Work,
}

pub struct Observation {
    pub case: Case,
    pub reference: Reference,
    pub local: LocalArm,
    pub dpll: transfer::Arm,
    pub graph: GraphArm,
    pub checks: Checks,
}

fn sorted(literals: &[i32]) -> Vec<i32> {
    let mut result = literals.to_vec();
    result.sort_unstable();
    result
}

fn clue_literals(graph: &GraphArm) -> Vec<i32> {
    graph.clues.iter().map(|clue| clue.literal).collect()
}

enum Expectation {
    Sat(Vec<i32>),
    Unsat,
    None,
}

/// Stated control properties of the fixed families; random cases have none.
fn expectation(case: &Case) -> Result<Expectation, Failure> {
    Ok(match case.family {
        "long-explanation" => Expectation::Sat(vec![case.sign.ok_or(Failure::InvalidInput)?]),
        "broken-explanation" => Expectation::Sat(Vec::new()),
        "immediate-pattern" => {
            let sign = case.sign.ok_or(Failure::InvalidInput)?;
            Expectation::Sat(
                (1..=case.variables / 2)
                    .map(|i| sign * (2 * i as i32 - 1))
                    .collect(),
            )
        }
        "parity-cycle" => {
            if case.variables.is_multiple_of(2) {
                Expectation::Sat(Vec::new())
            } else {
                Expectation::Unsat
            }
        }
        "empty-control" => {
            if case.input.is_empty() {
                Expectation::Sat(Vec::new())
            } else {
                Expectation::Unsat
            }
        }
        _ => Expectation::None,
    })
}

/// Every protocol acceptance check. A violation invalidates the calibration.
fn accept(row: &Observation) -> Result<(), Failure> {
    let label = row.reference.model_count > 0;
    if label != row.reference.backbone.is_some() {
        return Err(Failure::InvalidInput);
    }
    if !row.dpll.outcome.agrees(label) || !row.graph.decision.agrees(label) {
        return Err(Failure::InvalidInput);
    }
    if row.dpll.total_work() > ARM_BUDGET
        || row.local.work.work_units > ARM_BUDGET
        || row.graph.total_work() > ARM_BUDGET
    {
        return Err(Failure::InvalidInput);
    }
    let graph = &row.graph;
    match graph.decision {
        Outcome::Unknown => {
            if graph.certificate.is_some()
                || graph.backbone_status != BackboneStatus::Unknown
                || !graph.clues.is_empty()
            {
                return Err(Failure::InvalidInput);
            }
        }
        Outcome::Unsat => {
            if !matches!(
                graph.certificate,
                Some(Certificate::EmptyClause(_) | Certificate::OppositePaths { .. })
            ) || graph.backbone_status != BackboneStatus::NotDefinedUnsat
                || !graph.clues.is_empty()
            {
                return Err(Failure::InvalidInput);
            }
        }
        Outcome::Sat => {
            if !matches!(graph.certificate, Some(Certificate::Sat(_)))
                || graph.backbone_status == BackboneStatus::NotDefinedUnsat
                || (graph.backbone_status == BackboneStatus::Unknown && !graph.clues.is_empty())
            {
                return Err(Failure::InvalidInput);
            }
        }
    }
    if graph.certificate.is_some() != row.checks.certificate_valid.is_some()
        || row.checks.certificate_valid == Some(false)
        || row.checks.clues_checked != count(graph.clues.len())?
        || row.checks.clues_valid != row.checks.clues_checked
    {
        return Err(Failure::InvalidInput);
    }
    if let Some(backbone) = &row.reference.backbone {
        if graph.backbone_status == BackboneStatus::Complete && clue_literals(graph) != *backbone {
            return Err(Failure::InvalidInput);
        }
        if row.local.complete {
            let known = sorted(backbone);
            let mut seen = Vec::new();
            for &unit in &row.local.units {
                if known.binary_search(&unit).is_err() || seen.contains(&unit) {
                    return Err(Failure::InvalidInput);
                }
                seen.push(unit);
            }
        }
    }
    match expectation(&row.case)? {
        Expectation::Sat(expected) => {
            if row.reference.backbone.as_ref() != Some(&expected) {
                return Err(Failure::InvalidInput);
            }
        }
        Expectation::Unsat => {
            if label {
                return Err(Failure::InvalidInput);
            }
        }
        Expectation::None => (),
    }
    if row.local.complete {
        match row.case.family {
            // Two-clause subsets of F_k cannot entail a unit.
            "long-explanation" if !row.local.units.is_empty() => return Err(Failure::InvalidInput),
            // Training-overlap controls must validate reuse of the library.
            "immediate-pattern"
                if row.reference.backbone.as_ref().map(|b| sorted(b))
                    != Some(sorted(&row.local.units)) =>
            {
                return Err(Failure::InvalidInput)
            }
            _ => (),
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct LocalSummary {
    pub work_units: u64,
    pub complete: u64,
    pub unknown: u64,
    pub derived_units: u64,
}

impl LocalSummary {
    fn include(&mut self, arm: &LocalArm) -> Result<(), Failure> {
        add(&mut self.work_units, arm.work.work_units)?;
        increment(if arm.complete {
            &mut self.complete
        } else {
            &mut self.unknown
        })?;
        add(&mut self.derived_units, count(arm.units.len())?)
    }
    fn json(&self) -> String {
        format!(
            "{{\"work_units\":{},\"complete\":{},\"unknown\":{},\"derived_units\":{}}}",
            self.work_units, self.complete, self.unknown, self.derived_units
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct DpllSummary {
    pub work_units: u64,
    pub search_nodes: u64,
    pub sat: u64,
    pub unsat: u64,
    pub unknown: u64,
}

impl DpllSummary {
    fn include(&mut self, arm: &transfer::Arm) -> Result<(), Failure> {
        add(&mut self.work_units, arm.total_work())?;
        add(&mut self.search_nodes, arm.residual.search_nodes)?;
        increment(match arm.outcome {
            Outcome::Sat => &mut self.sat,
            Outcome::Unsat => &mut self.unsat,
            Outcome::Unknown => &mut self.unknown,
        })
    }
    fn json(&self) -> String {
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
pub struct GraphSummary {
    pub construction_work_units: u64,
    pub scc_work_units: u64,
    pub decision_certificate_work_units: u64,
    pub decision_work_units: u64,
    pub decision_search_nodes: u64,
    pub backbone_work_units: u64,
    pub backbone_search_nodes: u64,
    pub total_work_units: u64,
    pub sat: u64,
    pub unsat: u64,
    pub decision_unknown: u64,
    pub backbone_complete: u64,
    pub backbone_not_defined_unsat: u64,
    pub backbone_unknown: u64,
    pub clues: u64,
}

impl GraphSummary {
    fn include(&mut self, arm: &GraphArm) -> Result<(), Failure> {
        add(
            &mut self.construction_work_units,
            arm.construction.work_units,
        )?;
        add(&mut self.scc_work_units, arm.scc.work_units)?;
        add(
            &mut self.decision_certificate_work_units,
            arm.decision_certificate.work_units,
        )?;
        add(&mut self.decision_work_units, arm.decision_work())?;
        for phase in [&arm.construction, &arm.scc, &arm.decision_certificate] {
            add(&mut self.decision_search_nodes, phase.search_nodes)?;
        }
        add(&mut self.backbone_work_units, arm.backbone.work_units)?;
        add(&mut self.backbone_search_nodes, arm.backbone.search_nodes)?;
        add(&mut self.total_work_units, arm.total_work())?;
        increment(match arm.decision {
            Outcome::Sat => &mut self.sat,
            Outcome::Unsat => &mut self.unsat,
            Outcome::Unknown => &mut self.decision_unknown,
        })?;
        increment(match arm.backbone_status {
            BackboneStatus::Complete => &mut self.backbone_complete,
            BackboneStatus::NotDefinedUnsat => &mut self.backbone_not_defined_unsat,
            BackboneStatus::Unknown => &mut self.backbone_unknown,
        })?;
        add(&mut self.clues, count(arm.clues.len())?)
    }
    fn json(&self) -> String {
        format!(
            concat!(
                "{{\"construction_work_units\":{},\"scc_work_units\":{},\"decision_certificate_work_units\":{},",
                "\"decision_work_units\":{},\"decision_search_nodes\":{},\"backbone_work_units\":{},",
                "\"backbone_search_nodes\":{},\"total_work_units\":{},\"decision_complete\":{},\"sat\":{},",
                "\"unsat\":{},\"decision_unknown\":{},\"backbone_complete\":{},\"backbone_not_defined_unsat\":{},",
                "\"backbone_unknown\":{},\"clues\":{}}}"
            ),
            self.construction_work_units,
            self.scc_work_units,
            self.decision_certificate_work_units,
            self.decision_work_units,
            self.decision_search_nodes,
            self.backbone_work_units,
            self.backbone_search_nodes,
            self.total_work_units,
            self.sat + self.unsat,
            self.sat,
            self.unsat,
            self.decision_unknown,
            self.backbone_complete,
            self.backbone_not_defined_unsat,
            self.backbone_unknown,
            self.clues
        )
    }
}

/// Literal coverage on completed SAT cases only; UNSAT cases have no backbone.
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    pub cases: u64,
    pub reference_literals: u64,
    pub found_literals: u64,
    pub missed_literals: u64,
    pub full_coverage_cases: u64,
}

impl Coverage {
    fn include(&mut self, reference: &[i32], found: &[i32]) -> Result<(), Failure> {
        let (reference, found) = (count(reference.len())?, count(found.len())?);
        increment(&mut self.cases)?;
        add(&mut self.reference_literals, reference)?;
        add(&mut self.found_literals, found)?;
        add(
            &mut self.missed_literals,
            reference.checked_sub(found).ok_or(Failure::InvalidInput)?,
        )?;
        if reference == found {
            increment(&mut self.full_coverage_cases)?;
        }
        Ok(())
    }
    fn json(&self) -> String {
        format!(
            "{{\"cases\":{},\"reference_literals\":{},\"found_literals\":{},\"missed_literals\":{},\"full_coverage_cases\":{}}}",
            self.cases, self.reference_literals, self.found_literals, self.missed_literals, self.full_coverage_cases
        )
    }
}

/// Decision cost, graph (with its explicit certificate) on the left.
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
    fn include(&mut self, left: Option<u64>, right: Option<u64>) -> Result<(), Failure> {
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
    fn json(&self) -> String {
        format!(
            "{{\"both_complete\":{},\"left_better\":{},\"tied\":{},\"left_worse\":{},\"left_complete_only\":{},\"right_complete_only\":{},\"both_unknown\":{}}}",
            self.both_complete, self.left_better, self.tied, self.left_worse, self.left_complete_only, self.right_complete_only, self.both_unknown
        )
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
    pub local: LocalSummary,
    pub dpll: DpllSummary,
    pub graph: GraphSummary,
    pub local_coverage: Coverage,
    pub graph_coverage: Coverage,
    pub graph_decision_vs_dpll: Comparison,
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
        self.local.include(&row.local)?;
        self.dpll.include(&row.dpll)?;
        self.graph.include(&row.graph)?;
        if let Some(backbone) = &row.reference.backbone {
            if row.local.complete {
                self.local_coverage.include(backbone, &row.local.units)?;
            }
            if row.graph.backbone_status == BackboneStatus::Complete {
                self.graph_coverage
                    .include(backbone, &clue_literals(&row.graph))?;
            }
        }
        // Compared only when both decisions completed; acceptance already
        // established that every completed decision agrees with the reference.
        let graph = (row.graph.decision != Outcome::Unknown).then(|| row.graph.decision_work());
        let dpll = (row.dpll.outcome != Outcome::Unknown).then(|| row.dpll.total_work());
        self.graph_decision_vs_dpll.include(graph, dpll)
    }
    fn json(&self) -> String {
        format!(
            concat!(
                "{{\"cases\":{},\"reference_sat\":{},\"reference_unsat\":{},\"reference_work_units\":{},",
                "\"checker_work_units\":{},\"certificates_checked\":{},\"clues_checked\":{},",
                "\"arms\":{{\"local\":{},\"dpll\":{},\"graph\":{}}},",
                "\"coverage\":{{\"local\":{},\"graph\":{}}},",
                "\"comparisons\":{{\"graph_decision_vs_dpll\":{}}}}}"
            ),
            self.cases,
            self.reference_sat,
            self.reference_unsat,
            self.reference_work_units,
            self.checker_work_units,
            self.certificates_checked,
            self.clues_checked,
            self.local.json(),
            self.dpll.json(),
            self.graph.json(),
            self.local_coverage.json(),
            self.graph_coverage.json(),
            self.graph_decision_vs_dpll.json()
        )
    }
}

pub struct Experiment {
    pub mining: transfer::Mining,
    pub observations: Vec<Observation>,
    pub summary: Summary,
    pub families: BTreeMap<&'static str, Summary>,
}

/// Raw checkers on every completed certificate and every reported clue.
fn check(case: &Case, graph: &GraphArm) -> Result<Checks, Failure> {
    let mut checks = Checks::default();
    if let Some(certificate) = &graph.certificate {
        let (valid, work) = check_certificate(&case.input, case.variables, certificate)?;
        checks.certificate_valid = Some(valid);
        accumulate(&mut checks.work, &work)?;
    }
    for clue in &graph.clues {
        let (valid, work) = check_clue(&case.input, case.variables, clue)?;
        increment(&mut checks.clues_checked)?;
        add(&mut checks.clues_valid, u64::from(valid))?;
        accumulate(&mut checks.work, &work)?;
    }
    Ok(checks)
}

/// Run only against the frozen protocol. The library is mined before the
/// corpus exists; no label or reference backbone reaches any arm.
pub fn experiment() -> Result<Experiment, Failure> {
    if PROTOCOL_SHA256.len() != 64 {
        return Err(Failure::InvalidInput);
    }
    let mining = transfer::mine()?;
    if mining.library.rules().len() != 4 {
        return Err(Failure::InvalidInput);
    }
    let cases = corpus();
    if cases.len() != CORPUS_CASES {
        return Err(Failure::InvalidInput);
    }
    let mut observations = Vec::new();
    let mut summary = Summary::default();
    let mut families = BTreeMap::<&'static str, Summary>::new();
    for case in cases {
        let reference = reference(&case.input, case.variables)?;
        let local = local(&case.input, case.variables, &mining.library, ARM_BUDGET)?;
        let dpll = transfer::solve(&case.input, case.variables, None, ARM_BUDGET)?;
        let graph = analyze(&case.input, case.variables, ARM_BUDGET)?;
        let checks = check(&case, &graph)?;
        let row = Observation {
            case,
            reference,
            local,
            dpll,
            graph,
            checks,
        };
        accept(&row)?;
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

// ---------------------------------------------------------------------------
// Deterministic reporting. Formatting is outside every meter.

pub(crate) fn quoted(value: &str) -> String {
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

pub(crate) fn optional<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

pub(crate) fn outcome_name(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Sat => "sat",
        Outcome::Unsat => "unsat",
        Outcome::Unknown => "unknown-budget",
    }
}

impl BackboneStatus {
    fn name(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::NotDefinedUnsat => "not-defined-unsat",
            Self::Unknown => "unknown-budget",
        }
    }
}

impl Path {
    pub(crate) fn json(&self) -> String {
        format!(
            "{{\"nodes\":{:?},\"clauses\":{:?}}}",
            self.nodes, self.clauses
        )
    }
}

impl Certificate {
    pub(crate) fn json(&self) -> String {
        match self {
            Self::Sat(values) => format!("{{\"kind\":\"sat\",\"values\":{values:?}}}"),
            Self::EmptyClause(index) => {
                format!("{{\"kind\":\"empty-clause\",\"clause\":{index}}}")
            }
            Self::OppositePaths {
                variable,
                positive_to_negative,
                negative_to_positive,
            } => format!(
                "{{\"kind\":\"opposite-paths\",\"variable\":{},\"positive_to_negative\":{},\"negative_to_positive\":{}}}",
                variable,
                positive_to_negative.json(),
                negative_to_positive.json()
            ),
        }
    }
}

impl GraphArm {
    fn json(&self) -> String {
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
                "{{\"decision\":{},\"certificate\":{},\"backbone_status\":{},\"clues\":{},",
                "\"phases\":{{\"construction\":{},\"scc\":{},\"decision_certificate\":{},\"backbone\":{}}},",
                "\"decision_work_units\":{},\"backbone_work_units\":{},\"total_work_units\":{}}}"
            ),
            quoted(outcome_name(&self.decision)),
            self.certificate
                .as_ref()
                .map_or_else(|| "null".into(), Certificate::json),
            quoted(self.backbone_status.name()),
            clues,
            self.construction.json(),
            self.scc.json(),
            self.decision_certificate.json(),
            self.backbone.json(),
            self.decision_work(),
            self.backbone.work_units,
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
    let backbone = row
        .reference
        .backbone
        .as_ref()
        .map_or_else(|| "null".into(), |backbone| format!("{backbone:?}"));
    format!(
        concat!(
            "{{\"id\":{},\"family\":{},\"variables\":{},\"used_variables\":{},\"seed\":{},\"effective_seed\":{},",
            "\"density\":{},\"chain_length\":{},\"sign\":{},\"order\":{},\"input\":{:?},\n",
            " \"reference\":{{\"model_count\":{},\"sat\":{},\"backbone\":{},\"work\":{}}},\n",
            " \"local\":{{\"complete\":{},\"units\":{:?},\"work\":{}}},\n",
            " \"dpll\":{{\"outcome\":{},\"work\":{},\"total_work_units\":{},\"search_nodes\":{}}},\n",
            " \"graph\":{},\n",
            " \"checks\":{{\"certificate_valid\":{},\"clues_checked\":{},\"clues_valid\":{},\"work\":{}}}}}"
        ),
        quoted(&row.case.id),
        quoted(row.case.family),
        row.case.variables,
        used,
        optional(row.case.seed),
        optional(row.case.effective_seed),
        optional(row.case.density),
        optional(row.case.chain_length),
        optional(row.case.sign),
        row.case
            .order
            .map_or_else(|| "null".into(), |order| quoted(order.name())),
        row.case.input,
        row.reference.model_count,
        row.reference.model_count > 0,
        backbone,
        row.reference.work.json(),
        row.local.complete,
        row.local.units,
        row.local.work.json(),
        quoted(outcome_name(&row.dpll.outcome)),
        row.dpll.residual.json(),
        row.dpll.total_work(),
        row.dpll.residual.search_nodes,
        row.graph.json(),
        optional(row.checks.certificate_valid),
        row.checks.clues_checked,
        row.checks.clues_valid,
        row.checks.work.json()
    )
}

const LIMITATIONS: [&str; 10] = [
    "Calibration of known 2-SAT reasoning (Aspvall-Plass-Tarjan SCC decision and path-based forced literals); no evolutionary advantage, novel inference rule, or general SAT complexity result.",
    "Finite fixed corpus of at most 12 declared variables; pseudorandom cases are reproducible samples, not independent or representative population estimates, and all sizes are calibration cases, not scaling evidence.",
    "Immediate-pattern controls overlap the training grammar and deliberately favor the local library; they are mechanism checks only.",
    "The frozen-library root preprocessor is incomplete by design: a missing unit is not evidence that the literal is unforced, and its output set is not equivalent to complete backbone extraction.",
    "Declared event counters exclude allocator internals, loop control, arithmetic and comparison instructions, input generation, JSON formatting, and report construction; they are operational measurements, not elapsed time or a proved bit-cost bound.",
    "Measured totals do not prove the O(n+m) decision or O(n(n+m)) extraction asymptotic statements; label widths and encoding costs are explained separately.",
    "Reference truth-table, one-time library acquisition, and raw-certificate checker work are outside the compared arms and reported separately.",
    "Budget exhaustion means unknown: no exhausted arm is reported as UNSAT and no incomplete clue set is reported as complete.",
    "UNSAT cases have no defined solution-set backbone; no backbone or coverage claim is made for them.",
    "No required result is a speedup; the graph decision cost includes explicit certificate construction while DPLL returns only a Boolean.",
];

pub fn json(result: &Experiment) -> String {
    let mut output = format!(
        concat!(
            "{{\n\"schema_version\":1,\"experiment\":\"implication-calibration-v1\",\"status\":\"bounded-tested\",",
            "\"proof_status\":\"no-formal-proof\",\"protocol_sha256\":{},\"arm_budget\":{},\n",
            "\"counter_model\":\"transfer-v1-events-with-graph-phases\",\n\"limitations\":["
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
        "],\n\"frozen_library\":{{\"source_rule_instances\":{},\"training_pairs\":6,\"frozen_before_heldout\":true,\"rules\":[",
        result.mining.library.rules().len()
    )
    .expect("string write");
    for (index, rule) in result.mining.library.rules().iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&rule.json());
    }
    write!(
        output,
        "]}},\n\"setup\":{{\"acquisition\":{}}},\n",
        result.mining.work.json()
    )
    .expect("string write");
    write!(
        output,
        concat!(
            "\"corpus\":{{\"cases\":{},\"families\":{{\"long-explanation\":24,\"broken-explanation\":24,",
            "\"immediate-pattern\":12,\"parity-cycle\":10,\"empty-control\":2,\"random-2cnf\":48}},",
            "\"chain_lengths\":{:?},\"immediate_sizes\":{:?},\"parity_sizes\":{:?},\"seeds\":{:?},",
            "\"random_variables\":{:?},\"densities\":{:?},\"generator\":\"xorshift64\",",
            "\"effective_seed\":\"base_seed ^ (n << 32) ^ (d << 16)\",\"max_variables\":12}},\n",
            "\"observations\":[\n"
        ),
        CORPUS_CASES,
        CHAIN_LENGTHS,
        IMMEDIATE_SIZES,
        PARITY_SIZES.collect::<Vec<_>>(),
        SEEDS,
        RANDOM_VARIABLES,
        DENSITIES
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
            "\"outside_arms\":{{\"acquisition_work_units\":{},\"reference_work_units\":{},\"checker_work_units\":{}}},\n",
            "\"decision_comparison\":{{\"left\":\"graph decision including construction, SCC and explicit certificate output\",",
            "\"right\":\"DPLL Boolean decision without certificate\",",
            "\"restricted_to\":\"cases where both decisions completed; every completed decision agreed with the reference\"}},\n",
            "\"encoding\":{{\"vertex_labels\":\"usize 2*(v-1) and 2*(v-1)+1, at most 24 vertices\",",
            "\"clause_indices\":\"usize positions in the retained input\",\"literals\":\"i32 signed DIMACS-style\",",
            "\"counters\":\"u64 checked increments; identical in debug and release\"}},\n",
            "\"families\":{{"
        ),
        result.summary.json(),
        result.mining.work.work_units,
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
    output.push_str("}\n}\n");
    output
}

pub fn summary_line(result: &Experiment) -> String {
    let s = &result.summary;
    format!(
        concat!(
            "Checked {} cases in {} families. Acquisition {}; reference {}; checker {}. ",
            "Local {} ({} unknown); DPLL {} ({} unknown); graph decision {} ({} unknown), ",
            "backbone {} ({} unknown), total {}. Coverage on completed SAT cases: local {}/{}, graph {}/{}. ",
            "Graph decision vs DPLL on {} both-complete cases: {} better, {} tied, {} worse."
        ),
        s.cases,
        result.families.len(),
        result.mining.work.work_units,
        s.reference_work_units,
        s.checker_work_units,
        s.local.work_units,
        s.local.unknown,
        s.dpll.work_units,
        s.dpll.unknown,
        s.graph.decision_work_units,
        s.graph.decision_unknown,
        s.graph.backbone_work_units,
        s.graph.backbone_unknown,
        s.graph.total_work_units,
        s.local_coverage.found_literals,
        s.local_coverage.reference_literals,
        s.graph_coverage.found_literals,
        s.graph_coverage.reference_literals,
        s.graph_decision_vs_dpll.both_complete,
        s.graph_decision_vs_dpll.left_better,
        s.graph_decision_vs_dpll.tied,
        s.graph_decision_vs_dpll.left_worse
    )
}
