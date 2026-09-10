//! A deliberately small typed language for local CNF transformations.
//!
//! Evaluation work units count AST visits, clause reads/writes, and literal
//! reads/writes. They are implementation accounting, not a proved bit-cost model.
//! SAT truth-table enumeration is a research oracle and is not in this language.

use std::fmt;

pub mod implication;
pub mod indexed;
pub mod transfer;

pub type Cnf = Vec<Vec<i32>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiteralExpr {
    Unit,
    Neg(Box<LiteralExpr>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormulaExpr {
    Input,
    WithUnit(Box<FormulaExpr>, Box<FormulaExpr>),
    Rewrite {
        source: Box<FormulaExpr>,
        literal: LiteralExpr,
        drop_satisfied: bool,
        erase_falsified: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

fn read(tokens: &[String], at: &mut usize, depth: usize) -> Result<SExpr, String> {
    if depth > 64 {
        return Err("syntax nesting exceeds 64".into());
    }
    let token = tokens.get(*at).ok_or("unexpected end of input")?;
    *at += 1;
    match token.as_str() {
        "(" => {
            let mut xs = Vec::new();
            while tokens.get(*at).map(String::as_str) != Some(")") {
                xs.push(read(tokens, at, depth + 1)?);
            }
            *at += 1;
            Ok(SExpr::List(xs))
        }
        ")" => Err("unexpected closing parenthesis".into()),
        _ => Ok(SExpr::Atom(token.clone())),
    }
}

fn atom(x: &SExpr) -> Option<&str> {
    match x {
        SExpr::Atom(s) => Some(s),
        _ => None,
    }
}

fn literal(x: &SExpr, bound: bool) -> Result<LiteralExpr, String> {
    match x {
        SExpr::Atom(s) if s == "unit" && bound => Ok(LiteralExpr::Unit),
        SExpr::List(xs) if xs.len() == 2 && atom(&xs[0]) == Some("neg") => {
            Ok(LiteralExpr::Neg(Box::new(literal(&xs[1], bound)?)))
        }
        _ => Err("expected Literal; unit must be bound by with-unit".into()),
    }
}

fn boolean(x: &SExpr) -> Result<bool, String> {
    match atom(x) {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        _ => Err("expected Boolean constant".into()),
    }
}

fn formula(x: &SExpr, bound: bool) -> Result<FormulaExpr, String> {
    match x {
        SExpr::Atom(s) if s == "input" => Ok(FormulaExpr::Input),
        SExpr::List(xs) if xs.len() == 3 && atom(&xs[0]) == Some("with-unit") => {
            Ok(FormulaExpr::WithUnit(
                Box::new(formula(&xs[1], bound)?),
                Box::new(formula(&xs[2], true)?),
            ))
        }
        SExpr::List(xs) if xs.len() == 5 && atom(&xs[0]) == Some("rewrite") => {
            Ok(FormulaExpr::Rewrite {
                source: Box::new(formula(&xs[1], bound)?),
                literal: literal(&xs[2], bound)?,
                drop_satisfied: boolean(&xs[3])?,
                erase_falsified: boolean(&xs[4])?,
            })
        }
        _ => Err("expected Formula expression with a known operator and arity".into()),
    }
}

pub fn parse(source: &str) -> Result<FormulaExpr, String> {
    if source.len() > 16_384 {
        return Err("program exceeds 16384 bytes".into());
    }
    let spaced = source.replace('(', " ( ").replace(')', " ) ");
    let tokens: Vec<String> = spaced.split_whitespace().map(str::to_owned).collect();
    let mut at = 0;
    let tree = read(&tokens, &mut at, 0)?;
    if at != tokens.len() {
        return Err("trailing expression".into());
    }
    formula(&tree, false)
}

impl fmt::Display for LiteralExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => write!(f, "unit"),
            Self::Neg(x) => write!(f, "(neg {x})"),
        }
    }
}

impl fmt::Display for FormulaExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input => write!(f, "input"),
            Self::WithUnit(source, body) => write!(f, "(with-unit {source} {body})"),
            Self::Rewrite {
                source,
                literal,
                drop_satisfied,
                erase_falsified,
            } => {
                write!(
                    f,
                    "(rewrite {source} {literal} {drop_satisfied} {erase_falsified})"
                )
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cost {
    pub work_units: u64,
    pub ast_visits: u64,
    pub clause_reads: u64,
    pub clause_writes: u64,
    pub literal_reads: u64,
    pub literal_writes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvalError {
    UnknownBudget,
    CounterOverflow,
    InvalidLiteral(i32),
    UnboundUnit,
}

struct Meter {
    limit: u64,
    cost: Cost,
}

#[derive(Clone, Copy)]
enum Event {
    Ast,
    ClauseRead,
    ClauseWrite,
    LiteralRead,
    LiteralWrite,
}

impl Meter {
    fn step(&mut self, event: Event) -> Result<(), EvalError> {
        let next = self
            .cost
            .work_units
            .checked_add(1)
            .ok_or(EvalError::CounterOverflow)?;
        if next > self.limit {
            return Err(EvalError::UnknownBudget);
        }
        let field = match event {
            Event::Ast => &mut self.cost.ast_visits,
            Event::ClauseRead => &mut self.cost.clause_reads,
            Event::ClauseWrite => &mut self.cost.clause_writes,
            Event::LiteralRead => &mut self.cost.literal_reads,
            Event::LiteralWrite => &mut self.cost.literal_writes,
        };
        *field = field.checked_add(1).ok_or(EvalError::CounterOverflow)?;
        self.cost.work_units = next;
        Ok(())
    }
    fn copy(&mut self, input: &Cnf) -> Result<Cnf, EvalError> {
        let mut result = Vec::new();
        for clause in input {
            self.step(Event::ClauseRead)?;
            self.step(Event::ClauseWrite)?;
            let mut new_clause = Vec::new();
            for &lit in clause {
                self.step(Event::LiteralRead)?;
                self.step(Event::LiteralWrite)?;
                new_clause.push(lit);
            }
            result.push(new_clause);
        }
        Ok(result)
    }
}

fn eval_literal(x: &LiteralExpr, unit: Option<i32>, meter: &mut Meter) -> Result<i32, EvalError> {
    meter.step(Event::Ast)?;
    match x {
        LiteralExpr::Unit => unit.ok_or(EvalError::UnboundUnit),
        LiteralExpr::Neg(inner) => eval_literal(inner, unit, meter)?
            .checked_neg()
            .ok_or(EvalError::CounterOverflow),
    }
}

fn eval(
    x: &FormulaExpr,
    input: &Cnf,
    unit: Option<i32>,
    meter: &mut Meter,
) -> Result<Cnf, EvalError> {
    meter.step(Event::Ast)?;
    match x {
        FormulaExpr::Input => meter.copy(input),
        FormulaExpr::WithUnit(source, body) => {
            let source = eval(source, input, unit, meter)?;
            for clause in &source {
                meter.step(Event::ClauseRead)?;
                if clause.len() == 1 {
                    meter.step(Event::LiteralRead)?;
                    // The body sees the evaluated source as input and its first unit.
                    return eval(body, &source, Some(clause[0]), meter);
                }
            }
            Ok(source)
        }
        FormulaExpr::Rewrite {
            source,
            literal,
            drop_satisfied,
            erase_falsified,
        } => {
            let source = eval(source, input, unit, meter)?;
            let chosen = eval_literal(literal, unit, meter)?;
            let opposite = chosen.checked_neg().ok_or(EvalError::CounterOverflow)?;
            let mut result = Vec::new();
            for clause in source {
                meter.step(Event::ClauseRead)?;
                let mut satisfied = false;
                if *drop_satisfied {
                    for &lit in &clause {
                        meter.step(Event::LiteralRead)?;
                        if lit == chosen {
                            satisfied = true;
                            break;
                        }
                    }
                }
                if satisfied {
                    continue;
                }
                meter.step(Event::ClauseWrite)?;
                let mut new_clause = Vec::new();
                for lit in clause {
                    meter.step(Event::LiteralRead)?;
                    if !*erase_falsified || lit != opposite {
                        meter.step(Event::LiteralWrite)?;
                        new_clause.push(lit);
                    }
                }
                result.push(new_clause);
            }
            Ok(result)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evaluation {
    pub result: Result<Cnf, EvalError>,
    pub cost: Cost,
}

pub fn evaluate(program: &FormulaExpr, input: &Cnf, fuel: u64) -> Evaluation {
    let mut meter = Meter {
        limit: fuel,
        cost: Cost::default(),
    };
    let result = (|| {
        for clause in input {
            meter.step(Event::ClauseRead)?;
            for &lit in clause {
                meter.step(Event::LiteralRead)?;
                if lit == 0 || lit == i32::MIN {
                    return Err(EvalError::InvalidLiteral(lit));
                }
            }
        }
        eval(program, input, None, &mut meter)
    })();
    Evaluation {
        result,
        cost: meter.cost,
    }
}

pub const UNIT_RULE: &str = "(with-unit input (rewrite input unit true true))";

pub fn grammar() -> Vec<String> {
    let mut result = Vec::new();
    for lit in ["unit", "(neg unit)"] {
        for drop in [false, true] {
            for erase in [false, true] {
                result.push(format!(
                    "(with-unit input (rewrite input {lit} {drop} {erase}))"
                ));
            }
        }
    }
    result
}

/// Independent, bounded research oracle. Not available to FormulaExpr.
pub fn truth_table_sat(input: &Cnf, variables: u32) -> Result<bool, String> {
    if variables > 20 {
        return Err("truth-table oracle limited to 20 variables".into());
    }
    for clause in input {
        for &lit in clause {
            if lit == 0 || lit == i32::MIN || lit.unsigned_abs() > variables {
                return Err("literal outside oracle domain".into());
            }
        }
    }
    for assignment in 0_u64..(1_u64 << variables) {
        let satisfied = input.iter().all(|clause| {
            clause.iter().any(|&lit| {
                let value = assignment & (1_u64 << (lit.unsigned_abs() - 1)) != 0;
                value == (lit > 0)
            })
        });
        if satisfied {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn two_variable_formula(mask: u32) -> Cnf {
    let literals = [1, -1, 2, -2];
    (0..16)
        .filter(|clause| mask & (1 << clause) != 0)
        .map(|clause| {
            literals
                .iter()
                .enumerate()
                .filter_map(|(bit, &lit)| (clause & (1 << bit) != 0).then_some(lit))
                .collect()
        })
        .collect()
}

pub fn eliminates_first_unit(input: &Cnf, output: &Cnf) -> bool {
    match input.iter().find(|clause| clause.len() == 1) {
        None => output == input,
        Some(clause) => !output
            .iter()
            .flatten()
            .any(|&lit| lit == clause[0] || lit == -clause[0]),
    }
}

#[derive(Clone, Debug)]
pub struct CandidateResult {
    pub program: String,
    pub preservation_passes: u64,
    pub progress_passes: u64,
    pub checked_formulas: u64,
    pub first_counterexample: Option<(Cnf, Cnf, bool, bool)>,
    pub work_units: u64,
}

pub fn calibration() -> Result<Vec<CandidateResult>, String> {
    grammar()
        .into_iter()
        .map(|program| {
            let ast = parse(&program)?;
            let mut record = CandidateResult {
                program,
                preservation_passes: 0,
                progress_passes: 0,
                checked_formulas: 0,
                first_counterexample: None,
                work_units: 0,
            };
            for mask in 0..65_536 {
                let input = two_variable_formula(mask);
                let observation = evaluate(&ast, &input, 100_000);
                let output = observation
                    .result
                    .map_err(|e| format!("unexpected evaluation failure: {e:?}"))?;
                record.work_units = record
                    .work_units
                    .checked_add(observation.cost.work_units)
                    .ok_or("aggregate cost overflow")?;
                let input_sat = truth_table_sat(&input, 2)?;
                let output_sat = truth_table_sat(&output, 2)?;
                record.checked_formulas += 1;
                if input_sat == output_sat {
                    record.preservation_passes += 1;
                } else if record.first_counterexample.is_none() {
                    record.first_counterexample =
                        Some((input.clone(), output.clone(), input_sat, output_sat));
                }
                if eliminates_first_unit(&input, &output) {
                    record.progress_passes += 1;
                }
            }
            Ok(record)
        })
        .collect()
}

pub fn false_completeness_example() -> Cnf {
    vec![vec![1, 2], vec![1, -2], vec![-1, 2], vec![-1, -2]]
}

pub fn result_json(records: &[CandidateResult]) -> Result<String, String> {
    let expected = grammar();
    if records.len() != expected.len()
        || expected.iter().any(|program| {
            records
                .iter()
                .filter(|record| &record.program == program)
                .count()
                != 1
        })
        || records.iter().any(|record| {
            record.checked_formulas != 65_536
                || record.preservation_passes > record.checked_formulas
                || record.progress_passes > record.checked_formulas
        })
    {
        return Err("expected exact calibration grammar and coverage counts".into());
    }
    let survivors: Vec<_> = records
        .iter()
        .filter(|r| r.preservation_passes == 65_536 && r.progress_passes == 65_536)
        .collect();
    if survivors.len() != 1 || survivors[0].program != UNIT_RULE {
        return Err("expected one known calibration survivor".into());
    }
    let example = false_completeness_example();
    let output = evaluate(&parse(UNIT_RULE)?, &example, 100_000)
        .result
        .map_err(|e| format!("{e:?}"))?;
    let sat = truth_table_sat(&example, 2)?;
    let has_unit = example.iter().any(|c| c.len() == 1);
    let unchanged = output == example;
    let rejected = !sat && !has_unit && unchanged;
    if !rejected {
        return Err("false-completeness control failed".into());
    }
    let candidates: Vec<String> = records.iter().map(|r| {
        let counterexample = match &r.first_counterexample {
            None => "null".into(),
            Some((input, output, input_sat, output_sat)) => format!(
                "{{\"input\":{input:?},\"output\":{output:?},\"input_sat\":{input_sat},\"output_sat\":{output_sat}}}"),
        };
        // Programs contain only this fixed ASCII grammar, so no JSON escaping is needed.
        format!("{{\"program\":\"{}\",\"preservation_passes\":{},\"progress_passes\":{},\"checked_formulas\":{},\"first_counterexample\":{},\"work_units\":{}}}",
            r.program, r.preservation_passes, r.progress_passes, r.checked_formulas, counterexample, r.work_units)
    }).collect();
    Ok(format!(concat!(
        "{{\n  \"schema_version\":1,\n  \"experiment\":\"unit-propagation-calibration-v1\",\n",
        "  \"status\":\"bounded-tested\",\n",
        "  \"grammar_size\":8,\n  \"corpus\":{{\"variables\":2,\"canonical_clauses\":16,\"formulas\":65536,\"assignments_per_formula\":4}},\n",
        "  \"candidates\":[{}],\n  \"selected_program\":\"{}\",\n",
        "  \"false_completeness_claim\":{{\"input\":{:?},\"satisfiable\":{},\"has_unit\":{},\"unchanged\":{},\"rejected\":{}}},\n",
        "  \"proof_status\":\"informal-general-argument-not-kernel-checked\",\n",
        "  \"limitations\":[\"Finite two-variable canonical corpus only\",\"Grammar has eight hand-scoped candidates\",\"Work units are not a proved bit-complexity model\",\"No P versus NP result\",\"No evolutionary or agent-efficiency comparison yet\"]\n}}\n"),
        candidates.join(",\n    "), survivors[0].program, example, sat, has_unit, unchanged, rejected))
}
