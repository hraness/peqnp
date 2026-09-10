//! Declared, certificate-checked reference oracle for formulas above the
//! truth-table bound.
//!
//! Up to [`TRUTH_TABLE_MAX_VARIABLES`] the reference is the truth table in
//! `implication`. Above it, a formula with an empty clause or with no clauses
//! is decided by definition, and every other formula is looked up in an
//! oracle-answer file produced once by the pinned CaDiCaL binary under a
//! conflict limit. A `sat` answer is accepted only after its model satisfies
//! the raw clauses ([`check_model`]); an `unsat` answer only after its LRAT
//! proof passes the std-only checker ([`check_lrat`]); a conflict-limit or
//! proof-over-cap answer is `unknown`. Replay never spawns a solver. Nothing
//! here reaches an arm: arms keep their `(input, n, budget)` signatures and
//! the driver labels a case only after every arm has returned for it.
//!
//! Checker work is reported outside every arm and is an operational
//! measurement, not a machine-model bound. No artifact produced through this
//! module carries seconds, solver conflict counts, or floating point.

use crate::implication::{self, Reference};
use crate::lrat::{self, LratError};
use crate::sha256::sha256_hex;
use crate::transfer::{Event, Failure, Meter, Work};
use crate::Cnf;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The pinned solver: name, the Homebrew link path, `--version` text, and
/// the SHA-256 of the link target. Re-pinning is a reference-oracle change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolverSpec {
    pub name: &'static str,
    pub path: &'static str,
    pub version: &'static str,
    pub sha256: &'static str,
}

pub const CADICAL: SolverSpec = SolverSpec {
    name: "cadical",
    path: "/opt/homebrew/bin/cadical",
    version: "3.0.1",
    sha256: "601c9fa8ba5d09fd81bb00c89b3e54832f138bccc3422bd8652e8cda4d74d1fa",
};
/// Fixed arguments; the invocation appends `-c <limit> <input.cnf> <proof.lrat>`.
pub const FIXED_ARGUMENTS: [&str; 3] = ["-q", "--lrat", "--binary=false"];
pub const TRUTH_TABLE_MAX_VARIABLES: u32 = 12;
/// LRAT text per entry beyond which the answer is unknown, never invalid.
pub const PROOF_TEXT_CAP_BYTES: u64 = 2_097_152;
/// Bytes per oracle-answer file.
pub const ANSWER_FILE_CAP_BYTES: u64 = 16_777_216;
pub const SCHEMA_VERSION: u64 = 1;
/// The reference-model string every protocol above the truth-table bound declares.
pub const REFERENCE_MODEL: &str =
    "truth-table<=12;definition;oracle:cadical-3.0.1+model-check+lrat-check";

/// The only oracle limit: a conflict count named by the protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub conflicts: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub path: String,
    pub version: String,
    pub sha256: String,
}

impl Identity {
    pub fn pinned() -> Self {
        Self {
            name: CADICAL.name.into(),
            path: CADICAL.path.into(),
            version: CADICAL.version.into(),
            sha256: CADICAL.sha256.into(),
        }
    }
    /// The preflight and answer-file form: name, version, path, digest.
    pub fn json(&self) -> String {
        format!(
            "{{\"name\":\"{}\",\"version\":\"{}\",\"path\":\"{}\",\"sha256\":\"{}\"}}",
            self.name, self.version, self.path, self.sha256
        )
    }
    /// The artifact form carries no path: the link path is host convention.
    pub fn artifact_json(&self) -> String {
        format!(
            "{{\"name\":\"{}\",\"version\":\"{}\",\"sha256\":\"{}\"}}",
            self.name, self.version, self.sha256
        )
    }
}

/// An exact rational plus its thousandths rendering by integer arithmetic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RatioPair {
    pub numerator: u64,
    pub denominator: u64,
    pub thousandths: String,
}

/// `None` for a zero denominator or on `u64` overflow of `a * 1000 + b / 2`;
/// otherwise round half up to thousandths, exactly as `fragment::ratio` does.
pub fn ratio_pair(numerator: u64, denominator: u64) -> Option<RatioPair> {
    if denominator == 0 {
        return None;
    }
    let scaled = numerator.checked_mul(1000)?.checked_add(denominator / 2)? / denominator;
    Some(RatioPair {
        numerator,
        denominator,
        thousandths: format!("{}.{:03}", scaled / 1000, scaled % 1000),
    })
}

/// `{"numerator":a,"denominator":b,"thousandths":"q.rrr"}` or `null`.
pub fn ratio_json(pair: Option<&RatioPair>) -> String {
    match pair {
        Some(pair) => format!(
            "{{\"numerator\":{},\"denominator\":{},\"thousandths\":\"{}\"}}",
            pair.numerator, pair.denominator, pair.thousandths
        ),
        None => "null".into(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnknownReason {
    ConflictLimit,
    ProofOverCap { bytes: u64 },
}

impl UnknownReason {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ConflictLimit => "conflict-limit",
            Self::ProofOverCap { .. } => "proof-over-cap",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Sat { model: Vec<i32> },
    Unsat { lrat: String },
    Unknown { reason: UnknownReason },
}

impl Answer {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Sat { .. } => "sat",
            Self::Unsat { .. } => "unsat",
            Self::Unknown { .. } => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerEntry {
    pub dimacs_sha256: String,
    pub variables: u32,
    pub clauses: u64,
    pub conflict_limit: u64,
    pub answer: Answer,
}

/// One committed answer file: every entry was produced under the file's
/// single conflict limit by the identity named in its `oracle` block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerFile {
    pub experiment: String,
    pub identity: Identity,
    pub conflict_limit: u64,
    pub entries: BTreeMap<String, AnswerEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelCheck {
    pub valid: bool,
    /// Index of the first unsatisfied clause when `valid` is false.
    pub unsatisfied_clause: Option<u64>,
    pub work: Work,
}

impl ModelCheck {
    pub fn json(&self) -> String {
        format!("{{\"valid\":{},\"work\":{}}}", self.valid, self.work.json())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofCheck {
    pub valid: bool,
    pub lrat_sha256: String,
    pub lrat_bytes: u64,
    pub lemmas: u64,
    pub deletions: u64,
    pub work: Work,
}

impl ProofCheck {
    pub fn json(&self) -> String {
        format!(
            "{{\"valid\":{},\"lrat_sha256\":\"{}\",\"lrat_bytes\":{},\"lemmas\":{},\"deletions\":{},\"work\":{}}}",
            self.valid,
            self.lrat_sha256,
            self.lrat_bytes,
            self.lemmas,
            self.deletions,
            self.work.json()
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Sat {
        model: Vec<i32>,
        model_check: ModelCheck,
    },
    Unsat {
        proof_check: ProofCheck,
    },
    Unknown {
        reason: UnknownReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OracleVerdict {
    pub dimacs_sha256: String,
    pub conflict_limit: u64,
    pub verdict: Verdict,
    pub identity: Identity,
}

impl OracleVerdict {
    pub fn label(&self) -> &'static str {
        match self.verdict {
            Verdict::Sat { .. } => "sat",
            Verdict::Unsat { .. } => "unsat",
            Verdict::Unknown { .. } => "unknown",
        }
    }
    pub fn json(&self) -> String {
        let reason = match &self.verdict {
            Verdict::Unknown { reason } => format!("\"reason\":\"{}\",", reason.name()),
            _ => String::new(),
        };
        let (model, model_check, proof_check) = match &self.verdict {
            Verdict::Sat { model, model_check } => {
                (format!("{model:?}"), model_check.json(), "null".to_owned())
            }
            Verdict::Unsat { proof_check } => ("null".into(), "null".into(), proof_check.json()),
            Verdict::Unknown { .. } => ("null".into(), "null".into(), "null".into()),
        };
        format!(
            "{{\"kind\":\"oracle\",\"label\":\"{}\",{}\"dimacs_sha256\":\"{}\",\"conflict_limit\":{},\"model\":{},\"model_check\":{},\"proof_check\":{},\"oracle\":{}}}",
            self.label(),
            reason,
            self.dimacs_sha256,
            self.conflict_limit,
            model,
            model_check,
            proof_check,
            self.identity.artifact_json()
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Definition {
    EmptyClause {
        clause_index: u64,
        work: Work,
    },
    NoClauses {
        model: Vec<i32>,
        model_check: ModelCheck,
    },
}

/// A case's reference label with its certificate and checker work.
#[derive(Clone, Debug)]
pub enum Label {
    TruthTable(Reference),
    Definition(Definition),
    Oracle(OracleVerdict),
}

impl Label {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::TruthTable(_) => "truth-table",
            Self::Definition(_) => "definition",
            Self::Oracle(_) => "oracle",
        }
    }
    /// `Some(true)` for sat, `Some(false)` for unsat, `None` for unknown.
    pub fn sat(&self) -> Option<bool> {
        match self {
            Self::TruthTable(reference) => Some(reference.model_count > 0),
            Self::Definition(Definition::EmptyClause { .. }) => Some(false),
            Self::Definition(Definition::NoClauses { .. }) => Some(true),
            Self::Oracle(verdict) => match verdict.verdict {
                Verdict::Sat { .. } => Some(true),
                Verdict::Unsat { .. } => Some(false),
                Verdict::Unknown { .. } => None,
            },
        }
    }
    /// The per-case `reference` object of an artifact.
    pub fn json(&self) -> String {
        match self {
            Self::TruthTable(reference) => {
                let backbone = reference
                    .backbone
                    .as_ref()
                    .map_or_else(|| "null".into(), |backbone| format!("{backbone:?}"));
                format!(
                    "{{\"kind\":\"truth-table\",\"model_count\":{},\"sat\":{},\"backbone\":{},\"work\":{}}}",
                    reference.model_count,
                    reference.model_count > 0,
                    backbone,
                    reference.work.json()
                )
            }
            Self::Definition(Definition::EmptyClause { clause_index, work }) => format!(
                "{{\"kind\":\"definition\",\"label\":\"unsat\",\"certificate\":{{\"kind\":\"empty-clause\",\"clause_index\":{}}},\"work\":{}}}",
                clause_index,
                work.json()
            ),
            Self::Definition(Definition::NoClauses { model, model_check }) => format!(
                "{{\"kind\":\"definition\",\"label\":\"sat\",\"model\":{:?},\"model_check\":{}}}",
                model,
                model_check.json()
            ),
            Self::Oracle(verdict) => verdict.json(),
        }
    }
}

#[derive(Debug)]
pub enum OracleError {
    Missing(String),
    DigestMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    VersionMismatch {
        expected: String,
        actual: String,
    },
    Spawn(io::Error),
    Exit {
        code: Option<i32>,
        stderr: String,
    },
    Parse(String),
    ModelInvalid {
        clause_index: u64,
    },
    ProofInvalid {
        line: u64,
        reason: String,
    },
    ProofOverCap {
        bytes: u64,
    },
    AnswerMissing {
        dimacs_sha256: String,
    },
    AnswerDuplicate {
        dimacs_sha256: String,
    },
    AnswerUnused {
        dimacs_sha256: String,
    },
    AnswerIdentity {
        field: String,
        expected: String,
        actual: String,
    },
    AnswerLimit {
        expected: u64,
        actual: u64,
    },
    AnswerFormat(String),
    FileCap {
        bytes: u64,
    },
    Scratch(io::Error),
    /// A live call failed after its scratch files were written; both are
    /// kept at the named paths for inspection.
    Kept {
        cnf: String,
        lrat: String,
        cause: Box<OracleError>,
    },
}

impl fmt::Display for OracleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(what) => write!(f, "missing: {what}"),
            Self::DigestMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "digest mismatch at {path}: expected {expected}, found {actual}"
            ),
            Self::VersionMismatch { expected, actual } => {
                write!(f, "version mismatch: expected {expected}, found {actual}")
            }
            Self::Spawn(error) => write!(f, "cannot run the solver: {error}"),
            Self::Exit { code, stderr } => {
                write!(f, "unexpected solver exit {code:?}: {}", stderr.trim())
            }
            Self::Parse(reason) => write!(f, "solver output not understood: {reason}"),
            Self::ModelInvalid { clause_index } => {
                write!(f, "model leaves clause {clause_index} unsatisfied")
            }
            Self::ProofInvalid { line, reason } => {
                write!(f, "proof invalid at line {line}: {reason}")
            }
            Self::ProofOverCap { bytes } => {
                write!(
                    f,
                    "proof text of {bytes} bytes exceeds {PROOF_TEXT_CAP_BYTES}"
                )
            }
            Self::AnswerMissing { dimacs_sha256 } => {
                write!(
                    f,
                    "no answer for formula {dimacs_sha256}; regenerate the answer file"
                )
            }
            Self::AnswerDuplicate { dimacs_sha256 } => {
                write!(f, "duplicate answer for formula {dimacs_sha256}")
            }
            Self::AnswerUnused { dimacs_sha256 } => {
                write!(f, "answer {dimacs_sha256} is not used by any case")
            }
            Self::AnswerIdentity {
                field,
                expected,
                actual,
            } => write!(
                f,
                "answer file {field}: expected {expected}, found {actual}"
            ),
            Self::AnswerLimit { expected, actual } => {
                write!(
                    f,
                    "conflict limit: protocol names {expected}, answer carries {actual}"
                )
            }
            Self::AnswerFormat(reason) => write!(f, "answer file format: {reason}"),
            Self::FileCap { bytes } => {
                write!(
                    f,
                    "answer file of {bytes} bytes exceeds {ANSWER_FILE_CAP_BYTES}"
                )
            }
            Self::Scratch(error) => write!(f, "scratch file: {error}"),
            Self::Kept { cnf, lrat, cause } => {
                write!(f, "{cause}; scratch files kept at {cnf} and {lrat}")
            }
        }
    }
}

impl From<io::Error> for OracleError {
    fn from(error: io::Error) -> Self {
        Self::Scratch(error)
    }
}

fn oracle_failure(dimacs_sha256: &str, error: &OracleError) -> Failure {
    Failure::Oracle {
        dimacs_sha256: dimacs_sha256.to_owned(),
        reason: error.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Binary identity and live solving
// ---------------------------------------------------------------------------

/// Resolves the pinned link, hashes and version-checks the target, and
/// returns the identity with the resolved path that will be executed.
pub fn identity_resolved() -> Result<(Identity, PathBuf), OracleError> {
    let path = Path::new(CADICAL.path);
    let resolved = fs::canonicalize(path)
        .map_err(|error| OracleError::Missing(format!("{}: {error}", path.display())))?;
    let bytes = fs::read(&resolved)
        .map_err(|error| OracleError::Missing(format!("{}: {error}", resolved.display())))?;
    let actual = sha256_hex(&bytes);
    if actual != CADICAL.sha256 {
        return Err(OracleError::DigestMismatch {
            path: resolved.display().to_string(),
            expected: CADICAL.sha256.into(),
            actual,
        });
    }
    let output = Command::new(&resolved)
        .arg("--version")
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(OracleError::Spawn)?;
    if !output.status.success() {
        return Err(OracleError::Exit {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if version != CADICAL.version {
        return Err(OracleError::VersionMismatch {
            expected: CADICAL.version.into(),
            actual: version,
        });
    }
    Ok((Identity::pinned(), resolved))
}

/// The documented preflight: presence, digest of the link target, `--version`.
pub fn identity() -> Result<Identity, OracleError> {
    identity_resolved().map(|(identity, _)| identity)
}

/// `p cnf n m`, then each clause's literals joined by one space and ` 0`;
/// an empty clause is the line `0`. Original clause ids are `1..=m` in
/// this order, which the LRAT checker relies on.
pub fn write_dimacs(input: &Cnf, n: u32) -> String {
    let mut text = format!("p cnf {} {}\n", n, input.len());
    for clause in input {
        for lit in clause {
            let _ = write!(text, "{lit} ");
        }
        text.push_str("0\n");
    }
    text
}

/// A strict reader for the DIMACS layout `write_dimacs` emits, used by the
/// demo corpus and by tests; comment lines are permitted before the header.
pub fn parse_dimacs(text: &str) -> Result<(Cnf, u32), OracleError> {
    let bad = |reason: &str| OracleError::Parse(format!("dimacs: {reason}"));
    let mut lines = text.lines().filter(|line| !line.starts_with('c'));
    let header = lines.next().ok_or_else(|| bad("missing header"))?;
    let mut fields = header.split(' ');
    if fields.next() != Some("p") || fields.next() != Some("cnf") {
        return Err(bad("header must start with `p cnf`"));
    }
    let variables: u32 = fields
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| bad("variable count"))?;
    let clauses: usize = fields
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| bad("clause count"))?;
    if fields.next().is_some() {
        return Err(bad("trailing header fields"));
    }
    let mut input = Vec::with_capacity(clauses);
    for line in lines {
        let mut clause = Vec::new();
        let mut terminated = false;
        for token in line.split(' ') {
            if terminated {
                return Err(bad("tokens after the clause terminator"));
            }
            let lit: i32 = token.parse().map_err(|_| bad("literal"))?;
            if lit == 0 {
                terminated = true;
            } else {
                if lit == i32::MIN || lit.unsigned_abs() > variables {
                    return Err(bad("literal outside the declared range"));
                }
                clause.push(lit);
            }
        }
        if !terminated {
            return Err(bad("unterminated clause"));
        }
        input.push(clause);
    }
    if input.len() != clauses {
        return Err(bad("clause count does not match the header"));
    }
    Ok((input, variables))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Sat,
    Unsat,
    Unknown,
}

fn strict_int(token: &str) -> Option<i64> {
    let digits = token.strip_prefix('-').unwrap_or(token);
    if digits.is_empty()
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
        || (token.starts_with('-') && digits == "0")
    {
        return None;
    }
    let mut magnitude: i64 = 0;
    for b in digits.bytes() {
        magnitude = magnitude
            .checked_mul(10)?
            .checked_add(i64::from(b - b'0'))?;
    }
    Some(if token.starts_with('-') {
        -magnitude
    } else {
        magnitude
    })
}

/// Reads a `-q` transcript. Exit 10 needs `s SATISFIABLE` and `v` lines
/// assigning every variable `1..=n` exactly once (returned ascending); exit
/// 20 needs `s UNSATISFIABLE`; exit 0 needs no `s` line and is the conflict
/// limit; anything else is `Exit` or `Parse`.
pub fn parse_output(
    stdout: &str,
    exit: Option<i32>,
    n: u32,
) -> Result<(Status, Option<Vec<i32>>), OracleError> {
    let parse = |reason: String| OracleError::Parse(reason);
    let mut status_lines = Vec::new();
    let mut value_tokens = Vec::new();
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("s ") {
            status_lines.push(rest);
        } else if let Some(rest) = line.strip_prefix("v ") {
            value_tokens.extend(rest.split(' ').filter(|t| !t.is_empty()));
        } else if line.is_empty() || line.starts_with("c ") || line == "c" {
            continue;
        } else {
            return Err(parse(format!("unexpected line {line:?}")));
        }
    }
    if status_lines.len() > 1 {
        return Err(parse("more than one s line".into()));
    }
    let status = status_lines.first().copied();
    match exit {
        Some(10) => {
            if status != Some("SATISFIABLE") {
                return Err(parse(format!("exit 10 with s line {status:?}")));
            }
            if value_tokens.is_empty() {
                return Err(parse("exit 10 without v lines".into()));
            }
            let mut assigned = vec![false; usize::try_from(n).map_err(|_| parse("n".into()))? + 1];
            let mut model = vec![0_i32; assigned.len() - 1];
            let mut terminated = false;
            for token in value_tokens {
                if terminated {
                    return Err(parse("v tokens after the terminating 0".into()));
                }
                let value = strict_int(token)
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or_else(|| parse(format!("v token {token:?}")))?;
                if value == 0 {
                    terminated = true;
                    continue;
                }
                let variable = value.unsigned_abs();
                if variable > n {
                    return Err(parse(format!("variable {variable} outside 1..{n}")));
                }
                let slot = variable as usize;
                if assigned[slot] {
                    return Err(parse(format!("variable {variable} assigned twice")));
                }
                assigned[slot] = true;
                model[slot - 1] = value;
            }
            if !terminated {
                return Err(parse("v lines lack the terminating 0".into()));
            }
            if let Some(missing) = assigned.iter().skip(1).position(|set| !set) {
                return Err(parse(format!("variable {} unassigned", missing + 1)));
            }
            Ok((Status::Sat, Some(model)))
        }
        Some(20) => {
            if status != Some("UNSATISFIABLE") {
                return Err(parse(format!("exit 20 with s line {status:?}")));
            }
            if !value_tokens.is_empty() {
                return Err(parse("exit 20 with v lines".into()));
            }
            Ok((Status::Unsat, None))
        }
        Some(0) => {
            if status.is_some() || !value_tokens.is_empty() {
                return Err(parse(format!("exit 0 with s line {status:?}")));
            }
            Ok((Status::Unknown, None))
        }
        code => Err(OracleError::Exit {
            code,
            stderr: String::new(),
        }),
    }
}

fn scratch_dir() -> PathBuf {
    std::env::var_os("PEQNP_ORACLE_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn lrat_charset_ok(text: &str) -> bool {
    text.bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b' ' | b'd' | b'\n' | b'-'))
}

/// Runs the pinned binary once (regeneration only): identity first, DIMACS
/// to a fresh scratch file, `FIXED_ARGUMENTS -c <limit> <cnf> <lrat>` with a
/// cleared environment, then `verify` on the resulting entry so an invalid
/// model or proof fails here. Returns the entry and the wall milliseconds
/// for the timing sidecar. Scratch files are removed on success and kept,
/// named in `Kept`, on any failure after they were written.
pub fn solve(input: &Cnf, n: u32, limits: &Limits) -> Result<(AnswerEntry, u64), OracleError> {
    let (_, resolved) = identity_resolved()?;
    let text = write_dimacs(input, n);
    let dimacs_sha256 = sha256_hex(text.as_bytes());
    let clauses = u64::try_from(input.len()).map_err(|_| OracleError::Parse("m".into()))?;
    let directory = scratch_dir();
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let stem = format!("peqnp-oracle-{}-{}", std::process::id(), sequence);
    let cnf_path = directory.join(format!("{stem}.cnf"));
    let lrat_path = directory.join(format!("{stem}.lrat"));
    File::create_new(&cnf_path)?.write_all(text.as_bytes())?;
    File::create_new(&lrat_path)?;
    let started = Instant::now();
    let output = Command::new(&resolved)
        .args(FIXED_ARGUMENTS)
        .arg("-c")
        .arg(limits.conflicts.to_string())
        .arg(&cnf_path)
        .arg(&lrat_path)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let wall_milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let attempt = (|| -> Result<AnswerEntry, OracleError> {
        let output = output.map_err(OracleError::Spawn)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (status, model) =
            parse_output(&stdout, output.status.code(), n).map_err(|error| match error {
                OracleError::Exit { code, .. } => OracleError::Exit {
                    code,
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                },
                other => other,
            })?;
        let answer = match status {
            Status::Sat => Answer::Sat {
                model: model.ok_or_else(|| OracleError::Parse("no model".into()))?,
            },
            Status::Unknown => Answer::Unknown {
                reason: UnknownReason::ConflictLimit,
            },
            Status::Unsat => {
                let bytes = fs::read(&lrat_path)?;
                let length =
                    u64::try_from(bytes.len()).map_err(|_| OracleError::Parse("len".into()))?;
                if length > PROOF_TEXT_CAP_BYTES {
                    Answer::Unknown {
                        reason: UnknownReason::ProofOverCap { bytes: length },
                    }
                } else {
                    let lrat = String::from_utf8(bytes)
                        .map_err(|_| OracleError::AnswerFormat("proof text is not UTF-8".into()))?;
                    if !lrat_charset_ok(&lrat) {
                        return Err(OracleError::AnswerFormat(
                            "proof text contains a character outside [0-9 d\\n-]".into(),
                        ));
                    }
                    Answer::Unsat { lrat }
                }
            }
        };
        let entry = AnswerEntry {
            dimacs_sha256: dimacs_sha256.clone(),
            variables: n,
            clauses,
            conflict_limit: limits.conflicts,
            answer,
        };
        verify(input, n, &entry, limits)?;
        Ok(entry)
    })();
    match attempt {
        Ok(entry) => {
            fs::remove_file(&cnf_path)?;
            fs::remove_file(&lrat_path)?;
            Ok((entry, wall_milliseconds))
        }
        Err(cause) => Err(OracleError::Kept {
            cnf: cnf_path.display().to_string(),
            lrat: lrat_path.display().to_string(),
            cause: Box::new(cause),
        }),
    }
}

// ---------------------------------------------------------------------------
// Checkers
// ---------------------------------------------------------------------------

fn literal_ok(lit: i32, n: u32) -> bool {
    lit != 0 && lit != i32::MIN && lit.unsigned_abs() <= n
}

/// Charges, on `meter`: one formula check; one container write for the
/// assignment array and one scalar write per variable; one clause read per
/// clause and one scalar read per literal examined until the clause is
/// satisfied. Returns the index of the first unsatisfied clause, or `None`
/// when every clause is satisfied. A model that is not a complete assignment
/// of `1..=n`, each variable once, is `InvalidInput` before any charge.
pub(crate) fn check_model_metered(
    input: &Cnf,
    n: u32,
    model: &[i32],
    meter: &mut Meter,
) -> Result<Option<u64>, Failure> {
    let size = usize::try_from(n).map_err(|_| Failure::InvalidInput)?;
    if model.len() != size {
        return Err(Failure::InvalidInput);
    }
    let mut seen = vec![false; size + 1];
    for &lit in model {
        if !literal_ok(lit, n) || seen[lit.unsigned_abs() as usize] {
            return Err(Failure::InvalidInput);
        }
        seen[lit.unsigned_abs() as usize] = true;
    }
    meter.tick(Event::Formula)?;
    meter.tick(Event::ClauseWrite)?;
    let mut positive = vec![false; size + 1];
    for &lit in model {
        meter.tick(Event::LiteralWrite)?;
        positive[lit.unsigned_abs() as usize] = lit > 0;
    }
    for (index, clause) in input.iter().enumerate() {
        meter.tick(Event::ClauseRead)?;
        let mut satisfied = false;
        for &lit in clause {
            meter.tick(Event::LiteralRead)?;
            if !literal_ok(lit, n) {
                return Err(Failure::InvalidInput);
            }
            if positive[lit.unsigned_abs() as usize] == (lit > 0) {
                satisfied = true;
                break;
            }
        }
        if !satisfied {
            return Ok(Some(
                u64::try_from(index).map_err(|_| Failure::CounterOverflow)?,
            ));
        }
    }
    Ok(None)
}

/// `check_model_metered` on a fresh, effectively unbounded checker meter.
pub fn check_model(input: &Cnf, n: u32, model: &[i32]) -> Result<ModelCheck, Failure> {
    let mut meter = Meter::new(u64::MAX);
    let unsatisfied_clause = check_model_metered(input, n, model, &mut meter)?;
    Ok(ModelCheck {
        valid: unsatisfied_clause.is_none(),
        unsatisfied_clause,
        work: meter.work,
    })
}

/// Strict textual LRAT format, checked before the propagation engine runs:
/// lines split on `\n` with one trailing empty line allowed, tokens split on
/// single spaces, decimal integers only except a lone `d` in second
/// position, lemma ids strictly increasing from `m + 1`, literals within
/// `±1..=±n` with no duplicate and no complementary pair, hints and deleted
/// ids live, no lemma after the empty clause, and the last lemma the empty
/// clause. Returns the line of each lemma id for error reporting.
fn scan_lrat(input: &Cnf, n: u32, text: &str) -> Result<BTreeMap<u64, u64>, OracleError> {
    let invalid = |line: u64, reason: String| OracleError::ProofInvalid { line, reason };
    for (index, line) in text.split('\n').enumerate() {
        if !lrat_charset_ok(line) {
            return Err(invalid(index as u64 + 1, "stray character".into()));
        }
    }
    let m = u64::try_from(input.len()).map_err(|_| invalid(0, "clause count".into()))?;
    for clause in input {
        if clause.iter().any(|&lit| !literal_ok(lit, n)) {
            return Err(invalid(
                0,
                "input literal outside the declared range".into(),
            ));
        }
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let mut live: BTreeSet<u64> = (1..=m).collect();
    let mut lemma_lines = BTreeMap::new();
    let mut last_id = m;
    let mut empty_seen = false;
    let mut marks: Vec<u8> = vec![0; usize::try_from(n).map_err(|_| invalid(0, "n".into()))? + 1];
    for (index, line) in lines.iter().enumerate() {
        let line_no = index as u64 + 1;
        if line.is_empty() {
            return Err(invalid(line_no, "empty line".into()));
        }
        let tokens: Vec<&str> = line.split(' ').collect();
        let number = |token: &str| {
            strict_int(token).ok_or_else(|| {
                invalid(line_no, format!("token {token:?} is not a decimal integer"))
            })
        };
        let id = number(tokens[0])?;
        if id < 1 {
            return Err(invalid(line_no, "line id must be at least 1".into()));
        }
        let id = id as u64;
        if tokens.get(1) == Some(&"d") {
            let rest = &tokens[2..];
            if rest.last() != Some(&"0") {
                return Err(invalid(line_no, "deletion line must end with 0".into()));
            }
            for token in &rest[..rest.len() - 1] {
                let target = number(token)?;
                if target < 1 {
                    return Err(invalid(
                        line_no,
                        format!("deleted id {target} is not a clause id"),
                    ));
                }
                if !live.remove(&(target as u64)) {
                    return Err(invalid(
                        line_no,
                        format!("deletion of clause {target} that is not live"),
                    ));
                }
            }
            continue;
        }
        if empty_seen {
            return Err(invalid(line_no, "lemma after the empty clause".into()));
        }
        if last_id == m && id != m + 1 {
            return Err(invalid(
                line_no,
                format!("first lemma id must be {}", m + 1),
            ));
        }
        if id <= last_id {
            return Err(invalid(line_no, "lemma id does not increase".into()));
        }
        let mut at = 1;
        let mut literals: Vec<i32> = Vec::new();
        loop {
            let token = tokens
                .get(at)
                .ok_or_else(|| invalid(line_no, "lemma lacks its literal terminator".into()))?;
            at += 1;
            let value = number(token)?;
            if value == 0 {
                break;
            }
            let lit = i32::try_from(value)
                .ok()
                .filter(|&lit| literal_ok(lit, n))
                .ok_or_else(|| {
                    invalid(
                        line_no,
                        format!("literal {value} outside the declared range"),
                    )
                })?;
            let slot = lit.unsigned_abs() as usize;
            let mark = if lit > 0 { 1 } else { 2 };
            if marks[slot] == mark {
                for l in &literals {
                    marks[l.unsigned_abs() as usize] = 0;
                }
                return Err(invalid(line_no, format!("duplicate literal {lit}")));
            }
            if marks[slot] != 0 {
                for l in &literals {
                    marks[l.unsigned_abs() as usize] = 0;
                }
                return Err(invalid(
                    line_no,
                    format!("tautological lemma on variable {slot}"),
                ));
            }
            marks[slot] = mark;
            literals.push(lit);
        }
        for l in &literals {
            marks[l.unsigned_abs() as usize] = 0;
        }
        let mut hints = 0_u64;
        loop {
            let token = tokens
                .get(at)
                .ok_or_else(|| invalid(line_no, "lemma lacks its hint terminator".into()))?;
            at += 1;
            let value = number(token)?;
            if value == 0 {
                break;
            }
            if value < 0 {
                return Err(invalid(
                    line_no,
                    format!("negative hint {value} (RAT is not supported)"),
                ));
            }
            if !live.contains(&(value as u64)) {
                return Err(invalid(
                    line_no,
                    format!("hint {value} is not a live clause"),
                ));
            }
            hints += 1;
        }
        if at != tokens.len() {
            return Err(invalid(line_no, "tokens after the hint terminator".into()));
        }
        if hints == 0 {
            return Err(invalid(line_no, "lemma has no hints".into()));
        }
        live.insert(id);
        lemma_lines.insert(id, line_no);
        last_id = id;
        if literals.is_empty() {
            empty_seen = true;
        }
    }
    if !empty_seen {
        return Err(invalid(lines.len() as u64, "no empty clause".into()));
    }
    Ok(lemma_lines)
}

/// Checks `lrat` against the raw clauses on a fresh checker meter. Text over
/// `PROOF_TEXT_CAP_BYTES` is `ProofOverCap` (an unknown, never an invalid
/// proof); every format or propagation failure is `ProofInvalid` with the
/// 1-based line. The propagation engine is `lrat::check`.
pub fn check_lrat(input: &Cnf, n: u32, lrat: &str) -> Result<ProofCheck, OracleError> {
    let lrat_bytes =
        u64::try_from(lrat.len()).map_err(|_| OracleError::ProofOverCap { bytes: u64::MAX })?;
    if lrat_bytes > PROOF_TEXT_CAP_BYTES {
        return Err(OracleError::ProofOverCap { bytes: lrat_bytes });
    }
    let lemma_lines = scan_lrat(input, n, lrat)?;
    let line_of = |lemma: u64| lemma_lines.get(&lemma).copied().unwrap_or(0);
    let report = lrat::check(input, n, lrat).map_err(|error| match error {
        LratError::HintNotUnit { lemma, hint } => OracleError::ProofInvalid {
            line: line_of(lemma),
            reason: format!(
                "hint {hint} is satisfied, leaves two literals open, or ends without a conflict"
            ),
        },
        LratError::ConflictNotLast { lemma, hint } => OracleError::ProofInvalid {
            line: line_of(lemma),
            reason: format!("conflict at hint {hint} before the last hint"),
        },
        other => OracleError::ProofInvalid {
            line: 0,
            reason: format!("{other:?}"),
        },
    })?;
    if !report.proves_unsat {
        return Err(OracleError::ProofInvalid {
            line: 0,
            reason: "no empty clause".into(),
        });
    }
    Ok(ProofCheck {
        valid: true,
        lrat_sha256: sha256_hex(lrat.as_bytes()),
        lrat_bytes,
        lemmas: report.lemmas,
        deletions: report.deletions,
        work: report.work,
    })
}

// ---------------------------------------------------------------------------
// Verification, definition cases, and labelling
// ---------------------------------------------------------------------------

/// Pure re-verification of one answer against the raw clauses: the DIMACS
/// digest, shape, and conflict limit must match, a model must satisfy every
/// clause, a proof must pass `check_lrat`, and an unknown passes through.
/// Never touches the filesystem or the binary.
pub fn verify(
    input: &Cnf,
    n: u32,
    entry: &AnswerEntry,
    limits: &Limits,
) -> Result<OracleVerdict, OracleError> {
    let digest = sha256_hex(write_dimacs(input, n).as_bytes());
    let mismatch = |field: &str, expected: String, actual: String| OracleError::AnswerIdentity {
        field: field.into(),
        expected,
        actual,
    };
    if entry.dimacs_sha256 != digest {
        return Err(mismatch(
            "dimacs_sha256",
            digest,
            entry.dimacs_sha256.clone(),
        ));
    }
    if entry.variables != n {
        return Err(mismatch(
            "variables",
            n.to_string(),
            entry.variables.to_string(),
        ));
    }
    if entry.clauses != input.len() as u64 {
        return Err(mismatch(
            "clauses",
            input.len().to_string(),
            entry.clauses.to_string(),
        ));
    }
    if entry.conflict_limit != limits.conflicts {
        return Err(OracleError::AnswerLimit {
            expected: limits.conflicts,
            actual: entry.conflict_limit,
        });
    }
    let verdict = match &entry.answer {
        Answer::Sat { model } => {
            let model_check = check_model(input, n, model).map_err(|_| {
                OracleError::AnswerFormat("model is not a complete assignment".into())
            })?;
            if let Some(clause_index) = model_check.unsatisfied_clause {
                return Err(OracleError::ModelInvalid { clause_index });
            }
            Verdict::Sat {
                model: model.clone(),
                model_check,
            }
        }
        Answer::Unsat { lrat } => Verdict::Unsat {
            proof_check: check_lrat(input, n, lrat)?,
        },
        Answer::Unknown { reason } => Verdict::Unknown {
            reason: reason.clone(),
        },
    };
    Ok(OracleVerdict {
        dimacs_sha256: digest,
        conflict_limit: entry.conflict_limit,
        verdict,
        identity: Identity::pinned(),
    })
}

/// The two shapes decided without a solver above the truth-table bound,
/// charged on the checker meter: an empty clause (one formula check, one
/// clause read and one scalar read of its length per clause scanned) or no
/// clauses at all (the all-false model, checked by `check_model`).
fn definition(input: &Cnf, n: u32) -> Result<Option<Definition>, Failure> {
    let mut meter = Meter::new(u64::MAX);
    meter.tick(Event::Formula)?;
    for (index, clause) in input.iter().enumerate() {
        meter.tick(Event::ClauseRead)?;
        meter.tick(Event::LiteralRead)?;
        if clause.is_empty() {
            return Ok(Some(Definition::EmptyClause {
                clause_index: u64::try_from(index).map_err(|_| Failure::CounterOverflow)?,
                work: meter.work,
            }));
        }
    }
    if input.is_empty() {
        let last = i32::try_from(n).map_err(|_| Failure::InvalidInput)?;
        let model: Vec<i32> = (1..=last).map(|v| -v).collect();
        let model_check = check_model(input, n, &model)?;
        return Ok(Some(Definition::NoClauses { model, model_check }));
    }
    Ok(None)
}

/// Labels one formula: the truth table at or below 12 variables, then the
/// definition cases, then the answer file (the digest is inserted into
/// `used`; a missing digest stops the run). An unknown verdict is a label,
/// not an error. Called by drivers only after every arm has returned.
pub fn label(
    input: &Cnf,
    n: u32,
    limits: &Limits,
    answers: &AnswerFile,
    used: &mut BTreeSet<String>,
) -> Result<Label, Failure> {
    if n <= TRUTH_TABLE_MAX_VARIABLES {
        return implication::reference_wide(input, n).map(Label::TruthTable);
    }
    if let Some(definition) = definition(input, n)? {
        return Ok(Label::Definition(definition));
    }
    let digest = sha256_hex(write_dimacs(input, n).as_bytes());
    let entry = answers.entries.get(&digest).ok_or_else(|| {
        oracle_failure(
            &digest,
            &OracleError::AnswerMissing {
                dimacs_sha256: digest.clone(),
            },
        )
    })?;
    used.insert(digest.clone());
    verify(input, n, entry, limits)
        .map(Label::Oracle)
        .map_err(|error| oracle_failure(&digest, &error))
}

/// After a corpus: every committed answer must have been used.
pub fn check_used(answers: &AnswerFile, used: &BTreeSet<String>) -> Result<(), OracleError> {
    match answers.entries.keys().find(|key| !used.contains(*key)) {
        Some(key) => Err(OracleError::AnswerUnused {
            dimacs_sha256: key.clone(),
        }),
        None => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Oracle-answer file
// ---------------------------------------------------------------------------

fn experiment_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn digest_ok(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_entry(file: &AnswerFile, key: &str, entry: &AnswerEntry) -> Result<(), OracleError> {
    let format = |reason: &str| OracleError::AnswerFormat(format!("entry {key}: {reason}"));
    if key != entry.dimacs_sha256 || !digest_ok(key) {
        return Err(format("digest key"));
    }
    if entry.conflict_limit != file.conflict_limit {
        return Err(OracleError::AnswerLimit {
            expected: file.conflict_limit,
            actual: entry.conflict_limit,
        });
    }
    match &entry.answer {
        Answer::Sat { model } => {
            let complete = model.len() as u64 == u64::from(entry.variables)
                && model.iter().enumerate().all(|(index, &lit)| {
                    lit != i32::MIN && u64::from(lit.unsigned_abs()) == index as u64 + 1
                });
            if !complete {
                return Err(format("model is not ascending and complete"));
            }
        }
        Answer::Unsat { lrat } => {
            if lrat.len() as u64 > PROOF_TEXT_CAP_BYTES {
                return Err(format("unsat text over the proof cap"));
            }
            if !lrat_charset_ok(lrat) {
                return Err(format("lrat text contains a character outside [0-9 d\\n-]"));
            }
        }
        Answer::Unknown {
            reason: UnknownReason::ProofOverCap { bytes },
        } => {
            if *bytes <= PROOF_TEXT_CAP_BYTES {
                return Err(format("proof-over-cap entry within the cap"));
            }
        }
        Answer::Unknown {
            reason: UnknownReason::ConflictLimit,
        } => {}
    }
    Ok(())
}

fn validate_file(file: &AnswerFile) -> Result<(), OracleError> {
    let pinned = Identity::pinned();
    for (field, expected, actual) in [
        ("name", &pinned.name, &file.identity.name),
        ("version", &pinned.version, &file.identity.version),
        ("path", &pinned.path, &file.identity.path),
        ("sha256", &pinned.sha256, &file.identity.sha256),
    ] {
        if expected != actual {
            return Err(OracleError::AnswerIdentity {
                field: field.into(),
                expected: expected.clone(),
                actual: actual.clone(),
            });
        }
    }
    if !experiment_id_ok(&file.experiment) {
        return Err(OracleError::AnswerFormat("experiment id".into()));
    }
    for (key, entry) in &file.entries {
        validate_entry(file, key, entry)?;
    }
    Ok(())
}

fn int_list(values: &[i32]) -> String {
    let mut text = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        let _ = write!(text, "{value}");
    }
    text.push(']');
    text
}

fn entry_json(entry: &AnswerEntry) -> String {
    let mut text = format!(
        "{{\"dimacs_sha256\":\"{}\",\"variables\":{},\"clauses\":{},\"conflict_limit\":{},\"label\":\"{}\"",
        entry.dimacs_sha256,
        entry.variables,
        entry.clauses,
        entry.conflict_limit,
        entry.answer.label()
    );
    match &entry.answer {
        Answer::Sat { model } => {
            let _ = write!(text, ",\"model\":{}", int_list(model));
        }
        Answer::Unsat { lrat } => {
            let _ = write!(
                text,
                ",\"lrat_sha256\":\"{}\",\"lrat_bytes\":{},\"lrat\":\"{}\"",
                sha256_hex(lrat.as_bytes()),
                lrat.len(),
                lrat.replace('\n', "\\n")
            );
        }
        Answer::Unknown { reason } => {
            let _ = write!(text, ",\"reason\":\"{}\"", reason.name());
            if let UnknownReason::ProofOverCap { bytes } = reason {
                let _ = write!(text, ",\"lrat_bytes\":{bytes}");
            }
        }
    }
    text.push('}');
    text
}

/// The fixed serialization: top-level keys in order, two-space indentation
/// of the top level only, entries sorted by digest one per line, integers
/// only, a final newline, and `\n` the only string escape.
pub fn serialize_answers(file: &AnswerFile) -> Result<String, OracleError> {
    validate_file(file)?;
    let mut text = String::new();
    let _ = write!(
        text,
        "{{\n  \"schema_version\": {SCHEMA_VERSION},\n  \"experiment\": \"{}\",\n  \"oracle\": {{\"name\":\"{}\",\"version\":\"{}\",\"path\":\"{}\",\"sha256\":\"{}\",\"arguments\":[\"{}\",\"{}\",\"{}\",\"-c\",\"{}\"]}},\n  \"proof_text_cap_bytes\": {PROOF_TEXT_CAP_BYTES},\n",
        file.experiment,
        file.identity.name,
        file.identity.version,
        file.identity.path,
        file.identity.sha256,
        FIXED_ARGUMENTS[0],
        FIXED_ARGUMENTS[1],
        FIXED_ARGUMENTS[2],
        file.conflict_limit
    );
    if file.entries.is_empty() {
        text.push_str("  \"entries\": []\n}\n");
    } else {
        text.push_str("  \"entries\": [\n");
        let lines: Vec<String> = file
            .entries
            .values()
            .map(|entry| format!("    {}", entry_json(entry)))
            .collect();
        text.push_str(&lines.join(",\n"));
        text.push_str("\n  ]\n}\n");
    }
    if text.len() as u64 > ANSWER_FILE_CAP_BYTES {
        return Err(OracleError::FileCap {
            bytes: text.len() as u64,
        });
    }
    Ok(text)
}

pub fn write_answers(path: &Path, file: &AnswerFile) -> Result<(), OracleError> {
    let text = serialize_answers(file)?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    Ok(())
}

/// A minimal strict JSON reader for the fixed answer-file format: objects,
/// arrays, strings whose only escape is `\n`, and integers.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Json {
    Object(Vec<(String, Json)>),
    Array(Vec<Json>),
    Str(String),
    Int(i64),
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn fail<T>(&self, reason: &str) -> Result<T, OracleError> {
        Err(OracleError::AnswerFormat(format!(
            "json at byte {}: {reason}",
            self.at
        )))
    }
    fn skip_space(&mut self) {
        while self.at < self.bytes.len() && matches!(self.bytes[self.at], b' ' | b'\n') {
            self.at += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }
    fn expect(&mut self, byte: u8) -> Result<(), OracleError> {
        if self.peek() == Some(byte) {
            self.at += 1;
            Ok(())
        } else {
            self.fail(&format!("expected {:?}", byte as char))
        }
    }
    fn value(&mut self, depth: u32) -> Result<Json, OracleError> {
        if depth > 8 {
            return self.fail("nesting");
        }
        self.skip_space();
        match self.peek() {
            Some(b'{') => {
                self.at += 1;
                let mut fields = Vec::new();
                self.skip_space();
                if self.peek() == Some(b'}') {
                    self.at += 1;
                    return Ok(Json::Object(fields));
                }
                loop {
                    self.skip_space();
                    let key = self.string()?;
                    self.skip_space();
                    self.expect(b':')?;
                    let value = self.value(depth + 1)?;
                    if fields.iter().any(|(k, _)| *k == key) {
                        return self.fail("duplicate key");
                    }
                    fields.push((key, value));
                    self.skip_space();
                    match self.peek() {
                        Some(b',') => self.at += 1,
                        Some(b'}') => {
                            self.at += 1;
                            return Ok(Json::Object(fields));
                        }
                        _ => return self.fail("expected , or }"),
                    }
                }
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.skip_space();
                if self.peek() == Some(b']') {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.skip_space();
                    match self.peek() {
                        Some(b',') => self.at += 1,
                        Some(b']') => {
                            self.at += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return self.fail("expected , or ]"),
                    }
                }
            }
            Some(b'"') => self.string().map(Json::Str),
            Some(b'-' | b'0'..=b'9') => {
                let start = self.at;
                self.at += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.at += 1;
                }
                let token = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or("");
                strict_int(token).map(Json::Int).ok_or_else(|| {
                    OracleError::AnswerFormat(format!("json at byte {start}: integer {token:?}"))
                })
            }
            _ => self.fail("unexpected byte"),
        }
    }
    fn string(&mut self) -> Result<String, OracleError> {
        self.expect(b'"')?;
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None => return self.fail("unterminated string"),
                Some(b'"') => {
                    self.at += 1;
                    break;
                }
                Some(b'\\') => {
                    if self.bytes.get(self.at + 1) != Some(&b'n') {
                        return self.fail("only \\n may be escaped");
                    }
                    out.push(b'\n');
                    self.at += 2;
                }
                Some(byte) if byte < 0x20 => return self.fail("control character in string"),
                Some(byte) => {
                    out.push(byte);
                    self.at += 1;
                }
            }
        }
        String::from_utf8(out).map_err(|_| OracleError::AnswerFormat("string is not UTF-8".into()))
    }
}

fn parse_json(text: &str) -> Result<Json, OracleError> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = reader.value(0)?;
    reader.skip_space();
    if reader.at != reader.bytes.len() {
        return reader.fail("trailing bytes");
    }
    Ok(value)
}

fn object<'a>(value: &'a Json, what: &str) -> Result<&'a [(String, Json)], OracleError> {
    match value {
        Json::Object(fields) => Ok(fields),
        _ => Err(OracleError::AnswerFormat(format!(
            "{what} must be an object"
        ))),
    }
}

fn field<'a>(fields: &'a [(String, Json)], key: &str) -> Result<&'a Json, OracleError> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| OracleError::AnswerFormat(format!("missing field {key}")))
}

fn text_field(fields: &[(String, Json)], key: &str) -> Result<String, OracleError> {
    match field(fields, key)? {
        Json::Str(text) => Ok(text.clone()),
        _ => Err(OracleError::AnswerFormat(format!(
            "field {key} must be a string"
        ))),
    }
}

fn count_field(fields: &[(String, Json)], key: &str) -> Result<u64, OracleError> {
    match field(fields, key)? {
        Json::Int(value) => u64::try_from(*value)
            .map_err(|_| OracleError::AnswerFormat(format!("field {key} must be nonnegative"))),
        _ => Err(OracleError::AnswerFormat(format!(
            "field {key} must be an integer"
        ))),
    }
}

fn expect_keys(fields: &[(String, Json)], keys: &[&str], what: &str) -> Result<(), OracleError> {
    let actual: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
    if actual != keys {
        return Err(OracleError::AnswerFormat(format!(
            "{what} fields {actual:?} differ from {keys:?}"
        )));
    }
    Ok(())
}

/// Parses answer-file bytes, validates every entry, and requires the bytes
/// to be exactly the fixed serialization of their content.
pub fn read_answers_bytes(bytes: &[u8]) -> Result<AnswerFile, OracleError> {
    if bytes.len() as u64 > ANSWER_FILE_CAP_BYTES {
        return Err(OracleError::FileCap {
            bytes: bytes.len() as u64,
        });
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| OracleError::AnswerFormat("file is not UTF-8".into()))?;
    let root = parse_json(text)?;
    let top = object(&root, "file")?;
    expect_keys(
        top,
        &[
            "schema_version",
            "experiment",
            "oracle",
            "proof_text_cap_bytes",
            "entries",
        ],
        "file",
    )?;
    if count_field(top, "schema_version")? != SCHEMA_VERSION {
        return Err(OracleError::AnswerFormat("schema_version".into()));
    }
    let experiment = text_field(top, "experiment")?;
    let oracle = object(field(top, "oracle")?, "oracle")?;
    expect_keys(
        oracle,
        &["name", "version", "path", "sha256", "arguments"],
        "oracle",
    )?;
    let identity = Identity {
        name: text_field(oracle, "name")?,
        path: text_field(oracle, "path")?,
        version: text_field(oracle, "version")?,
        sha256: text_field(oracle, "sha256")?,
    };
    let arguments = match field(oracle, "arguments")? {
        Json::Array(items) => items
            .iter()
            .map(|item| match item {
                Json::Str(text) => Ok(text.clone()),
                _ => Err(OracleError::AnswerFormat(
                    "arguments must be strings".into(),
                )),
            })
            .collect::<Result<Vec<String>, OracleError>>()?,
        _ => {
            return Err(OracleError::AnswerFormat(
                "arguments must be an array".into(),
            ))
        }
    };
    let fixed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let conflict_limit = match fixed.as_slice() {
        [a, b, c, "-c", limit] if [*a, *b, *c] == FIXED_ARGUMENTS => strict_int(limit)
            .and_then(|v| u64::try_from(v).ok())
            .ok_or_else(|| OracleError::AnswerFormat("conflict limit argument".into()))?,
        _ => {
            return Err(OracleError::AnswerIdentity {
                field: "arguments".into(),
                expected: format!("{FIXED_ARGUMENTS:?} then -c <limit>"),
                actual: format!("{fixed:?}"),
            })
        }
    };
    let cap = count_field(top, "proof_text_cap_bytes")?;
    if cap != PROOF_TEXT_CAP_BYTES {
        return Err(OracleError::AnswerIdentity {
            field: "proof_text_cap_bytes".into(),
            expected: PROOF_TEXT_CAP_BYTES.to_string(),
            actual: cap.to_string(),
        });
    }
    let mut entries = BTreeMap::new();
    let Json::Array(items) = field(top, "entries")? else {
        return Err(OracleError::AnswerFormat("entries must be an array".into()));
    };
    for item in items {
        let fields = object(item, "entry")?;
        let common = [
            "dimacs_sha256",
            "variables",
            "clauses",
            "conflict_limit",
            "label",
        ];
        let dimacs_sha256 = text_field(fields, "dimacs_sha256")?;
        let variables = u32::try_from(count_field(fields, "variables")?)
            .map_err(|_| OracleError::AnswerFormat("variables".into()))?;
        let clauses = count_field(fields, "clauses")?;
        let conflict_limit = count_field(fields, "conflict_limit")?;
        let label = text_field(fields, "label")?;
        let answer = match label.as_str() {
            "sat" => {
                expect_keys(
                    fields,
                    &[common.as_slice(), &["model"]].concat(),
                    "sat entry",
                )?;
                let Json::Array(items) = field(fields, "model")? else {
                    return Err(OracleError::AnswerFormat("model must be an array".into()));
                };
                let model = items
                    .iter()
                    .map(|item| match item {
                        Json::Int(value) => i32::try_from(*value)
                            .map_err(|_| OracleError::AnswerFormat("model literal".into())),
                        _ => Err(OracleError::AnswerFormat("model literal".into())),
                    })
                    .collect::<Result<Vec<i32>, OracleError>>()?;
                Answer::Sat { model }
            }
            "unsat" => {
                expect_keys(
                    fields,
                    &[common.as_slice(), &["lrat_sha256", "lrat_bytes", "lrat"]].concat(),
                    "unsat entry",
                )?;
                let lrat = text_field(fields, "lrat")?;
                if count_field(fields, "lrat_bytes")? != lrat.len() as u64 {
                    return Err(OracleError::AnswerFormat(format!(
                        "entry {dimacs_sha256}: lrat_bytes disagrees with the text"
                    )));
                }
                if text_field(fields, "lrat_sha256")? != sha256_hex(lrat.as_bytes()) {
                    return Err(OracleError::AnswerFormat(format!(
                        "entry {dimacs_sha256}: lrat_sha256 disagrees with the text"
                    )));
                }
                Answer::Unsat { lrat }
            }
            "unknown" => {
                let reason = text_field(fields, "reason")?;
                match reason.as_str() {
                    "conflict-limit" => {
                        expect_keys(
                            fields,
                            &[common.as_slice(), &["reason"]].concat(),
                            "unknown entry",
                        )?;
                        Answer::Unknown {
                            reason: UnknownReason::ConflictLimit,
                        }
                    }
                    "proof-over-cap" => {
                        expect_keys(
                            fields,
                            &[common.as_slice(), &["reason", "lrat_bytes"]].concat(),
                            "unknown entry",
                        )?;
                        Answer::Unknown {
                            reason: UnknownReason::ProofOverCap {
                                bytes: count_field(fields, "lrat_bytes")?,
                            },
                        }
                    }
                    _ => {
                        return Err(OracleError::AnswerFormat(format!(
                            "unknown reason {reason:?}"
                        )))
                    }
                }
            }
            _ => return Err(OracleError::AnswerFormat(format!("label {label:?}"))),
        };
        let entry = AnswerEntry {
            dimacs_sha256: dimacs_sha256.clone(),
            variables,
            clauses,
            conflict_limit,
            answer,
        };
        if entries.insert(dimacs_sha256.clone(), entry).is_some() {
            return Err(OracleError::AnswerDuplicate { dimacs_sha256 });
        }
    }
    let file = AnswerFile {
        experiment,
        identity,
        conflict_limit,
        entries,
    };
    let canonical = serialize_answers(&file)?;
    if canonical != text {
        return Err(OracleError::AnswerFormat(
            "file bytes are not the fixed serialization of their content".into(),
        ));
    }
    Ok(file)
}

pub fn read_answers(path: &Path) -> Result<AnswerFile, OracleError> {
    let bytes = fs::read(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            OracleError::Missing(path.display().to_string())
        } else {
            OracleError::Scratch(error)
        }
    })?;
    read_answers_bytes(&bytes)
}

// ---------------------------------------------------------------------------
// Driver helpers, registered experiments, regeneration, and the demo replay
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ReplayCase {
    pub id: String,
    pub input: Cnf,
    pub variables: u32,
}

#[derive(Clone, Debug)]
pub struct ReplayRow<A> {
    pub case: ReplayCase,
    pub arm: A,
    pub label: Label,
}

/// The replay order every oracle experiment follows: for each case, run the
/// arm to completion, then label; after the corpus, every committed answer
/// must have been used. The arm receives no oracle value.
pub fn replay_cases<A>(
    experiment: &str,
    cases: Vec<ReplayCase>,
    limits: &Limits,
    answers: &AnswerFile,
    mut arm: impl FnMut(&Cnf, u32) -> Result<A, Failure>,
) -> Result<Vec<ReplayRow<A>>, Failure> {
    if answers.experiment != experiment {
        let error = OracleError::AnswerIdentity {
            field: "experiment".to_string(),
            expected: experiment.to_string(),
            actual: answers.experiment.clone(),
        };
        return Err(oracle_failure("", &error));
    }
    let mut used = BTreeSet::new();
    let mut rows = Vec::with_capacity(cases.len());
    for case in cases {
        let result = arm(&case.input, case.variables)?;
        let label = label(&case.input, case.variables, limits, answers, &mut used)?;
        rows.push(ReplayRow {
            case,
            arm: result,
            label,
        });
    }
    check_used(answers, &used).map_err(|error| match &error {
        OracleError::AnswerUnused { dimacs_sha256 } => oracle_failure(dimacs_sha256, &error),
        _ => oracle_failure("", &error),
    })?;
    Ok(rows)
}

/// Counts and checker totals over a corpus's labels.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub cases: u64,
    pub truth_table_cases: u64,
    pub definition_cases: u64,
    pub oracle_cases: u64,
    pub oracle_calls: u64,
    pub oracle_sat: u64,
    pub oracle_unsat: u64,
    pub oracle_unknown: u64,
    pub model_check_work_units: u64,
    pub proof_check_work_units: u64,
    pub proof_lemmas_total: u64,
    pub reference_work_units: Option<u64>,
}

fn add(total: &mut u64, amount: u64) -> Result<(), Failure> {
    *total = total.checked_add(amount).ok_or(Failure::CounterOverflow)?;
    Ok(())
}

impl Summary {
    /// Adds one case's label; composed calls are added with `include_call`.
    pub fn include(&mut self, label: &Label) -> Result<(), Failure> {
        add(&mut self.cases, 1)?;
        match label {
            Label::TruthTable(reference) => {
                add(&mut self.truth_table_cases, 1)?;
                let total = self.reference_work_units.unwrap_or(0);
                self.reference_work_units = Some(
                    total
                        .checked_add(reference.work.work_units)
                        .ok_or(Failure::CounterOverflow)?,
                );
            }
            Label::Definition(definition) => {
                add(&mut self.definition_cases, 1)?;
                if let Definition::NoClauses { model_check, .. } = definition {
                    add(
                        &mut self.model_check_work_units,
                        model_check.work.work_units,
                    )?;
                }
            }
            Label::Oracle(verdict) => {
                add(&mut self.oracle_cases, 1)?;
                self.include_call(verdict)?;
            }
        }
        Ok(())
    }
    pub fn include_call(&mut self, verdict: &OracleVerdict) -> Result<(), Failure> {
        add(&mut self.oracle_calls, 1)?;
        match &verdict.verdict {
            Verdict::Sat { model_check, .. } => {
                add(&mut self.oracle_sat, 1)?;
                add(
                    &mut self.model_check_work_units,
                    model_check.work.work_units,
                )
            }
            Verdict::Unsat { proof_check } => {
                add(&mut self.oracle_unsat, 1)?;
                add(
                    &mut self.proof_check_work_units,
                    proof_check.work.work_units,
                )?;
                add(&mut self.proof_lemmas_total, proof_check.lemmas)
            }
            Verdict::Unknown { .. } => add(&mut self.oracle_unknown, 1),
        }
    }
    pub fn json(&self, enumeration_ratio: Option<&RatioPair>) -> String {
        format!(
            "{{\"cases\":{},\"truth_table_cases\":{},\"definition_cases\":{},\"oracle_cases\":{},\"oracle_calls\":{},\"oracle_sat\":{},\"oracle_unsat\":{},\"oracle_unknown\":{},\"model_check_work_units\":{},\"proof_check_work_units\":{},\"proof_lemmas_total\":{},\"reference_work_units\":{},\"enumeration_ratio\":{}}}",
            self.cases,
            self.truth_table_cases,
            self.definition_cases,
            self.oracle_cases,
            self.oracle_calls,
            self.oracle_sat,
            self.oracle_unsat,
            self.oracle_unknown,
            self.model_check_work_units,
            self.proof_check_work_units,
            self.proof_lemmas_total,
            self.reference_work_units
                .map_or_else(|| "null".into(), |units| units.to_string()),
            ratio_json(enumeration_ratio)
        )
    }
}

/// A registered oracle experiment: its corpus and its conflict limit.
pub struct Experiment {
    pub name: &'static str,
    pub limits: Limits,
    pub corpus: fn() -> Result<Vec<ReplayCase>, OracleError>,
}

pub const DEMO_LIMITS: Limits = Limits { conflicts: 100 };

/// Three DIMACS fixtures, one per label at the demo limit: random 3-CNF
/// r16 (unsat), r30 (sat), and s60 (unknown at 100 conflicts).
pub fn demo_corpus() -> Result<Vec<ReplayCase>, OracleError> {
    [
        ("r16", include_str!("../tests/lrat/r16.cnf")),
        ("r30", include_str!("../tests/lrat/r30.cnf")),
        ("s60", include_str!("../tests/oracle/s60.cnf")),
    ]
    .into_iter()
    .map(|(id, text)| {
        let (input, variables) = parse_dimacs(text)?;
        Ok(ReplayCase {
            id: id.into(),
            input,
            variables,
        })
    })
    .collect()
}

pub const EXPERIMENTS: [Experiment; 1] = [Experiment {
    name: "demo",
    limits: DEMO_LIMITS,
    corpus: demo_corpus,
}];

pub fn experiment(name: &str) -> Option<&'static Experiment> {
    EXPERIMENTS
        .iter()
        .find(|experiment| experiment.name == name)
}

pub fn answers_path(name: &str) -> String {
    format!("artifacts/{name}-oracle.json")
}

/// `<dir>/<experiment>-timing.json` beside an answer file.
pub fn timing_path(answers: &Path) -> PathBuf {
    let name = answers
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("oracle.json");
    let stem = name
        .strip_suffix("-oracle.json")
        .or_else(|| name.strip_suffix(".json"))
        .unwrap_or(name);
    answers.with_file_name(format!("{stem}-timing.json"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regeneration {
    pub answers_path: PathBuf,
    pub timing_path: PathBuf,
    pub entries: u64,
    pub sat: u64,
    pub unsat: u64,
    pub unknown: u64,
    pub total_wall_milliseconds: u64,
}

/// Regeneration (`peqnp oracle-answers <experiment> [path]`): identity, the
/// frozen corpus, one `solve` per distinct formula above the truth-table
/// bound that is not a definition case, `write_answers`, and the timing
/// sidecar. Never runs an arm and never reads an artifact.
pub fn regenerate(name: &str, path: &Path) -> Result<Regeneration, OracleError> {
    let experiment = experiment(name).ok_or_else(|| {
        OracleError::Missing(format!(
            "experiment {name:?} is not registered for oracle regeneration"
        ))
    })?;
    let (identity, resolved) = identity_resolved()?;
    let cases = (experiment.corpus)()?;
    let mut entries = BTreeMap::new();
    let mut calls: Vec<(String, u64)> = Vec::new();
    let mut total_wall_milliseconds = 0_u64;
    for case in &cases {
        if case.variables <= TRUTH_TABLE_MAX_VARIABLES {
            continue;
        }
        if case.input.is_empty() || case.input.iter().any(Vec::is_empty) {
            continue;
        }
        let digest = sha256_hex(write_dimacs(&case.input, case.variables).as_bytes());
        if entries.contains_key(&digest) {
            continue;
        }
        let (entry, wall) = solve(&case.input, case.variables, &experiment.limits)?;
        total_wall_milliseconds = total_wall_milliseconds.saturating_add(wall);
        calls.push((digest.clone(), wall));
        entries.insert(digest, entry);
    }
    let file = AnswerFile {
        experiment: name.into(),
        identity,
        conflict_limit: experiment.limits.conflicts,
        entries,
    };
    let text = serialize_answers(&file)?;
    write_answers(path, &file)?;
    let (sat, unsat, unknown) =
        file.entries
            .values()
            .fold((0, 0, 0), |(s, u, k), entry| match entry.answer {
                Answer::Sat { .. } => (s + 1, u, k),
                Answer::Unsat { .. } => (s, u + 1, k),
                Answer::Unknown { .. } => (s, u, k + 1),
            });
    let recorded_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut timing = format!(
        "{{\"experiment\":\"{}\",\"answers_sha256\":\"{}\",\"host\":{{\"os\":\"{}\",\"arch\":\"{}\",\"resolved_path\":\"{}\"}},\"recorded_at\":{},\"calls\":[",
        name,
        sha256_hex(text.as_bytes()),
        std::env::consts::OS,
        std::env::consts::ARCH,
        resolved.display(),
        recorded_at
    );
    for (index, (digest, wall)) in calls.iter().enumerate() {
        if index > 0 {
            timing.push(',');
        }
        let _ = write!(
            timing,
            "{{\"dimacs_sha256\":\"{digest}\",\"wall_milliseconds\":{wall}}}"
        );
    }
    let _ = writeln!(
        timing,
        "],\"total_wall_milliseconds\":{total_wall_milliseconds}}}"
    );
    let timing_path = timing_path(path);
    fs::write(&timing_path, timing)?;
    Ok(Regeneration {
        answers_path: path.to_path_buf(),
        timing_path,
        entries: file.entries.len() as u64,
        sat,
        unsat,
        unknown,
        total_wall_milliseconds,
    })
}

/// The demo replay artifact: no arm, three labelled cases, the summary, and
/// the answer-file identity. Byte-identical on every host for one answer file.
pub fn demo_replay(answers_bytes: &[u8]) -> Result<String, Failure> {
    let answers = read_answers_bytes(answers_bytes).map_err(|error| oracle_failure("", &error))?;
    let cases = demo_corpus().map_err(|error| oracle_failure("", &error))?;
    let rows = replay_cases("demo", cases, &DEMO_LIMITS, &answers, |_, _| Ok(()))?;
    let mut summary = Summary::default();
    let mut text = format!(
        "{{\n\"schema_version\":1,\"experiment\":\"demo\",\"status\":\"plumbing-demo\",\"proof_status\":\"no-formal-proof\",\n\"reference_model\":\"{}\",\"oracle_limits\":{{\"conflicts\":{}}},\"proof_text_cap_bytes\":{},\n\"oracle_answers\":{{\"path\":\"{}\",\"sha256\":\"{}\"}},\n\"limitations\":[\"Plumbing demonstration over three fixed DIMACS fixtures with no arm: it exercises regeneration, the answer file, and replay verification, and supports no research claim.\",\"A conflict-limit unknown is unknown, never a decision and never a refutation; finite runs prove no asymptotic bound; no P versus NP claim.\"],\n\"cases\":[\n",
        REFERENCE_MODEL,
        DEMO_LIMITS.conflicts,
        PROOF_TEXT_CAP_BYTES,
        answers_path("demo"),
        sha256_hex(answers_bytes)
    );
    for (index, row) in rows.iter().enumerate() {
        summary.include(&row.label)?;
        let _ = write!(
            text,
            "{}{{\"id\":\"{}\",\"variables\":{},\"clauses\":{},\"arm\":null,\"reference\":{}}}",
            if index > 0 { ",\n" } else { "" },
            row.case.id,
            row.case.variables,
            row.case.input.len(),
            row.label.json()
        );
    }
    let _ = write!(text, "\n],\n\"summary\":{}\n}}\n", summary.json(None));
    Ok(text)
}

pub fn demo_summary_line(artifact: &str) -> String {
    let count = |label: &str| {
        artifact
            .matches(&format!("\"kind\":\"oracle\",\"label\":\"{label}\""))
            .count()
    };
    format!(
        "Demo replay: 3 cases labelled by the answer file; sat {}, unsat {}, unknown {}.",
        count("sat"),
        count("unsat"),
        count("unknown")
    )
}

#[cfg(test)]
mod tests {
    use super::{ratio_json, ratio_pair};
    use crate::fragment;

    #[test]
    fn ratio_pair_agrees_with_fragment_ratio() {
        for (numerator, denominator) in [
            (0, 0),
            (1, 0),
            (0, 1),
            (1, 2),
            (1, 3),
            (2, 3),
            (1, 16),
            (3, 16),
            (7, 8),
            (999, 1000),
            (1_518_173, 124_045),
            (u64::MAX, u64::MAX),
            (u64::MAX, 1),
            (u64::MAX / 1000, 1),
            (u64::MAX / 1000 + 1, 1),
        ] {
            let expected = fragment::ratio(numerator, denominator);
            let pair = ratio_pair(numerator, denominator);
            let thousandths = pair
                .as_ref()
                .map_or_else(|| "null".to_owned(), |p| p.thousandths.clone());
            assert_eq!(thousandths, expected, "{numerator}/{denominator}");
            let json = ratio_json(pair.as_ref());
            if expected == "null" {
                assert_eq!(json, "null");
            } else {
                assert_eq!(
                    json,
                    format!(
                        "{{\"numerator\":{numerator},\"denominator\":{denominator},\"thousandths\":\"{expected}\"}}"
                    )
                );
            }
        }
    }
}
