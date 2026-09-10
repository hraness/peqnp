//! Extraction cost: a settled extraction of every forced literal of the
//! binary fragment, with one stamped state allocated once, against the
//! per-literal search of the fragment-interface arm, holding every other
//! phase fixed. No truth table, reference label, or reference backbone
//! reaches any arm. See the frozen protocol
//! `experiments/extraction-cost-protocol.md`.

use crate::fragment::{self, BaselineSummary, Comparison, FragmentArm, FragmentSummary};
use crate::implication::{
    self, accumulate, add, array, count, increment, literal, node, optional, outcome_name, quoted,
    ticks, Certificate, Checks, Clue, Edge, Graph, Path, Reference,
};
use crate::transfer::{self, Arm, Event, Failure, Meter, Outcome};
use crate::Cnf;
use std::collections::BTreeMap;
use std::fmt::Write;

// SHA-256 of experiments/extraction-cost-protocol.md at its freeze commit 0e1e975.
pub const PROTOCOL_SHA256: &str =
    "e5da948b245df7c1055f6bdcd59e37c947f823c6ee39d858e66f3449a649f8d3";
pub const ARM_BUDGET: u64 = 1_000_000;
pub const SEEDS: [u64; 4] = [4001, 8009, 16001, 32003];
pub const VARIABLES: [u32; 3] = [8, 10, 12];
pub const DENSITIES: [u32; 3] = [3, 4, 5];
pub const LADDERS: [(u32, u32); 4] = [(2, 6), (3, 9), (5, 7), (1, 11)];
pub const PRIMARY_CASES: usize = 176;
pub const SECONDARY_CASES: usize = 162;

// ---------------------------------------------------------------------------
// Settled extraction: one stamped state, inheritance, and settled searches.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Unknown,
    Bad,
    Good,
}

/// Descriptive counts of the settled procedure's three hypothesized savings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettledStats {
    /// Depth-first searches run (each increments the query counter).
    pub searches: u64,
    /// Vertices settled bad by inheritance from a bad forward successor.
    pub inherited: u64,
    /// Vertices other than the search root whose status an exhausted search
    /// moved from unknown to good.
    pub settled_good: u64,
    /// Entries appended to the visited list across all searches, roots included.
    pub vertices_visited: u64,
}

impl SettledStats {
    fn include(&mut self, other: &SettledStats) -> Result<(), Failure> {
        add(&mut self.searches, other.searches)?;
        add(&mut self.inherited, other.inherited)?;
        add(&mut self.settled_good, other.settled_good)?;
        add(&mut self.vertices_visited, other.vertices_visited)
    }
    fn json(&self) -> String {
        format!(
            "{{\"searches\":{},\"inherited\":{},\"settled_good\":{},\"vertices_visited\":{}}}",
            self.searches, self.inherited, self.settled_good, self.vertices_visited
        )
    }
}

/// The stamped scratch state, allocated once per extraction.
struct State {
    status: Vec<Status>,
    stamp: Vec<u64>,
    predecessor: Vec<usize>,
    predecessor_clause: Vec<usize>,
    position: Vec<usize>,
    counter: u64,
}

impl State {
    fn next_stamp(&mut self) -> Result<u64, Failure> {
        self.counter = self
            .counter
            .checked_add(1)
            .ok_or(Failure::CounterOverflow)?;
        Ok(self.counter)
    }
}

/// Reduce a constructed path to a simple path by one left-to-right scan with
/// the stamped position array: a node already placed in this scan truncates
/// the path back to its first occurrence. Nodes cut by a truncation lose
/// their stamp so a later reappearance is placed afresh.
fn simplify(
    nodes: &mut Vec<i32>,
    clauses: &mut Vec<usize>,
    state: &mut State,
    m: &mut Meter,
) -> Result<(), Failure> {
    let current = state.next_stamp()?;
    let mut write = 0;
    for read in 0..nodes.len() {
        let x = node(nodes[read]);
        ticks(m, Event::LiteralRead, 2)?;
        if state.stamp[x] == current {
            let keep = state.position[x] + 1;
            m.tick(Event::ClauseWrite)?;
            for &cut in &nodes[keep..write] {
                m.tick(Event::LiteralWrite)?;
                state.stamp[node(cut)] = 0;
            }
            write = keep;
            continue;
        }
        if write != read {
            ticks(m, Event::LiteralRead, 2)?;
            ticks(m, Event::LiteralWrite, 2)?;
            nodes[write] = nodes[read];
            clauses[write - 1] = clauses[read - 1];
        }
        ticks(m, Event::LiteralWrite, 2)?;
        state.position[x] = write;
        state.stamp[x] = current;
        write += 1;
    }
    if write < nodes.len() {
        m.tick(Event::ClauseWrite)?;
        nodes.truncate(write);
        clauses.truncate(write - 1);
    }
    Ok(())
}

/// Copy a recorded path (from a bad vertex to its dual) onto the end of a
/// path under construction, skipping its first node when the destination
/// already ends there.
fn append_recorded(
    recorded: &Path,
    skip_first: bool,
    nodes: &mut Vec<i32>,
    clauses: &mut Vec<usize>,
    m: &mut Meter,
) -> Result<(), Failure> {
    m.tick(Event::ClauseRead)?;
    for &item in recorded.nodes.iter().skip(usize::from(skip_first)) {
        m.tick(Event::LiteralRead)?;
        m.tick(Event::LiteralWrite)?;
        nodes.push(item);
    }
    for &item in &recorded.clauses {
        m.tick(Event::LiteralRead)?;
        m.tick(Event::LiteralWrite)?;
        clauses.push(item);
    }
    Ok(())
}

/// The tree path from the search root to `target`, reconstructed by
/// following predecessors exactly as the existing path search does and
/// charged identically.
fn tree_path(root: usize, target: usize, state: &State, m: &mut Meter) -> Result<Path, Failure> {
    ticks(m, Event::ClauseWrite, 4)?;
    let mut reversed_nodes = Vec::new();
    let mut reversed_clauses = Vec::new();
    let mut v = target;
    loop {
        m.tick(Event::LiteralWrite)?;
        reversed_nodes.push(literal(v));
        if v == root {
            break;
        }
        ticks(m, Event::LiteralRead, 2)?;
        m.tick(Event::LiteralWrite)?;
        reversed_clauses.push(state.predecessor_clause[v]);
        v = state.predecessor[v];
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
    Ok(Path { nodes, clauses })
}

/// One depth-first search from `u` with the current stamp. Returns the
/// vertex that stopped it (the dual of `u` or a bad vertex), or `None` when
/// the stack exhausted.
fn search(
    g: &Graph,
    u: usize,
    state: &mut State,
    visited: &mut Vec<usize>,
    stats: &mut SettledStats,
    m: &mut Meter,
) -> Result<Option<usize>, Failure> {
    let current = state.next_stamp()?;
    increment(&mut stats.searches)?;
    ticks(m, Event::LiteralWrite, 2)?;
    state.stamp[u] = current;
    state.predecessor[u] = u;
    m.tick(Event::ClauseWrite)?;
    let mut stack: Vec<(usize, usize)> = Vec::new();
    ticks(m, Event::LiteralWrite, 2)?;
    stack.push((u, 0));
    m.tick(Event::LiteralWrite)?;
    visited.push(u);
    increment(&mut stats.vertices_visited)?;
    while let Some((v, next)) = stack.last().copied() {
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
        if state.stamp[e.to] == current {
            continue;
        }
        m.tick(Event::Search)?;
        ticks(m, Event::LiteralWrite, 3)?;
        state.stamp[e.to] = current;
        state.predecessor[e.to] = v;
        state.predecessor_clause[e.to] = e.clause;
        m.tick(Event::LiteralWrite)?;
        visited.push(e.to);
        increment(&mut stats.vertices_visited)?;
        m.tick(Event::LiteralRead)?;
        let status = state.status[e.to];
        if e.to == u ^ 1 || status == Status::Bad {
            return Ok(Some(e.to));
        }
        ticks(m, Event::LiteralWrite, 2)?;
        stack.push((e.to, 0));
    }
    Ok(None)
}

/// The path of a vertex settled bad by inheritance from the edge `u -> v`:
/// `u`, the recorded path of `v`, then the dual of `u` by the dual edge.
fn inherited_path(
    u: usize,
    e: Edge,
    recorded: &Path,
    state: &mut State,
    m: &mut Meter,
) -> Result<Path, Failure> {
    m.tick(Event::LiteralRead)?;
    ticks(m, Event::ClauseWrite, 2)?;
    let mut nodes = Vec::new();
    let mut clauses = Vec::new();
    ticks(m, Event::LiteralWrite, 2)?;
    nodes.push(literal(u));
    clauses.push(e.clause);
    append_recorded(recorded, false, &mut nodes, &mut clauses, m)?;
    ticks(m, Event::LiteralWrite, 2)?;
    clauses.push(e.clause);
    nodes.push(literal(u ^ 1));
    simplify(&mut nodes, &mut clauses, state, m)?;
    Ok(Path { nodes, clauses })
}

/// The path of a vertex settled bad by a search that stopped at `target`:
/// the tree path alone when the target is the dual of `u`; otherwise the
/// tree path, the recorded path of the bad target, and the duals of the
/// tree edges in reverse order, each citing the clause it mirrors.
fn searched_path(
    u: usize,
    target: usize,
    recorded: Option<&Path>,
    state: &mut State,
    m: &mut Meter,
) -> Result<Path, Failure> {
    let Path {
        mut nodes,
        mut clauses,
    } = tree_path(u, target, state, m)?;
    let Some(recorded) = recorded else {
        return Ok(Path { nodes, clauses });
    };
    let tree_length = nodes.len();
    append_recorded(recorded, true, &mut nodes, &mut clauses, m)?;
    for index in (1..tree_length).rev() {
        ticks(m, Event::LiteralRead, 2)?;
        ticks(m, Event::LiteralWrite, 2)?;
        let clause = clauses[index - 1];
        let dual = -nodes[index - 1];
        clauses.push(clause);
        nodes.push(dual);
    }
    simplify(&mut nodes, &mut clauses, state, m)?;
    Ok(Path { nodes, clauses })
}

/// Settle every vertex of the fragment graph as bad (a path to its dual
/// exists) or good, then emit the forced literals in the order
/// `1, -1, 2, -2, ...`. Requires a satisfiable fragment: the certificate
/// phase has already refuted any component containing a literal and its
/// dual, which the dual-good rule relies on.
pub(crate) fn settled_extract(
    g: &Graph,
    component: &[usize],
    n: u32,
    m: &mut Meter,
) -> Result<(Vec<Clue>, SettledStats), Failure> {
    let size = g.forward.len();
    if size != 2 * n as usize || component.len() != size {
        return Err(Failure::InvalidInput);
    }
    let mut state = State {
        status: array(size, Status::Unknown, m)?,
        stamp: array(size, 0_u64, m)?,
        predecessor: array(size, usize::MAX, m)?,
        predecessor_clause: array(size, usize::MAX, m)?,
        position: array(size, usize::MAX, m)?,
        counter: 0,
    };
    m.tick(Event::ClauseWrite)?;
    let mut recorded: Vec<Option<Path>> = (0..size).map(|_| None).collect();
    m.tick(Event::ClauseWrite)?;
    let mut visited: Vec<usize> = Vec::new();
    let mut stats = SettledStats::default();
    // Group vertices by component: one container write for the group list,
    // one per group, and one component read plus one append per vertex.
    m.tick(Event::ClauseWrite)?;
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (x, &c) in component.iter().enumerate() {
        m.tick(Event::LiteralRead)?;
        while groups.len() <= c {
            m.tick(Event::ClauseWrite)?;
            groups.push(Vec::new());
        }
        m.tick(Event::LiteralWrite)?;
        groups[c].push(x);
    }
    for group in groups.iter().rev() {
        for &u in group {
            m.tick(Event::LiteralRead)?;
            if state.status[u] != Status::Unknown {
                continue;
            }
            m.tick(Event::ClauseRead)?;
            let mut inherited: Option<Edge> = None;
            for e in &g.forward[u] {
                ticks(m, Event::LiteralRead, 2)?;
                if state.status[e.to] == Status::Bad {
                    inherited = Some(*e);
                    break;
                }
            }
            let path = if let Some(e) = inherited {
                increment(&mut stats.inherited)?;
                let source = recorded[e.to].as_ref().expect("bad vertex has a path");
                Some(inherited_path(u, e, source, &mut state, m)?)
            } else {
                match search(g, u, &mut state, &mut visited, &mut stats, m)? {
                    Some(target) if target == u ^ 1 => {
                        Some(searched_path(u, target, None, &mut state, m)?)
                    }
                    Some(target) => {
                        let source = recorded[target].as_ref().expect("bad vertex has a path");
                        Some(searched_path(u, target, Some(source), &mut state, m)?)
                    }
                    None => None,
                }
            };
            match path {
                Some(path) => {
                    m.tick(Event::ClauseWrite)?;
                    recorded[u] = Some(path);
                    ticks(m, Event::LiteralWrite, 2)?;
                    state.status[u] = Status::Bad;
                    state.status[u ^ 1] = Status::Good;
                }
                None => {
                    for &w in &visited {
                        m.tick(Event::LiteralRead)?;
                        m.tick(Event::LiteralWrite)?;
                        if w != u && state.status[w] == Status::Unknown {
                            increment(&mut stats.settled_good)?;
                        }
                        state.status[w] = Status::Good;
                    }
                }
            }
            m.tick(Event::ClauseWrite)?;
            visited.clear();
        }
    }
    m.tick(Event::ClauseWrite)?;
    let mut clues = Vec::new();
    for v in 1..=n as i32 {
        for l in [v, -v] {
            m.tick(Event::LiteralRead)?;
            let dual = node(-l);
            if state.status[dual] == Status::Bad {
                m.tick(Event::ClauseWrite)?;
                m.tick(Event::LiteralWrite)?;
                let path = recorded[dual].take().expect("bad vertex has a path");
                clues.push(Clue { literal: l, path });
            }
        }
    }
    Ok((clues, stats))
}

// ---------------------------------------------------------------------------
// One driver for both fragment arms; only the extraction function differs.

/// Build the fragment graph, decide the fragment, extract its forced
/// literals with the supplied procedure, append them to a copy of the
/// original formula, and run the common DPLL with whatever budget remains.
/// Every phase other than extraction is this one code path for both arms.
pub(crate) fn drive<S>(
    input: &Cnf,
    n: u32,
    budget: u64,
    extract: impl FnOnce(&Graph, &[usize], u32, &mut Meter) -> Result<(Vec<Clue>, S), Failure>,
) -> Result<(FragmentArm, Option<S>), Failure> {
    let mut meter = Meter::new(budget);
    let mut arm = FragmentArm::empty();
    let Some(graph) = fragment::phase(&mut meter, &mut arm.construction, |m| {
        implication::build_fragment(input, n, m)
    })?
    else {
        return Ok((fragment::exhausted(arm, meter), None));
    };
    let Some(component) = fragment::phase(&mut meter, &mut arm.components, |m| {
        implication::components(&graph, m)
    })?
    else {
        return Ok((fragment::exhausted(arm, meter), None));
    };
    let Some(certificate) = fragment::phase(&mut meter, &mut arm.decision_certificate, |m| {
        implication::decision_certificate(&graph, &component, n, m)
    })?
    else {
        return Ok((fragment::exhausted(arm, meter), None));
    };
    if !matches!(certificate, Certificate::Sat(_)) {
        arm.outcome = Outcome::Unsat;
        arm.certificate = Some(certificate);
        arm.total = meter.work;
        return Ok((arm, None));
    }
    let Some((clues, stats)) = fragment::phase(&mut meter, &mut arm.extraction, |m| {
        extract(&graph, &component, n, m)
    })?
    else {
        return Ok((fragment::exhausted(arm, meter), None));
    };
    let Some(processed) = fragment::phase(&mut meter, &mut arm.append, |m| {
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
        return Ok((fragment::exhausted(arm, meter), None));
    };
    let Some(satisfiable) = fragment::phase(&mut meter, &mut arm.residual, |m| {
        transfer::decide(&processed, n, m)
    })?
    else {
        return Ok((fragment::exhausted(arm, meter), None));
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
    Ok((arm, Some(stats)))
}

/// Arm 2: the driver with the fragment-interface per-literal extraction.
/// The experiment asserts on every case that this equals
/// `fragment::solve_fragment` in every field.
pub fn solve_per_literal(input: &Cnf, n: u32, budget: u64) -> Result<FragmentArm, Failure> {
    let (arm, _) = drive(input, n, budget, |g, _, n, m| {
        implication::extract(g, n, m).map(|clues| (clues, ()))
    })?;
    Ok(arm)
}

#[derive(Clone, Debug)]
pub struct SettledArm {
    pub arm: FragmentArm,
    /// Present only when the settled extraction ran to completion.
    pub stats: Option<SettledStats>,
}

/// Arm 3: the driver with the settled extraction.
pub fn solve_settled(input: &Cnf, n: u32, budget: u64) -> Result<SettledArm, Failure> {
    let (arm, stats) = drive(input, n, budget, settled_extract)?;
    Ok(SettledArm { arm, stats })
}

/// Field-by-field equality of two fragment arms.
pub fn same_arm(a: &FragmentArm, b: &FragmentArm) -> bool {
    a.outcome == b.outcome
        && a.certificate == b.certificate
        && a.clues == b.clues
        && a.derived_units == b.derived_units
        && a.construction == b.construction
        && a.components == b.components
        && a.decision_certificate == b.decision_certificate
        && a.extraction == b.extraction
        && a.append == b.append
        && a.residual == b.residual
        && a.total == b.total
}

// ---------------------------------------------------------------------------
// Fixed corpus: 176 primary cases, then the 162 fragment-interface-v1 cases.

#[derive(Clone, Debug)]
pub struct Case {
    pub id: String,
    pub family: String,
    pub secondary: bool,
    pub variables: u32,
    pub input: Cnf,
    pub seed: Option<u64>,
    pub effective_seed: Option<u64>,
    pub density: Option<u32>,
    pub binary_count: Option<u32>,
    pub binary_label: Option<String>,
    pub ladder: Option<(u32, u32)>,
    pub chain_length: Option<u32>,
    pub sign: Option<i32>,
    pub cycle_length: Option<u32>,
}

fn plain(id: String, family: &str, variables: u32, input: Cnf) -> Case {
    Case {
        id,
        family: family.into(),
        secondary: false,
        variables,
        input,
        seed: None,
        effective_seed: None,
        density: None,
        binary_count: None,
        binary_label: None,
        ladder: None,
        chain_length: None,
        sign: None,
        cycle_length: None,
    }
}

fn sign_name(sign: i32) -> &'static str {
    if sign > 0 {
        "pos"
    } else {
        "neg"
    }
}

/// The ladder control: `y_1..y_k` are `1..k`, `x_1..x_j` are `k+1..k+j`;
/// `F_k` on `x_1` with the y chain, then the x chain. The negative variant
/// negates every occurrence of every x variable.
pub fn ladder(k: u32, j: u32, sign: i32) -> Cnf {
    let y = |i: u32| i as i32;
    let x = |i: u32| (k + i) as i32 * sign;
    let mut clauses = vec![vec![x(1), y(1)]];
    for i in 1..k {
        clauses.push(vec![-y(i), y(i + 1)]);
    }
    clauses.push(vec![-y(k), x(1)]);
    for i in 1..j {
        clauses.push(vec![-x(i), x(i + 1)]);
    }
    clauses
}

/// The forced ladder literals of a ladder control, in literal order.
pub fn ladder_units(k: u32, j: u32, sign: i32) -> Vec<i32> {
    (1..=j).map(|i| (k + i) as i32 * sign).collect()
}

fn random_case(seed: u64, n: u32, t: u32, b: u32, label: &str, id: String, family: &str) -> Case {
    let effective = fragment::effective_seed(seed, n, t, b);
    let mut case = plain(id, family, n, fragment::random(n, t, b, effective));
    case.seed = Some(seed);
    case.effective_seed = Some(effective);
    case.density = Some(t);
    case.binary_count = Some(b);
    case.binary_label = Some(label.into());
    case
}

fn primary() -> Vec<Case> {
    let mut cases = Vec::new();
    for seed in SEEDS {
        for n in VARIABLES {
            for t in DENSITIES {
                for (label, b) in [
                    ("b=n", n),
                    ("b=2n", 2 * n),
                    ("b=3n", 3 * n),
                    ("b=4n", 4 * n),
                ] {
                    cases.push(random_case(
                        seed,
                        n,
                        t,
                        b,
                        label,
                        format!("random-n{n}-t{t}-b{b}-s{seed}"),
                        "random-mixed",
                    ));
                }
            }
        }
    }
    for seed in SEEDS {
        for n in VARIABLES {
            for (label, b) in [("b=n", n), ("b=2n", 2 * n)] {
                cases.push(random_case(
                    seed,
                    n,
                    0,
                    b,
                    label,
                    format!("fragment-only-n{n}-b{b}-s{seed}"),
                    "fragment-only",
                ));
            }
        }
    }
    for (k, j) in LADDERS {
        for sign in [1, -1] {
            let mut case = plain(
                format!("ladder-k{k}-j{j}-{}", sign_name(sign)),
                "ladder-control",
                k + j,
                ladder(k, j, sign),
            );
            case.ladder = Some((k, j));
            case.chain_length = Some(k);
            case.sign = Some(sign);
            case.density = Some(0);
            case.binary_count = Some(k + j);
            cases.push(case);
        }
    }
    cases
}

fn secondary() -> Result<Vec<Case>, Failure> {
    Ok(fragment::corpus()?
        .into_iter()
        .map(|case| Case {
            id: format!("v1:{}", case.id),
            family: format!("v1:{}", case.family),
            secondary: true,
            variables: case.variables,
            input: case.input,
            seed: case.seed,
            effective_seed: case.effective_seed,
            density: case.density,
            binary_count: case.binary_count,
            binary_label: case.binary_label.map(String::from),
            ladder: None,
            chain_length: case.chain_length,
            sign: case.sign,
            cycle_length: case.cycle_length,
        })
        .collect())
}

/// The fixed evaluation set in protocol enumeration order: 176 primary
/// cases, rejected if any exact `(declared variables, ordered CNF)` pair
/// repeats within them or appears in one of the four earlier corpora, then
/// the 162 secondary cases, which are the fragment-interface-v1 corpus by
/// construction.
pub fn corpus() -> Result<Vec<Case>, Failure> {
    let cases = primary();
    if cases.len() != PRIMARY_CASES {
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
    let v1 = secondary()?;
    if v1.len() != SECONDARY_CASES {
        return Err(Failure::InvalidInput);
    }
    earlier.extend(v1.iter().map(|case| (case.variables, case.input.clone())));
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
    let mut all = cases;
    all.extend(v1);
    Ok(all)
}

// ---------------------------------------------------------------------------
// Experiment: reference, three arms, raw checkers, acceptance, summaries.

pub struct Observation {
    pub case: Case,
    pub reference: Reference,
    pub baseline: Arm,
    pub per_literal: FragmentArm,
    pub settled: SettledArm,
    pub per_literal_checks: Checks,
    pub settled_checks: Checks,
}

/// Raw checkers on a fragment certificate and every derived unit's path.
fn check(case: &Case, arm: &FragmentArm) -> Result<Checks, Failure> {
    let mut checks = Checks::default();
    if let Some(certificate) = &arm.certificate {
        let (valid, work) = fragment::check_certificate(&case.input, case.variables, certificate)?;
        checks.certificate_valid = Some(valid);
        accumulate(&mut checks.work, &work)?;
    }
    for clue in &arm.clues {
        let (valid, work) = fragment::check_clue(&case.input, case.variables, clue)?;
        increment(&mut checks.clues_checked)?;
        add(&mut checks.clues_valid, u64::from(valid))?;
        accumulate(&mut checks.work, &work)?;
    }
    Ok(checks)
}

fn phases_after_certificate_are_idle(arm: &FragmentArm) -> bool {
    arm.extraction.work_units == 0 && arm.append.work_units == 0 && arm.residual.work_units == 0
}

fn sorted(literals: &[i32]) -> Vec<i32> {
    let mut result = literals.to_vec();
    result.sort_unstable();
    result
}

/// The fragment-arm invariants shared by both fragment arms.
fn accept_fragment_arm(
    row: &Observation,
    arm: &FragmentArm,
    checks: &Checks,
) -> Result<(), Failure> {
    let label = row.reference.model_count > 0;
    if !arm.outcome.agrees(label) || arm.total_work() > ARM_BUDGET {
        return Err(Failure::InvalidInput);
    }
    if arm.total_work() != arm.fragment_work() + arm.residual.work_units {
        return Err(Failure::InvalidInput);
    }
    match arm.outcome {
        Outcome::Unknown => {
            if arm.total_work() != ARM_BUDGET
                || arm.certificate.is_some()
                || !arm.clues.is_empty()
                || !arm.derived_units.is_empty()
            {
                return Err(Failure::InvalidInput);
            }
        }
        Outcome::Unsat | Outcome::Sat => {
            let Some(certificate) = &arm.certificate else {
                return Err(Failure::InvalidInput);
            };
            if !matches!(certificate, Certificate::Sat(_)) {
                if arm.outcome != Outcome::Unsat
                    || !arm.clues.is_empty()
                    || !arm.derived_units.is_empty()
                    || !phases_after_certificate_are_idle(arm)
                {
                    return Err(Failure::InvalidInput);
                }
            } else if arm.residual.work_units == 0 {
                return Err(Failure::InvalidInput);
            }
            let units: Vec<i32> = arm.clues.iter().map(|clue| clue.literal).collect();
            if units != arm.derived_units {
                return Err(Failure::InvalidInput);
            }
        }
    }
    if arm.certificate.is_some() != checks.certificate_valid.is_some()
        || checks.certificate_valid == Some(false)
        || checks.clues_checked != count(arm.clues.len())?
        || checks.clues_valid != checks.clues_checked
    {
        return Err(Failure::InvalidInput);
    }
    if let Some(backbone) = &row.reference.backbone {
        let mut seen: Vec<i32> = Vec::new();
        for unit in &arm.derived_units {
            if !backbone.contains(unit) || seen.contains(unit) {
                return Err(Failure::InvalidInput);
            }
            seen.push(*unit);
        }
    }
    // Family expectations: the ladder controls and the v1 control families.
    if arm.outcome != Outcome::Unknown {
        match row.case.family.as_str() {
            "ladder-control" => {
                let (k, j) = row.case.ladder.ok_or(Failure::InvalidInput)?;
                let sign = row.case.sign.ok_or(Failure::InvalidInput)?;
                if !matches!(arm.certificate, Some(Certificate::Sat(_)))
                    || arm.derived_units != ladder_units(k, j, sign)
                    || arm
                        .derived_units
                        .iter()
                        .any(|unit| unit.unsigned_abs() <= k)
                {
                    return Err(Failure::InvalidInput);
                }
            }
            "v1:chain-embedded" => {
                let sign = row.case.sign.ok_or(Failure::InvalidInput)?;
                if !matches!(arm.certificate, Some(Certificate::Sat(_)))
                    || arm.derived_units != vec![sign]
                {
                    return Err(Failure::InvalidInput);
                }
            }
            "v1:contradictory-fragment" => {
                if arm.outcome != Outcome::Unsat
                    || !matches!(
                        arm.certificate,
                        Some(Certificate::EmptyClause(_) | Certificate::OppositePaths { .. })
                    )
                    || !phases_after_certificate_are_idle(arm)
                {
                    return Err(Failure::InvalidInput);
                }
            }
            "v1:random-ternary-only"
                if !arm.derived_units.is_empty()
                    || !matches!(arm.certificate, Some(Certificate::Sat(_))) =>
            {
                return Err(Failure::InvalidInput);
            }
            _ => (),
        }
    }
    Ok(())
}

/// Every protocol acceptance check. A violation invalidates the calibration.
fn accept(row: &Observation) -> Result<(), Failure> {
    let label = row.reference.model_count > 0;
    if label != row.reference.backbone.is_some() {
        return Err(Failure::InvalidInput);
    }
    let baseline = &row.baseline;
    if !baseline.outcome.agrees(label) || baseline.total_work() > ARM_BUDGET {
        return Err(Failure::InvalidInput);
    }
    if baseline.outcome == Outcome::Unknown && baseline.total_work() != ARM_BUDGET {
        return Err(Failure::InvalidInput);
    }
    let per_literal = &row.per_literal;
    let settled = &row.settled.arm;
    accept_fragment_arm(row, per_literal, &row.per_literal_checks)?;
    accept_fragment_arm(row, settled, &row.settled_checks)?;
    // Arm 2 is the unchanged fragment-interface-v1 arm.
    let v1 = fragment::solve_fragment(&row.case.input, row.case.variables, ARM_BUDGET)?;
    if !same_arm(per_literal, &v1) {
        return Err(Failure::InvalidInput);
    }
    // Settled stats exist exactly when the settled extraction completed.
    let extracted = settled.outcome != Outcome::Unknown
        && matches!(settled.certificate, Some(Certificate::Sat(_)));
    if row.settled.stats.is_some() != extracted {
        return Err(Failure::InvalidInput);
    }
    if let Some(stats) = &row.settled.stats {
        // Each vertex is settled by at most one route, every search appends
        // its root and one entry per search-node event, and each search
        // settles a bad root or an exhausted root plus the settled-good list.
        let vertices = u64::from(row.case.variables) * 2;
        let searches_plus_inherited = stats
            .searches
            .checked_add(stats.inherited)
            .ok_or(Failure::CounterOverflow)?;
        let visited = stats
            .searches
            .checked_add(settled.extraction.search_nodes)
            .ok_or(Failure::CounterOverflow)?;
        if searches_plus_inherited > vertices
            || stats.settled_good > vertices
            || stats.vertices_visited != visited
        {
            return Err(Failure::InvalidInput);
        }
    }
    // Arms 2 and 3 differ in extraction alone: when both completed, the same
    // unit set, the same ordered units, the same processed formula (both
    // append in literal order), and identical counters in every other phase.
    let both_complete =
        per_literal.outcome != Outcome::Unknown && settled.outcome != Outcome::Unknown;
    if both_complete
        && (per_literal.outcome != settled.outcome
            || per_literal.certificate != settled.certificate
            || sorted(&per_literal.derived_units) != sorted(&settled.derived_units)
            || per_literal.derived_units != settled.derived_units
            || per_literal.construction != settled.construction
            || per_literal.components != settled.components
            || per_literal.decision_certificate != settled.decision_certificate
            || per_literal.append != settled.append
            || per_literal.residual != settled.residual)
    {
        return Err(Failure::InvalidInput);
    }
    Ok(())
}

/// The extraction phase alone, settled on the left, compared only when both
/// arms completed and the fragment was satisfiable so both extractions ran.
#[derive(Clone, Debug, Default)]
pub struct ExtractionComparison {
    pub compared: u64,
    pub settled_better: u64,
    pub tied: u64,
    pub settled_worse: u64,
    pub not_compared: u64,
    pub per_literal_work_units: u64,
    pub settled_work_units: u64,
    pub per_literal_searches: u64,
    pub settled_searches: u64,
}

impl ExtractionComparison {
    fn include(&mut self, row: &Observation) -> Result<(), Failure> {
        let per_literal = &row.per_literal;
        let settled = &row.settled.arm;
        let both = per_literal.outcome != Outcome::Unknown
            && settled.outcome != Outcome::Unknown
            && matches!(settled.certificate, Some(Certificate::Sat(_)));
        let Some(stats) = row.settled.stats.as_ref().filter(|_| both) else {
            return increment(&mut self.not_compared);
        };
        increment(&mut self.compared)?;
        add(
            &mut self.per_literal_work_units,
            per_literal.extraction.work_units,
        )?;
        add(&mut self.settled_work_units, settled.extraction.work_units)?;
        add(
            &mut self.per_literal_searches,
            per_literal.extraction.rule_attempts,
        )?;
        add(&mut self.settled_searches, stats.searches)?;
        increment(
            match settled
                .extraction
                .work_units
                .cmp(&per_literal.extraction.work_units)
            {
                std::cmp::Ordering::Less => &mut self.settled_better,
                std::cmp::Ordering::Equal => &mut self.tied,
                std::cmp::Ordering::Greater => &mut self.settled_worse,
            },
        )
    }
    fn json(&self) -> String {
        format!(
            concat!(
                "{{\"compared\":{},\"settled_better\":{},\"tied\":{},\"settled_worse\":{},\"not_compared\":{},",
                "\"per_literal_work_units\":{},\"settled_work_units\":{},\"per_literal_searches\":{},\"settled_searches\":{}}}"
            ),
            self.compared,
            self.settled_better,
            self.tied,
            self.settled_worse,
            self.not_compared,
            self.per_literal_work_units,
            self.settled_work_units,
            self.per_literal_searches,
            self.settled_searches
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
    pub baseline: BaselineSummary,
    pub per_literal: FragmentSummary,
    pub settled: FragmentSummary,
    pub per_literal_searches: u64,
    pub settled_stats: SettledStats,
    pub settled_stats_cases: u64,
    pub settled_vs_baseline: Comparison,
    pub settled_vs_per_literal: Comparison,
    pub per_literal_vs_baseline: Comparison,
    pub extraction: ExtractionComparison,
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
        for checks in [&row.per_literal_checks, &row.settled_checks] {
            add(&mut self.checker_work_units, checks.work.work_units)?;
            add(
                &mut self.certificates_checked,
                u64::from(checks.certificate_valid.is_some()),
            )?;
            add(&mut self.clues_checked, checks.clues_checked)?;
        }
        self.baseline.include(&row.baseline)?;
        self.per_literal.include(&row.per_literal)?;
        self.settled.include(&row.settled.arm)?;
        add(
            &mut self.per_literal_searches,
            row.per_literal.extraction.rule_attempts,
        )?;
        if let Some(stats) = &row.settled.stats {
            self.settled_stats.include(stats)?;
            increment(&mut self.settled_stats_cases)?;
        }
        // Acceptance already established that every completed decision
        // agrees with the reference, so completed totals are comparable.
        let complete =
            |outcome: &Outcome, total: u64| (*outcome != Outcome::Unknown).then_some(total);
        let baseline = complete(&row.baseline.outcome, row.baseline.total_work());
        let per_literal = complete(&row.per_literal.outcome, row.per_literal.total_work());
        let settled = complete(&row.settled.arm.outcome, row.settled.arm.total_work());
        self.settled_vs_baseline.include(settled, baseline)?;
        self.settled_vs_per_literal.include(settled, per_literal)?;
        self.per_literal_vs_baseline
            .include(per_literal, baseline)?;
        self.extraction.include(row)
    }
    fn json(&self) -> String {
        format!(
            concat!(
                "{{\"cases\":{},\"reference_sat\":{},\"reference_unsat\":{},\"reference_work_units\":{},",
                "\"checker_work_units\":{},\"certificates_checked\":{},\"clues_checked\":{},\n",
                "  \"arms\":{{\"baseline\":{},\n   \"per_literal\":{},\n   \"settled\":{}}},\n",
                "  \"searches\":{{\"per_literal\":{},\"settled\":{}}},",
                "\"settled_stats\":{},\"settled_stats_cases\":{},\n",
                "  \"comparisons\":{{\"settled_vs_baseline\":{},\n   \"settled_vs_per_literal\":{},\n",
                "   \"per_literal_vs_baseline\":{},\n   \"extraction_settled_vs_per_literal\":{}}},\n",
                "  \"enumeration_ratio\":{{\"reference_over_baseline\":{},\"reference_over_per_literal\":{},\"reference_over_settled\":{}}}}}"
            ),
            self.cases,
            self.reference_sat,
            self.reference_unsat,
            self.reference_work_units,
            self.checker_work_units,
            self.certificates_checked,
            self.clues_checked,
            self.baseline.json(),
            self.per_literal.json(),
            self.settled.json(),
            self.per_literal_searches,
            self.settled_stats.searches,
            self.settled_stats.json(),
            self.settled_stats_cases,
            self.settled_vs_baseline.json(),
            self.settled_vs_per_literal.json(),
            self.per_literal_vs_baseline.json(),
            self.extraction.json(),
            fragment::ratio(self.reference_work_units, self.baseline.work_units),
            fragment::ratio(self.reference_work_units, self.per_literal.total_work_units),
            fragment::ratio(self.reference_work_units, self.settled.total_work_units)
        )
    }
}

/// One corpus section: its summary, families, and the random-mixed
/// subtotals by binary count.
#[derive(Default)]
pub struct Section {
    pub summary: Summary,
    pub families: BTreeMap<String, Summary>,
    pub binary_counts: BTreeMap<String, Summary>,
}

impl Section {
    fn include(&mut self, row: &Observation, mixed_family: &str) -> Result<(), Failure> {
        self.summary.include(row)?;
        self.families
            .entry(row.case.family.clone())
            .or_default()
            .include(row)?;
        if row.case.family == mixed_family {
            let label = row.case.binary_label.clone().ok_or(Failure::InvalidInput)?;
            self.binary_counts.entry(label).or_default().include(row)?;
        }
        Ok(())
    }
}

pub struct Experiment {
    pub observations: Vec<Observation>,
    pub primary: Section,
    pub secondary: Section,
}

/// Run only against the frozen protocol. No label or reference backbone
/// reaches any arm; the checkers run after the arms on each case.
pub fn experiment() -> Result<Experiment, Failure> {
    if PROTOCOL_SHA256.len() != 64 {
        return Err(Failure::InvalidInput);
    }
    let cases = corpus()?;
    let mut observations = Vec::new();
    let mut primary = Section::default();
    let mut secondary = Section::default();
    for case in cases {
        let reference = fragment::reference(&case.input, case.variables)?;
        let baseline = transfer::solve(&case.input, case.variables, None, ARM_BUDGET)?;
        let per_literal = solve_per_literal(&case.input, case.variables, ARM_BUDGET)?;
        let settled = solve_settled(&case.input, case.variables, ARM_BUDGET)?;
        let per_literal_checks = check(&case, &per_literal)?;
        let settled_checks = check(&case, &settled.arm)?;
        let row = Observation {
            case,
            reference,
            baseline,
            per_literal,
            settled,
            per_literal_checks,
            settled_checks,
        };
        accept(&row)?;
        if row.case.secondary {
            secondary.include(&row, "v1:random-mixed")?;
        } else {
            primary.include(&row, "random-mixed")?;
        }
        observations.push(row);
    }
    if primary.summary.cases != count(PRIMARY_CASES)?
        || secondary.summary.cases != count(SECONDARY_CASES)?
    {
        return Err(Failure::InvalidInput);
    }
    Ok(Experiment {
        observations,
        primary,
        secondary,
    })
}

// ---------------------------------------------------------------------------
// Deterministic reporting. Formatting is outside every meter.

fn checks_json(checks: &Checks) -> String {
    format!(
        "{{\"certificate_valid\":{},\"clues_checked\":{},\"clues_valid\":{},\"work\":{}}}",
        optional(checks.certificate_valid),
        checks.clues_checked,
        checks.clues_valid,
        checks.work.json()
    )
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
    let (ladder_k, ladder_j) = row
        .case
        .ladder
        .map_or((None, None), |(k, j)| (Some(k), Some(j)));
    format!(
        concat!(
            "{{\"id\":{},\"family\":{},\"secondary\":{},\"variables\":{},\"used_variables\":{},\"clauses\":{},\"fragment_clauses\":{},",
            "\"seed\":{},\"effective_seed\":{},\"density\":{},\"binary_count\":{},\"binary_label\":{},",
            "\"ladder_k\":{},\"ladder_j\":{},\"chain_length\":{},\"sign\":{},\"cycle_length\":{},\"input\":{:?},\n",
            " \"reference\":{{\"model_count\":{},\"sat\":{},\"backbone\":{},\"work\":{}}},\n",
            " \"baseline\":{{\"outcome\":{},\"work\":{},\"total_work_units\":{},\"search_nodes\":{}}},\n",
            " \"per_literal\":{},\n",
            " \"settled\":{},\n",
            " \"settled_stats\":{},\n",
            " \"checks\":{{\"per_literal\":{},\"settled\":{}}}}}"
        ),
        quoted(&row.case.id),
        quoted(&row.case.family),
        row.case.secondary,
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
            .as_deref()
            .map_or_else(|| "null".into(), quoted),
        optional(ladder_k),
        optional(ladder_j),
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
        row.per_literal.json(),
        row.settled.arm.json(),
        row.settled
            .stats
            .as_ref()
            .map_or_else(|| "null".into(), SettledStats::json),
        checks_json(&row.per_literal_checks),
        checks_json(&row.settled_checks)
    )
}

const LIMITATIONS: [&str; 12] = [
    "Calibration of a known interface: the Aspvall-Plass-Tarjan decision and path-based forced literals of the binary fragment feed a toy DPLL; no novel inference rule, evolutionary advantage, or general SAT complexity result.",
    "No algorithm is known that reads every forced literal of a 2-CNF in time linear in its size; Buss, Kullmann, and Vassilevska Williams show a subquadratic complete 2-CNF backbone algorithm would improve the best known k-cycle detection. Both extraction procedures have worst-case cost O(n(n + m2)) events; the settled procedure adds O(n) initialization and O(n) output work and may perform fewer than 2n searches. No linear bound is claimed for either.",
    "The three hypothesized savings of the settled procedure (one initialization instead of one per query, queries answered by inheritance, queries settled by an earlier failed search) are measured in this event model, not proved.",
    "Finite fixed corpus of at most 12 declared variables and clause width at most three; pseudorandom cases are reproducible samples, not independent or representative population estimates, and all sizes are calibration sizes chosen so the truth-table reference stays feasible, not scaling evidence.",
    "Arms 2 and 3 share one driver and differ in extraction alone; when both complete, their processed formulas and residual counters are asserted identical, so the difference between them is extraction cost only.",
    "The fragment arms use no learned library; the implication-graph procedures are fixed algorithmic knowledge with no acquisition step, so no acquisition cost is reported.",
    "The residual DPLL has no polynomial bound; measured totals do not prove any stated phase bound.",
    "Declared event counters exclude allocator internals, loop control, arithmetic and comparison instructions, input generation, JSON formatting, and report construction; they are operational measurements, not elapsed time or a proved bit-cost bound.",
    "Reference truth-table and raw-clause checker work are outside all arms and reported separately; the enumeration ratio is descriptive only.",
    "Budget exhaustion means unknown: no exhausted arm is reported as SAT or UNSAT, an unknown arm reports no certificate, unit, or settled statistics, and comparisons count only cases where both arms completed; an unfinished arm's smaller work count is not a speedup.",
    "A fragment SAT certificate is an assignment of the binary fragment, not a model of the whole formula; only fragment UNSAT certificates decide the whole formula without search. UNSAT cases have no defined solution-set backbone; the subset check on derived units applies to satisfiable cases only.",
    "No required result is a positive delta; a negative result is reported with the same completeness as a positive one.",
];

fn section_json(output: &mut String, section: &Section, subtotal_key: &str) {
    write!(
        output,
        "\"summary\":{},\n\"families\":{{",
        section.summary.json()
    )
    .expect("string write");
    for (index, (family, summary)) in section.families.iter().enumerate() {
        if index > 0 {
            output.push_str(",\n");
        }
        write!(output, "{}:{}", quoted(family), summary.json()).expect("string write");
    }
    write!(output, "}},\n{}:{{", quoted(subtotal_key)).expect("string write");
    for (index, (label, summary)) in section.binary_counts.iter().enumerate() {
        if index > 0 {
            output.push_str(",\n");
        }
        write!(output, "{}:{}", quoted(label), summary.json()).expect("string write");
    }
    output.push('}');
}

pub fn json(result: &Experiment) -> String {
    let mut output = format!(
        concat!(
            "{{\n\"schema_version\":1,\"experiment\":\"extraction-cost-v1\",\"status\":\"bounded-tested\",",
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
            "],\n\"corpus\":{{\"primary_cases\":{},\"primary_families\":{{\"random-mixed\":144,\"fragment-only\":24,\"ladder-control\":8}},",
            "\"seeds\":{:?},\"variables\":{:?},\"ternary_densities\":{:?},\"binary_counts\":[\"n\",\"2n\",\"3n\",\"4n\"],",
            "\"fragment_only_density\":0,\"fragment_only_binary_counts\":[\"n\",\"2n\"],\"ladders\":[[2,6],[3,9],[5,7],[1,11]],",
            "\"generator\":\"xorshift64\",\"effective_seed\":\"base_seed ^ (n << 32) ^ (t << 40) ^ (b << 48)\",",
            "\"max_variables\":12,\"max_width\":3,\"exact_earlier_overlap\":0,",
            "\"secondary_cases\":{},\"secondary_source\":\"fragment-interface-v1 corpus regenerated by its frozen generator; ids and families prefixed v1:\"}},\n",
            "\"observations\":[\n"
        ),
        PRIMARY_CASES,
        SEEDS,
        VARIABLES,
        DENSITIES,
        SECONDARY_CASES
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
            "\n],\n\"outside_arms\":{{\"reference_work_units\":{},\"checker_work_units\":{}}},\n",
            "\"arms\":{{\"baseline\":\"the deterministic unit-propagation/DPLL solver on the original formula\",",
            "\"per_literal\":\"the unchanged fragment-interface-v1 arm: fragment graph, components, fragment certificate, one depth-first search per literal with two freshly initialized arrays, copy and unit appends in literal order, then the residual solver\",",
            "\"settled\":\"identical to per_literal in every phase except extraction, which settles every vertex with one stamped state, inheritance from a bad successor, and searches that settle every vertex an exhausted search reached\"}},\n",
            "\"comparison\":{{\"totals\":\"total online work compared only on cases where both arms completed; every completed decision agreed with the reference\",",
            "\"extraction\":\"the extraction phase alone, compared only where both arms completed and the fragment certificate was SAT so both extractions ran\"}},\n",
            "\"encoding\":{{\"vertex_labels\":\"usize 2*(v-1) and 2*(v-1)+1, at most 24 vertices; dual is x XOR 1\",",
            "\"clause_indices\":\"usize positions in the original input, ternary clauses included\",",
            "\"literals\":\"i32 signed DIMACS-style\",",
            "\"counters\":\"u64 checked increments; identical in debug and release\",",
            "\"enumeration_ratio\":\"reference work units divided by arm work units, nearest thousandth\"}},\n",
            "\"primary\":{{"
        ),
        result.primary.summary.reference_work_units + result.secondary.summary.reference_work_units,
        result.primary.summary.checker_work_units + result.secondary.summary.checker_work_units
    )
    .expect("string write");
    section_json(&mut output, &result.primary, "random_mixed_by_binary_count");
    output.push_str("},\n\"secondary\":{\"arm2_reproduces_v1\":true,\n");
    section_json(
        &mut output,
        &result.secondary,
        "v1_random_mixed_by_binary_count",
    );
    output.push_str("}\n}\n");
    output
}

pub fn summary_line(result: &Experiment) -> String {
    let s = &result.primary.summary;
    let t = &result.secondary.summary;
    let line = |c: &Comparison| {
        format!(
            "{} both-complete: {} better, {} tied, {} worse",
            c.both_complete, c.left_better, c.tied, c.left_worse
        )
    };
    format!(
        concat!(
            "Primary: {} cases in {} families ({} sat, {} unsat). Reference {}; checker {}. ",
            "Baseline {} ({} unknown); per-literal {} (extraction {}, {} searches, {} units); ",
            "settled {} (extraction {}, {} searches, {} inherited, {} settled good, {} units). ",
            "Settled vs baseline on {}; settled vs per-literal on {}; per-literal vs baseline on {}; ",
            "extraction alone on {} compared: {} better, {} tied, {} worse. ",
            "Secondary: {} v1 cases; per-literal {} vs settled {} (extraction {} vs {}); settled vs per-literal on {}."
        ),
        s.cases,
        result.primary.families.len(),
        s.reference_sat,
        s.reference_unsat,
        s.reference_work_units,
        s.checker_work_units,
        s.baseline.work_units,
        s.baseline.unknown,
        s.per_literal.total_work_units,
        s.per_literal.extraction_work_units,
        s.per_literal_searches,
        s.per_literal.derived_units,
        s.settled.total_work_units,
        s.settled.extraction_work_units,
        s.settled_stats.searches,
        s.settled_stats.inherited,
        s.settled_stats.settled_good,
        s.settled.derived_units,
        line(&s.settled_vs_baseline),
        line(&s.settled_vs_per_literal),
        line(&s.per_literal_vs_baseline),
        s.extraction.compared,
        s.extraction.settled_better,
        s.extraction.tied,
        s.extraction.settled_worse,
        t.cases,
        t.per_literal.total_work_units,
        t.settled.total_work_units,
        t.per_literal.extraction_work_units,
        t.settled.extraction_work_units,
        line(&t.settled_vs_per_literal)
    )
}
