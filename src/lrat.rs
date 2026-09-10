//! Checker for textual LRAT proofs as CaDiCaL emits them with
//! `--lrat --binary=false`.
//!
//! Original clauses are numbered `1..=m` in DIMACS order. A proof line is
//! either an addition `id l1 .. lk 0 h1 .. hj 0` or a deletion
//! `id d c1 .. cj 0`. An addition is verified by reverse unit propagation:
//! under the assignment that falsifies every literal of the new clause, every
//! hinted clause visited in the given order must be unit (and is propagated)
//! or falsified (a conflict, which completes the check and must be the last
//! hint). Negative hints mark RAT steps, which this checker does not support.
//!
//! Every deletion line CaDiCaL writes reuses the id of the most recent lemma,
//! so ids are only required to increase across additions. Checker work is
//! charged on a [`Meter`] with an effectively unbounded budget; it is research
//! oracle cost outside any experimental arm and is not a proved bit-cost model.

use crate::transfer::{Event, Failure, Meter, Work};
use crate::Cnf;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LratReport {
    /// Additions verified.
    pub lemmas: u64,
    /// Clause ids removed from the active set (one deletion line may list many).
    pub deletions: u64,
    /// True exactly when an addition of the empty clause was verified.
    pub proves_unsat: bool,
    pub work: Work,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LratError {
    /// A malformed proof line; `line` is 1-based.
    Parse { line: u64 },
    /// Hint `hint` of lemma `lemma` is neither unit nor falsified when reached.
    HintNotUnit { lemma: u64, hint: u64 },
    /// Hint `hint` of lemma `lemma` is the conflict but further hints follow;
    /// a strict proof names the conflict last.
    ConflictNotLast { lemma: u64, hint: u64 },
    /// A hint or deletion names a clause that is not active.
    UnknownClause { id: u64 },
    /// An addition id does not exceed every earlier addition id.
    IdNotIncreasing,
    /// A literal is zero, `i32::MIN`, or names a variable above `n`.
    LiteralOutOfRange,
    /// RAT hints, absurd ids, or counter overflow.
    Unsupported,
}

impl From<Failure> for LratError {
    fn from(_: Failure) -> Self {
        LratError::Unsupported
    }
}

/// Active clauses indexed by id. Literals live in one arena so deletion never
/// frees memory; the arena is bounded by the input plus the proof text.
struct Table {
    arena: Vec<i32>,
    spans: Vec<Option<(usize, usize)>>,
    id_bound: u64,
}

impl Table {
    fn slot(&self, id: u64) -> Result<usize, LratError> {
        if id == 0 || id > self.id_bound {
            return Err(LratError::Unsupported);
        }
        usize::try_from(id).map_err(|_| LratError::Unsupported)
    }
    fn insert(&mut self, id: u64, literals: &[i32]) -> Result<(), LratError> {
        let slot = self.slot(id)?;
        if slot >= self.spans.len() {
            let target = slot.checked_add(1).ok_or(LratError::Unsupported)?;
            self.spans.resize(target, None);
        }
        let start = self.arena.len();
        self.arena.extend_from_slice(literals);
        self.spans[slot] = Some((start, literals.len()));
        Ok(())
    }
    fn get(&self, id: u64) -> Result<&[i32], LratError> {
        let slot = self.slot(id)?;
        match self.spans.get(slot).copied().flatten() {
            Some((start, len)) => Ok(&self.arena[start..start + len]),
            None => Err(LratError::UnknownClause { id }),
        }
    }
    fn delete(&mut self, id: u64) -> Result<(), LratError> {
        let slot = self.slot(id)?;
        match self.spans.get_mut(slot) {
            Some(entry @ Some(_)) => {
                *entry = None;
                Ok(())
            }
            _ => Err(LratError::UnknownClause { id }),
        }
    }
}

/// Stamped partial assignment. A variable is assigned in the current lemma
/// exactly when its stamp equals the lemma stamp, so a reset is one increment.
struct Assignment {
    stamp: Vec<u64>,
    positive: Vec<bool>,
    current: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Value {
    True,
    False,
    Unassigned,
}

impl Assignment {
    fn new(variables: usize) -> Self {
        Self {
            stamp: vec![0; variables + 1],
            positive: vec![false; variables + 1],
            current: 0,
        }
    }
    fn reset(&mut self) -> Result<(), LratError> {
        self.current = self.current.checked_add(1).ok_or(LratError::Unsupported)?;
        Ok(())
    }
    fn value(&self, lit: i32) -> Value {
        let var = lit.unsigned_abs() as usize;
        if self.stamp[var] != self.current {
            Value::Unassigned
        } else if self.positive[var] == (lit > 0) {
            Value::True
        } else {
            Value::False
        }
    }
    fn assign(&mut self, lit: i32) {
        let var = lit.unsigned_abs() as usize;
        self.stamp[var] = self.current;
        self.positive[var] = lit > 0;
    }
}

fn check_literal(lit: i32, n: u32) -> Result<(), LratError> {
    if lit == 0 || lit == i32::MIN || lit.unsigned_abs() > n {
        return Err(LratError::LiteralOutOfRange);
    }
    Ok(())
}

/// Reads one optionally signed decimal integer, skipping leading spaces and
/// tabs. Returns `Ok(None)` at end of line.
fn next_number(bytes: &[u8], at: &mut usize) -> Result<Option<i64>, ()> {
    while *at < bytes.len() && matches!(bytes[*at], b' ' | b'\t' | b'\r') {
        *at += 1;
    }
    if *at >= bytes.len() {
        return Ok(None);
    }
    let negative = bytes[*at] == b'-';
    if negative {
        *at += 1;
    }
    let start = *at;
    let mut magnitude: i64 = 0;
    while *at < bytes.len() && bytes[*at].is_ascii_digit() {
        magnitude = magnitude
            .checked_mul(10)
            .and_then(|m| m.checked_add(i64::from(bytes[*at] - b'0')))
            .ok_or(())?;
        *at += 1;
    }
    if *at == start {
        return Err(());
    }
    if *at < bytes.len() && !matches!(bytes[*at], b' ' | b'\t' | b'\r') {
        return Err(());
    }
    Ok(Some(if negative { -magnitude } else { magnitude }))
}

/// Reads numbers up to and including a `0` terminator into `out`.
fn read_until_zero(bytes: &[u8], at: &mut usize, out: &mut Vec<i64>) -> Result<(), ()> {
    loop {
        match next_number(bytes, at)? {
            Some(0) => return Ok(()),
            Some(x) => out.push(x),
            None => return Err(()),
        }
    }
}

fn propagate(
    lemma: u64,
    literals: &[i32],
    hints: &[i64],
    table: &Table,
    assignment: &mut Assignment,
    meter: &mut Meter,
) -> Result<(), LratError> {
    meter.tick(Event::Formula)?;
    assignment.reset()?;
    for &lit in literals {
        meter.tick(Event::LiteralRead)?;
        // A lemma containing both x and -x is a tautology and sound however
        // this check ends; the last write simply wins.
        meter.tick(Event::LiteralWrite)?;
        let negated = lit.checked_neg().ok_or(LratError::LiteralOutOfRange)?;
        assignment.assign(negated);
    }
    for (index, &hint) in hints.iter().enumerate() {
        let id = u64::try_from(hint).map_err(|_| LratError::Unsupported)?;
        meter.tick(Event::ClauseRead)?;
        let clause = table.get(id)?;
        let mut unit: Option<i32> = None;
        let mut open = 0_u32;
        for &lit in clause {
            meter.tick(Event::LiteralRead)?;
            match assignment.value(lit) {
                Value::False => {}
                Value::True => return Err(LratError::HintNotUnit { lemma, hint: id }),
                Value::Unassigned => {
                    open = open.checked_add(1).ok_or(LratError::Unsupported)?;
                    if open > 1 {
                        return Err(LratError::HintNotUnit { lemma, hint: id });
                    }
                    unit = Some(lit);
                }
            }
        }
        match unit {
            None if index + 1 == hints.len() => return Ok(()),
            None => return Err(LratError::ConflictNotLast { lemma, hint: id }),
            Some(lit) => {
                meter.tick(Event::LiteralWrite)?;
                assignment.assign(lit);
            }
        }
    }
    Err(LratError::HintNotUnit {
        lemma,
        hint: u64::try_from(hints.last().copied().unwrap_or(0)).unwrap_or(0),
    })
}

/// Checks `proof` against `input` over variables `1..=n`.
///
/// Returns `proves_unsat == true` exactly when a verified addition is the
/// empty clause. A proof without the empty clause is still checked lemma by
/// lemma and reports `proves_unsat == false`.
pub fn check(input: &Cnf, n: u32, proof: &str) -> Result<LratReport, LratError> {
    let mut meter = Meter::new(u64::MAX);
    let original = u64::try_from(input.len()).map_err(|_| LratError::Unsupported)?;
    let id_bound = original
        .checked_add(u64::try_from(proof.len()).map_err(|_| LratError::Unsupported)?)
        .ok_or(LratError::Unsupported)?;
    let mut table = Table {
        arena: Vec::new(),
        spans: Vec::new(),
        id_bound,
    };
    for (index, clause) in input.iter().enumerate() {
        for &lit in clause {
            check_literal(lit, n)?;
        }
        let id = u64::try_from(index)
            .ok()
            .and_then(|i| i.checked_add(1))
            .ok_or(LratError::Unsupported)?;
        table.insert(id, clause)?;
    }
    let mut assignment = Assignment::new(usize::try_from(n).map_err(|_| LratError::Unsupported)?);
    let mut report = LratReport::default();
    let mut last_id = original;
    let mut numbers: Vec<i64> = Vec::new();
    let mut literals: Vec<i32> = Vec::new();
    let mut line_no: u64 = 0;
    for line in proof.split('\n') {
        line_no = line_no.checked_add(1).ok_or(LratError::Unsupported)?;
        let bytes = line.as_bytes();
        let parse = LratError::Parse { line: line_no };
        let mut at = 0;
        let id = match next_number(bytes, &mut at).map_err(|_| parse.clone())? {
            None => continue,
            Some(id) => u64::try_from(id).map_err(|_| parse.clone())?,
        };
        while at < bytes.len() && matches!(bytes[at], b' ' | b'\t') {
            at += 1;
        }
        if bytes.get(at) == Some(&b'd') {
            at += 1;
            numbers.clear();
            read_until_zero(bytes, &mut at, &mut numbers).map_err(|_| parse.clone())?;
            if next_number(bytes, &mut at)
                .map_err(|_| parse.clone())?
                .is_some()
            {
                return Err(parse);
            }
            for &target in &numbers {
                let target = u64::try_from(target).map_err(|_| parse.clone())?;
                table.delete(target)?;
                report.deletions = report
                    .deletions
                    .checked_add(1)
                    .ok_or(LratError::Unsupported)?;
            }
            continue;
        }
        numbers.clear();
        read_until_zero(bytes, &mut at, &mut numbers).map_err(|_| parse.clone())?;
        literals.clear();
        for &lit in &numbers {
            let lit = i32::try_from(lit).map_err(|_| LratError::LiteralOutOfRange)?;
            check_literal(lit, n)?;
            literals.push(lit);
        }
        numbers.clear();
        read_until_zero(bytes, &mut at, &mut numbers).map_err(|_| parse.clone())?;
        if next_number(bytes, &mut at)
            .map_err(|_| parse.clone())?
            .is_some()
        {
            return Err(parse);
        }
        if id <= last_id {
            return Err(LratError::IdNotIncreasing);
        }
        if numbers.iter().any(|&hint| hint < 0) {
            return Err(LratError::Unsupported);
        }
        propagate(id, &literals, &numbers, &table, &mut assignment, &mut meter)?;
        table.insert(id, &literals)?;
        last_id = id;
        report.lemmas = report.lemmas.checked_add(1).ok_or(LratError::Unsupported)?;
        if literals.is_empty() {
            report.proves_unsat = true;
        }
    }
    report.work = meter.work;
    Ok(report)
}
