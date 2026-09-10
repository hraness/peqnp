//! Offline checks of the reference oracle (no solver) and, under `--ignored`,
//! live checks against the pinned CaDiCaL 3.0.1 binary.
//!
//! Transcripts were captured from `/opt/homebrew/bin/cadical -q --lrat
//! --binary=false -c 100` on the fixtures named in each test.

use peqnp::oracle::{
    check_lrat, check_model, check_used, demo_corpus, demo_replay, identity, label, parse_dimacs,
    parse_output, ratio_pair, read_answers, read_answers_bytes, replay_cases, serialize_answers,
    solve, timing_path, verify, write_answers, write_dimacs, Answer, AnswerEntry, AnswerFile,
    Definition, Identity, Label, Limits, OracleError, Status, UnknownReason, Verdict,
    ANSWER_FILE_CAP_BYTES, CADICAL, DEMO_LIMITS, FIXED_ARGUMENTS, PROOF_TEXT_CAP_BYTES,
    TRUTH_TABLE_MAX_VARIABLES,
};
use peqnp::sha256::{hex, sha256, sha256_hex, Sha256};
use peqnp::transfer::Failure;
use peqnp::Cnf;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const R16: &str = include_str!("lrat/r16.cnf");
const R30: &str = include_str!("lrat/r30.cnf");
const S60: &str = include_str!("oracle/s60.cnf");
const R60: &str = include_str!("oracle/r60.cnf");
const P60: &str = include_str!("oracle/p60.lrat");
const PHP87: &str = include_str!("oracle/php87.cnf");
const DEMO_ANSWERS: &[u8] = include_bytes!("oracle/demo-oracle.json");
const DEMO_ARTIFACT: &[u8] = include_bytes!("oracle/demo-reference.json");

// `-q` transcripts of the pinned binary at `-c 100`.
const SAT_TRANSCRIPT: &str = "s SATISFIABLE\nv -1 2 3 -4 5 6 -7 8 9 10 11 -12 -13 14 15 16 17 -18 19 20 -21 -22 23 -24 25\nv -26 -27 28 29 -30 0\n";
const UNSAT_TRANSCRIPT: &str = "s UNSATISFIABLE\n";
const UNKNOWN_TRANSCRIPT: &str = "c UNKNOWN\n";

/// The four-clause contradiction over two variables: original ids 1..=4.
const CONTRADICTION: [&[i32]; 4] = [&[1, 2], &[-1, 2], &[1, -2], &[-1, -2]];
const CONTRADICTION_PROOF: &str = "5 2 0 1 2 0\n6 0 5 3 4 0\n";

fn contradiction() -> Cnf {
    CONTRADICTION.iter().map(|c| c.to_vec()).collect()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("peqnp-oracle-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir.join(name)
}

fn demo_file() -> AnswerFile {
    read_answers_bytes(DEMO_ANSWERS).expect("demo answer file")
}

fn digest_of(input: &Cnf, n: u32) -> String {
    sha256_hex(write_dimacs(input, n).as_bytes())
}

fn oracle_reason(failure: &Failure) -> String {
    match failure {
        Failure::Oracle { reason, .. } => reason.clone(),
        other => panic!("expected an oracle failure, got {other:?}"),
    }
}

fn invalid_line(error: OracleError) -> (u64, String) {
    match error {
        OracleError::ProofInvalid { line, reason } => (line, reason),
        other => panic!("expected ProofInvalid, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// SHA-256
// ---------------------------------------------------------------------------

#[test]
fn sha256_nist_vectors_and_streaming() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let two_block = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(
        sha256_hex(two_block),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    assert_eq!(
        sha256_hex(&vec![b'a'; 1_000_000]),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
    let expected = sha256(two_block);
    assert_eq!(hex(&expected), sha256_hex(two_block));
    for split in 0..=two_block.len() {
        let mut hasher = Sha256::new();
        hasher.update(&two_block[..split]);
        hasher.update(&two_block[split..]);
        assert_eq!(hasher.finish(), expected, "split {split}");
    }
}

// ---------------------------------------------------------------------------
// DIMACS
// ---------------------------------------------------------------------------

#[test]
fn write_dimacs_layout() {
    assert_eq!(write_dimacs(&vec![], 3), "p cnf 3 0\n");
    assert_eq!(write_dimacs(&vec![vec![]], 2), "p cnf 2 1\n0\n");
    assert_eq!(write_dimacs(&vec![vec![1, -4]], 9), "p cnf 9 1\n1 -4 0\n");
    assert_eq!(
        write_dimacs(&vec![vec![-15, 7, -8], vec![1, -14, 10]], 16),
        "p cnf 16 2\n-15 7 -8 0\n1 -14 10 0\n"
    );
    assert_eq!(write_dimacs(&vec![], 0), "p cnf 0 0\n");
    assert_eq!(
        write_dimacs(&vec![vec![1, 2], vec![1, 2], vec![1, 1]], 2),
        "p cnf 2 3\n1 2 0\n1 2 0\n1 1 0\n"
    );
}

#[test]
fn dimacs_round_trip_reproduces_every_fixture() {
    for (name, text) in [
        ("r16", R16),
        ("r30", R30),
        ("s60", S60),
        ("r60", R60),
        ("php87", PHP87),
    ] {
        let (input, n) = parse_dimacs(text).expect(name);
        assert_eq!(write_dimacs(&input, n), text, "{name}");
        let (again, m) = parse_dimacs(&write_dimacs(&input, n)).expect(name);
        assert_eq!((again, m), (input, n), "{name}");
    }
    for text in [
        "p cnf 2 1\n0\n",
        "p cnf 0 0\n",
        "p cnf 5 2\n1 -2 0\n1 1 0\n",
        "p cnf 3 1\n1 -1 3 0\n",
    ] {
        let (input, n) = parse_dimacs(text).expect(text);
        assert_eq!(write_dimacs(&input, n), text);
    }
    assert!(parse_dimacs("p cnf 2 1\n3 0\n").is_err(), "literal above n");
    assert!(parse_dimacs("p cnf 2 2\n1 0\n").is_err(), "clause count");
    assert!(parse_dimacs("p cnf 2 1\n1 2\n").is_err(), "unterminated");
}

// ---------------------------------------------------------------------------
// Solver transcripts
// ---------------------------------------------------------------------------

#[test]
fn parse_output_accepts_the_three_pinned_transcripts() {
    let (status, model) = parse_output(SAT_TRANSCRIPT, Some(10), 30).expect("sat");
    assert_eq!(status, Status::Sat);
    let model = model.expect("model");
    assert_eq!(model.len(), 30);
    assert_eq!(&model[..4], &[-1, 2, 3, -4]);
    assert_eq!(&model[25..], &[-26, -27, 28, 29, -30]);
    assert_eq!(
        parse_output(UNSAT_TRANSCRIPT, Some(20), 16).expect("unsat"),
        (Status::Unsat, None)
    );
    assert_eq!(
        parse_output(UNKNOWN_TRANSCRIPT, Some(0), 60).expect("unknown"),
        (Status::Unknown, None)
    );
    // `p cnf 0 0` prints `v 0`.
    assert_eq!(
        parse_output("s SATISFIABLE\nv 0\n", Some(10), 0).expect("empty"),
        (Status::Sat, Some(vec![]))
    );
    // A shuffled model is normalized to ascending order.
    let (_, model) = parse_output("s SATISFIABLE\nv 3 -1 2 0\n", Some(10), 3).expect("shuffled");
    assert_eq!(model, Some(vec![-1, 2, 3]));
}

#[test]
fn parse_output_rejects_inconsistent_transcripts() {
    let parse = |text: &str, exit: Option<i32>, n: u32| match parse_output(text, exit, n) {
        Err(OracleError::Parse(reason)) => reason,
        other => panic!("expected Parse for {text:?}, got {other:?}"),
    };
    assert!(
        parse("v 1 2 0\n", Some(10), 2).contains("exit 10"),
        "missing s line"
    );
    assert!(
        parse(UNSAT_TRANSCRIPT, Some(10), 16).contains("exit 10"),
        "exit/s mismatch"
    );
    assert!(
        parse(SAT_TRANSCRIPT, Some(20), 30).contains("exit 20"),
        "exit/s mismatch"
    );
    assert!(
        parse(UNSAT_TRANSCRIPT, Some(0), 16).contains("exit 0"),
        "s line on exit 0"
    );
    assert!(parse("s SATISFIABLE\n", Some(10), 2).contains("without v lines"));
    assert!(parse("s SATISFIABLE\nv 1 -1 2 0\n", Some(10), 2).contains("assigned twice"));
    assert!(parse("s SATISFIABLE\nv 1 2 3 0\n", Some(10), 2).contains("outside 1..2"));
    assert!(parse("s SATISFIABLE\nv 1 0\n", Some(10), 2).contains("unassigned"));
    assert!(parse("s SATISFIABLE\nv 1 2\n", Some(10), 2).contains("terminating 0"));
    assert!(parse("s SATISFIABLE\nv +1 2 0\n", Some(10), 2).contains("v token"));
    assert!(parse("junk\n", Some(20), 2).contains("unexpected line"));
    assert!(matches!(
        parse_output("", Some(1), 2),
        Err(OracleError::Exit { code: Some(1), .. })
    ));
    assert!(matches!(
        parse_output("", None, 2),
        Err(OracleError::Exit { code: None, .. })
    ));
}

// ---------------------------------------------------------------------------
// Model checker
// ---------------------------------------------------------------------------

#[test]
fn check_model_charges_exactly() {
    // (1 ∨ 2) ∧ (¬1 ∨ 3) ∧ (¬2 ∨ ¬3) under 1, ¬2, 3: reads 1 | 2 | 1,2 = 4.
    let input: Cnf = vec![vec![1, 2], vec![-1, 3], vec![-2, -3]];
    let check = check_model(&input, 3, &[1, -2, 3]).expect("valid");
    assert!(check.valid);
    assert_eq!(check.unsatisfied_clause, None);
    let w = &check.work;
    assert_eq!(
        (
            w.formula_checks,
            w.clause_writes,
            w.literal_writes,
            w.clause_reads,
            w.literal_reads
        ),
        (1, 1, 3, 3, 4)
    );
    assert_eq!(w.work_units, 12);
    assert_eq!(
        w.assignments + w.pair_checks + w.rule_attempts + w.search_nodes,
        0
    );
    // Under ¬1, ¬2, 3 the first clause fails after both its literals are read.
    let check = check_model(&input, 3, &[-1, -2, 3]).expect("checked");
    assert!(!check.valid);
    assert_eq!(check.unsatisfied_clause, Some(0));
    let w = &check.work;
    assert_eq!((w.clause_reads, w.literal_reads, w.work_units), (1, 2, 8));
    // Incomplete and duplicated models are refused before any charge.
    assert_eq!(check_model(&input, 3, &[1, -2]), Err(Failure::InvalidInput));
    assert_eq!(
        check_model(&input, 3, &[1, -2, 2]),
        Err(Failure::InvalidInput)
    );
    assert_eq!(
        check_model(&input, 3, &[1, -2, 4]),
        Err(Failure::InvalidInput)
    );
    assert_eq!(
        check_model(&input, 3, &[1, -2, 0]),
        Err(Failure::InvalidInput)
    );
    // The pinned r30 model: every clause read, every variable written once.
    let (r30, n) = parse_dimacs(R30).expect("r30");
    let (_, model) = parse_output(SAT_TRANSCRIPT, Some(10), n).expect("sat");
    let check = check_model(&r30, n, &model.expect("model")).expect("valid");
    assert!(check.valid);
    let w = &check.work;
    assert_eq!(
        (
            w.formula_checks,
            w.clause_writes,
            w.literal_writes,
            w.clause_reads
        ),
        (1, 1, 30, 128)
    );
    assert_eq!(w.literal_reads, 203);
    assert_eq!(w.work_units, 363);
}

// ---------------------------------------------------------------------------
// LRAT checker
// ---------------------------------------------------------------------------

#[test]
fn check_lrat_accepts_the_random_60_proof_with_exact_charges() {
    let (input, n) = parse_dimacs(R60).expect("r60");
    assert_eq!((input.len(), n), (300, 60));
    let check = check_lrat(&input, n, P60).expect("valid proof");
    assert!(check.valid);
    assert_eq!(check.lrat_bytes, 5604);
    assert_eq!(
        check.lrat_sha256,
        "99921bf4b64e0e368d10f0b775a93d800456e3b8434d87a25b8241a813cfcb9f"
    );
    assert_eq!((check.lemmas, check.deletions), (89, 27));
    let w = &check.work;
    assert_eq!(w.formula_checks, 89);
    assert_eq!(
        w.clause_writes + w.assignments + w.pair_checks + w.rule_attempts + w.search_nodes,
        0
    );
    assert_eq!(
        (w.clause_reads, w.literal_reads, w.literal_writes),
        (956, 3157, 1193)
    );
    assert_eq!(w.work_units, 5395);
    // The tiny contradiction: two lemmas, the second empty.
    let check = check_lrat(&contradiction(), 2, CONTRADICTION_PROOF).expect("valid");
    assert_eq!(
        (check.lemmas, check.deletions, check.lrat_bytes),
        (2, 0, 24)
    );
    // Lemma 5: 1 formula, 1 literal read+write, hints 1 and 2 (2 clause reads,
    // 4 literal reads, 1 unit write); lemma 6: 1 formula, hints 5,3,4 (3 clause
    // reads, 5 literal reads, 2 unit writes).
    let w = &check.work;
    assert_eq!(
        (
            w.formula_checks,
            w.clause_reads,
            w.literal_reads,
            w.literal_writes
        ),
        (2, 5, 10, 4)
    );
    assert_eq!(w.work_units, 21);
}

#[test]
fn check_lrat_names_each_failure() {
    let input = contradiction();
    let check = |proof: &str| invalid_line(check_lrat(&input, 2, proof).expect_err(proof));
    let (line, reason) = check("5 2 0 1 2 0\n");
    assert_eq!((line, reason.as_str()), (1, "no empty clause"));
    let (line, reason) = check("5 2 0 1 0\n6 0 5 3 4 0\n");
    assert!(
        line == 1 && reason.contains("hint 1"),
        "{reason}: no conflict"
    );
    let (line, reason) = check("5 2 0 3 0\n6 0 5 3 4 0\n");
    assert!(
        line == 1 && reason.contains("hint 3"),
        "{reason}: satisfied hint"
    );
    let (line, reason) = check("5 2 0 1 2 3 0\n6 0 5 3 4 0\n");
    assert_eq!(
        (line, reason.as_str()),
        (1, "conflict at hint 2 before the last hint")
    );
    let (line, reason) = check("5 0 1 0\n");
    assert!(
        line == 1 && reason.contains("hint 1"),
        "{reason}: two unassigned"
    );
    let (line, reason) = check("5 2 0 1 2 0\n5 d 1 0\n6 0 5 3 4 1 0\n");
    assert_eq!((line, reason.as_str()), (3, "hint 1 is not a live clause"));
    let (line, reason) = check("5 2 0 1 2 0\n5 d 1 0\n5 d 1 0\n6 0 5 3 4 0\n");
    assert_eq!(
        (line, reason.as_str()),
        (3, "deletion of clause 1 that is not live")
    );
    let (line, reason) = check("5 2 0 1 2 0\n5 0 5 3 4 0\n");
    assert_eq!((line, reason.as_str()), (2, "lemma id does not increase"));
    let (line, reason) = check("6 2 0 1 2 0\n7 0 6 3 4 0\n");
    assert_eq!((line, reason.as_str()), (1, "first lemma id must be 5"));
    let (line, reason) = check("5 3 0 1 2 0\n");
    assert!(
        line == 1 && reason.contains("literal 3 outside"),
        "{reason}"
    );
    let (line, reason) = check("5 2 -2 0 1 2 0\n");
    assert!(line == 1 && reason.contains("tautological"), "{reason}");
    let (line, reason) = check("5 2 2 0 1 2 0\n");
    assert_eq!((line, reason.as_str()), (1, "duplicate literal 2"));
    let (line, reason) = check("5 2 0 1 2 0\n6 0 5 3 4 0\n7 1 0 1 0\n");
    assert_eq!((line, reason.as_str()), (3, "lemma after the empty clause"));
    let (line, reason) = check("5 2 0 1 2 0\n6 0 5 3 4 0 c\n");
    assert_eq!((line, reason.as_str()), (2, "stray character"));
    let (line, reason) = check("5 2 0 1 2 0\t\n");
    assert_eq!((line, reason.as_str()), (1, "stray character"));
    let (line, reason) = check("5 2 0 1 2 0\n\n6 0 5 3 4 0\n");
    assert_eq!((line, reason.as_str()), (2, "empty line"));
    let (line, reason) = check("5 2 0  1 2 0\n");
    assert!(
        line == 1 && reason.contains("not a decimal integer"),
        "{reason}"
    );
    let (line, reason) = check("0 d 1 0\n5 2 0 1 2 0\n");
    assert_eq!((line, reason.as_str()), (1, "line id must be at least 1"));
    let (line, reason) = check("5 2 0 -1 2 0\n");
    assert!(line == 1 && reason.contains("negative hint"), "{reason}");
    let (line, reason) = check("5 2 0 0\n");
    assert_eq!((line, reason.as_str()), (1, "lemma has no hints"));
    assert!(
        check_lrat(&input, 2, "").is_err(),
        "empty proof proves nothing"
    );
    // Deletions after the empty clause are permitted.
    let check = check_lrat(&input, 2, "5 2 0 1 2 0\n6 0 5 3 4 0\n6 d 1 2 0\n").expect("valid");
    assert_eq!(check.deletions, 2);
    // Text over the cap is unknown, never invalid.
    let over = "0".repeat(usize::try_from(PROOF_TEXT_CAP_BYTES).expect("cap") + 1);
    assert!(matches!(
        check_lrat(&input, 2, &over),
        Err(OracleError::ProofOverCap { bytes }) if bytes == PROOF_TEXT_CAP_BYTES + 1
    ));
    // A damaged pinned proof fails on its own line.
    let damaged = P60.replacen("301 -52 -9 -58 -59 -60 0", "301 52 -9 -58 -59 -60 0", 1);
    let (r60, n) = parse_dimacs(R60).expect("r60");
    let (line, _) = invalid_line(check_lrat(&r60, n, &damaged).expect_err("damaged"));
    assert_eq!(line, 1);
}

// ---------------------------------------------------------------------------
// Answer files
// ---------------------------------------------------------------------------

fn four_shapes() -> AnswerFile {
    let entry = |digest: &str, variables: u32, clauses: u64, answer: Answer| AnswerEntry {
        dimacs_sha256: digest.into(),
        variables,
        clauses,
        conflict_limit: 100,
        answer,
    };
    let (r16, n) = parse_dimacs(R16).expect("r16");
    let entries = [
        entry(
            &"a".repeat(64),
            3,
            2,
            Answer::Sat {
                model: vec![-1, 2, -3],
            },
        ),
        entry(
            &digest_of(&r16, n),
            n,
            r16.len() as u64,
            Answer::Unsat {
                lrat: include_str!("lrat/p16-1.lrat").into(),
            },
        ),
        entry(
            &"c".repeat(64),
            60,
            240,
            Answer::Unknown {
                reason: UnknownReason::ConflictLimit,
            },
        ),
        entry(
            &"d".repeat(64),
            150,
            640,
            Answer::Unknown {
                reason: UnknownReason::ProofOverCap { bytes: 3_407_112 },
            },
        ),
    ];
    AnswerFile {
        experiment: "shapes".into(),
        identity: Identity::pinned(),
        conflict_limit: 100,
        entries: entries
            .into_iter()
            .map(|entry| (entry.dimacs_sha256.clone(), entry))
            .collect(),
    }
}

#[test]
fn answer_file_round_trip_in_the_fixed_serialization() {
    let file = four_shapes();
    let text = serialize_answers(&file).expect("serialize");
    assert!(text.starts_with("{\n  \"schema_version\": 1,\n  \"experiment\": \"shapes\",\n  \"oracle\": {\"name\":\"cadical\",\"version\":\"3.0.1\",\"path\":\"/opt/homebrew/bin/cadical\",\"sha256\":\"601c9fa8ba5d09fd81bb00c89b3e54832f138bccc3422bd8652e8cda4d74d1fa\",\"arguments\":[\"-q\",\"--lrat\",\"--binary=false\",\"-c\",\"100\"]},\n  \"proof_text_cap_bytes\": 2097152,\n  \"entries\": [\n    {\"dimacs_sha256\":\"aaaa"));
    assert!(text.ends_with("\"reason\":\"proof-over-cap\",\"lrat_bytes\":3407112}\n  ]\n}\n"));
    assert!(text.contains("\"label\":\"unsat\",\"lrat_sha256\":\"b18ef7200d397b899a01934c29486e5aa72bb28fe7925b593d18d2c7c5a48603\",\"lrat_bytes\":389,\"lrat\":\"69 -13 -15 -16 0 42 54 40 46 28 12 0\\n70 "));
    assert!(text.contains("\"label\":\"sat\",\"model\":[-1,2,-3]}"));
    assert!(text.contains("\"label\":\"unknown\",\"reason\":\"conflict-limit\"}"));
    assert_eq!(text.matches('\n').count(), 12);
    assert_eq!(read_answers_bytes(text.as_bytes()).expect("read"), file);
    let path = scratch("shapes-oracle.json");
    write_answers(&path, &file).expect("write");
    assert_eq!(std::fs::read(&path).expect("bytes"), text.as_bytes());
    assert_eq!(read_answers(&path).expect("read"), file);
    assert_eq!(
        timing_path(&path).file_name().and_then(|n| n.to_str()),
        Some("shapes-timing.json")
    );
    let empty = AnswerFile {
        entries: BTreeMap::new(),
        ..file.clone()
    };
    let text = serialize_answers(&empty).expect("empty");
    assert!(text.ends_with("  \"entries\": []\n}\n"));
    assert_eq!(read_answers_bytes(text.as_bytes()).expect("read"), empty);
    assert!(matches!(
        read_answers(Path::new("artifacts/does-not-exist-oracle.json")),
        Err(OracleError::Missing(_))
    ));
}

#[test]
fn answer_file_rejections() {
    let text = serialize_answers(&four_shapes()).expect("serialize");
    let sat_line = text
        .lines()
        .find(|line| line.contains("\"label\":\"sat\""))
        .expect("sat line")
        .trim_end_matches(',')
        .to_owned();
    let duplicate = text.replacen(&sat_line, &format!("{sat_line},\n{sat_line}"), 1);
    let result = read_answers_bytes(duplicate.as_bytes());
    assert!(
        matches!(&result, Err(OracleError::AnswerDuplicate { dimacs_sha256 }) if *dimacs_sha256 == "a".repeat(64)),
        "{result:?}"
    );
    let identity = text.replacen("\"version\":\"3.0.1\"", "\"version\":\"3.0.0\"", 1);
    assert!(matches!(
        read_answers_bytes(identity.as_bytes()),
        Err(OracleError::AnswerIdentity { field, .. }) if field == "version"
    ));
    let digest = text.replacen("601c9fa8", "601c9fa9", 1);
    assert!(matches!(
        read_answers_bytes(digest.as_bytes()),
        Err(OracleError::AnswerIdentity { field, .. }) if field == "sha256"
    ));
    let cap = text.replacen(
        "\"proof_text_cap_bytes\": 2097152",
        "\"proof_text_cap_bytes\": 1",
        1,
    );
    assert!(matches!(
        read_answers_bytes(cap.as_bytes()),
        Err(OracleError::AnswerIdentity { field, .. }) if field == "proof_text_cap_bytes"
    ));
    let arguments = text.replacen("\"--binary=false\"", "\"--binary=true\"", 1);
    assert!(matches!(
        read_answers_bytes(arguments.as_bytes()),
        Err(OracleError::AnswerIdentity { field, .. }) if field == "arguments"
    ));
    let order = text.replacen("\"model\":[-1,2,-3]", "\"model\":[2,-1,-3]", 1);
    assert!(matches!(
        read_answers_bytes(order.as_bytes()),
        Err(OracleError::AnswerFormat(_))
    ));
    let incomplete = text.replacen("\"model\":[-1,2,-3]", "\"model\":[-1,2]", 1);
    assert!(matches!(
        read_answers_bytes(incomplete.as_bytes()),
        Err(OracleError::AnswerFormat(_))
    ));
    let sha = text.replacen("b18ef7200d39", "b18ef7200d3a", 1);
    assert!(matches!(
        read_answers_bytes(sha.as_bytes()),
        Err(OracleError::AnswerFormat(reason)) if reason.contains("lrat_sha256")
    ));
    let bytes = text.replacen("\"lrat_bytes\":389", "\"lrat_bytes\":388", 1);
    assert!(matches!(
        read_answers_bytes(bytes.as_bytes()),
        Err(OracleError::AnswerFormat(reason)) if reason.contains("lrat_bytes")
    ));
    let charset = text.replacen("\"lrat\":\"69 ", "\"lrat\":\"69 x", 1);
    assert!(matches!(
        read_answers_bytes(charset.as_bytes()),
        Err(OracleError::AnswerFormat(_))
    ));
    let loose = text.replacen("\"schema_version\": 1", "\"schema_version\":  1", 1);
    assert!(matches!(
        read_answers_bytes(loose.as_bytes()),
        Err(OracleError::AnswerFormat(reason)) if reason.contains("fixed serialization")
    ));
    let escape = text.replacen("\\n70 ", "\\t70 ", 1);
    assert!(matches!(
        read_answers_bytes(escape.as_bytes()),
        Err(OracleError::AnswerFormat(_))
    ));
    let float = text.replacen(
        "\"conflict_limit\":100,\"label\":\"sat\"",
        "\"conflict_limit\":100.0,\"label\":\"sat\"",
        1,
    );
    assert!(matches!(
        read_answers_bytes(float.as_bytes()),
        Err(OracleError::AnswerFormat(_))
    ));
    let limit = text.replacen(
        "\"conflict_limit\":100,\"label\":\"sat\"",
        "\"conflict_limit\":99,\"label\":\"sat\"",
        1,
    );
    assert!(matches!(
        read_answers_bytes(limit.as_bytes()),
        Err(OracleError::AnswerLimit {
            expected: 100,
            actual: 99
        })
    ));
    let over = vec![b' '; usize::try_from(ANSWER_FILE_CAP_BYTES).expect("cap") + 1];
    assert!(matches!(
        read_answers_bytes(&over),
        Err(OracleError::FileCap { bytes }) if bytes == ANSWER_FILE_CAP_BYTES + 1
    ));
    let mut wrong_identity = four_shapes();
    wrong_identity.identity.path = "/usr/local/bin/cadical".into();
    assert!(matches!(
        serialize_answers(&wrong_identity),
        Err(OracleError::AnswerIdentity { field, .. }) if field == "path"
    ));
    assert_eq!(FIXED_ARGUMENTS, ["-q", "--lrat", "--binary=false"]);
    assert_eq!(CADICAL.version, "3.0.1");
}

// ---------------------------------------------------------------------------
// Labelling
// ---------------------------------------------------------------------------

#[test]
fn label_uses_the_truth_table_up_to_twelve_variables() {
    let answers = demo_file();
    let mut used = BTreeSet::new();
    let input: Cnf = vec![vec![1, 2], vec![-1, 2]];
    match label(&input, 12, &DEMO_LIMITS, &answers, &mut used).expect("label") {
        Label::TruthTable(reference) => {
            assert_eq!(reference.model_count, 2048);
            assert_eq!(reference.backbone, Some(vec![2]));
            assert_eq!(reference.work.assignments, 4096);
        }
        other => panic!("{other:?}"),
    }
    assert!(used.is_empty());
    assert_eq!(TRUTH_TABLE_MAX_VARIABLES, 12);
}

#[test]
fn label_decides_definition_cases_before_any_lookup() {
    let answers = AnswerFile {
        experiment: "none".into(),
        identity: Identity::pinned(),
        conflict_limit: 100,
        entries: BTreeMap::new(),
    };
    let mut used = BTreeSet::new();
    let input: Cnf = vec![vec![1, 2, 3], vec![-4, 5], vec![], vec![13]];
    let label13 = label(&input, 13, &DEMO_LIMITS, &answers, &mut used).expect("definition");
    match &label13 {
        Label::Definition(Definition::EmptyClause { clause_index, work }) => {
            assert_eq!(*clause_index, 2);
            // One formula check, then one clause read and one length read for
            // each of the three clauses scanned.
            assert_eq!(
                (work.formula_checks, work.clause_reads, work.literal_reads),
                (1, 3, 3)
            );
            assert_eq!(work.work_units, 7);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(label13.sat(), Some(false));
    assert!(label13.json().starts_with("{\"kind\":\"definition\",\"label\":\"unsat\",\"certificate\":{\"kind\":\"empty-clause\",\"clause_index\":2},\"work\":"));
    let none = label(&vec![], 13, &DEMO_LIMITS, &answers, &mut used).expect("no clauses");
    match &none {
        Label::Definition(Definition::NoClauses { model, model_check }) => {
            assert_eq!(model, &(1..=13).map(|v| -v).collect::<Vec<i32>>());
            assert!(model_check.valid);
            let w = &model_check.work;
            assert_eq!(
                (
                    w.formula_checks,
                    w.clause_writes,
                    w.literal_writes,
                    w.work_units
                ),
                (1, 1, 13, 15)
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(none.sat(), Some(true));
    assert!(used.is_empty(), "definition cases touch no answer");
}

#[test]
fn label_reports_missing_limit_and_unused_answers() {
    let answers = demo_file();
    let mut used = BTreeSet::new();
    let (r60, n) = parse_dimacs(R60).expect("r60");
    let failure = label(&r60, n, &DEMO_LIMITS, &answers, &mut used).expect_err("missing");
    let digest = digest_of(&r60, n);
    assert!(matches!(&failure, Failure::Oracle { dimacs_sha256, .. } if *dimacs_sha256 == digest));
    assert!(oracle_reason(&failure).contains("no answer for formula"));
    let (r16, n) = parse_dimacs(R16).expect("r16");
    let failure =
        label(&r16, n, &Limits { conflicts: 200 }, &answers, &mut used).expect_err("limit");
    assert!(oracle_reason(&failure).contains("protocol names 200, answer carries 100"));
    assert_eq!(
        used.len(),
        1,
        "the digest is marked used before verification"
    );
    let (r30, n30) = parse_dimacs(R30).expect("r30");
    let verdict = match label(&r30, n30, &DEMO_LIMITS, &answers, &mut used).expect("sat") {
        Label::Oracle(verdict) => verdict,
        other => panic!("{other:?}"),
    };
    assert!(matches!(verdict.verdict, Verdict::Sat { .. }));
    assert_eq!(verdict.label(), "sat");
    let unused = check_used(&answers, &used).expect_err("s60 unused");
    let (s60, n60) = parse_dimacs(S60).expect("s60");
    assert!(
        matches!(unused, OracleError::AnswerUnused { dimacs_sha256 } if dimacs_sha256 == digest_of(&s60, n60))
    );
    used.insert(digest_of(&s60, n60));
    check_used(&answers, &used).expect("all used");
}

#[test]
fn verify_is_pure_and_rejects_mismatched_entries() {
    let answers = demo_file();
    let (r16, n) = parse_dimacs(R16).expect("r16");
    let entry = &answers.entries[&digest_of(&r16, n)];
    let verdict = verify(&r16, n, entry, &DEMO_LIMITS).expect("unsat");
    match verdict.verdict {
        Verdict::Unsat { proof_check } => {
            assert_eq!(
                (
                    proof_check.lemmas,
                    proof_check.deletions,
                    proof_check.lrat_bytes
                ),
                (15, 2, 389)
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(verdict.identity, Identity::pinned());
    let (r30, _) = parse_dimacs(R30).expect("r30");
    assert!(matches!(
        verify(&r30, 30, entry, &DEMO_LIMITS),
        Err(OracleError::AnswerIdentity { field, .. }) if field == "dimacs_sha256"
    ));
    let mut wrong = entry.clone();
    wrong.variables = 17;
    assert!(matches!(
        verify(&r16, n, &wrong, &DEMO_LIMITS),
        Err(OracleError::AnswerIdentity { field, .. }) if field == "variables"
    ));
    // A model that violates a clause is `ModelInvalid` with the clause index.
    let (r30, n30) = parse_dimacs(R30).expect("r30");
    let sat = &answers.entries[&digest_of(&r30, n30)];
    let Answer::Sat { model } = &sat.answer else {
        panic!("sat")
    };
    let mut flipped = model.clone();
    flipped[0] = -flipped[0];
    let entry = AnswerEntry {
        answer: Answer::Sat { model: flipped },
        ..sat.clone()
    };
    assert!(matches!(
        verify(&r30, n30, &entry, &DEMO_LIMITS),
        Err(OracleError::ModelInvalid { .. })
    ));
    // A truncated proof is `ProofInvalid`, never unknown.
    let Answer::Unsat { lrat } = &answers.entries[&digest_of(&r16, n)].answer else {
        panic!("unsat")
    };
    let truncated = AnswerEntry {
        answer: Answer::Unsat {
            lrat: lrat.lines().take(3).map(|l| format!("{l}\n")).collect(),
        },
        ..answers.entries[&digest_of(&r16, n)].clone()
    };
    assert!(matches!(
        verify(&r16, n, &truncated, &DEMO_LIMITS),
        Err(OracleError::ProofInvalid { .. })
    ));
}

// ---------------------------------------------------------------------------
// Ratios
// ---------------------------------------------------------------------------

#[test]
fn ratio_pair_boundaries() {
    assert_eq!(ratio_pair(1, 0), None);
    let pair = ratio_pair(1, 2).expect("half");
    assert_eq!(
        (pair.numerator, pair.denominator, pair.thousandths.as_str()),
        (1, 2, "0.500")
    );
    assert_eq!(ratio_pair(1, 3).expect("third").thousandths, "0.333");
    assert_eq!(ratio_pair(2, 3).expect("two thirds").thousandths, "0.667");
    assert_eq!(
        ratio_pair(1, 16).expect("exact half rounds up").thousandths,
        "0.063"
    );
    assert_eq!(
        ratio_pair(3, 16).expect("exact half rounds up").thousandths,
        "0.188"
    );
    assert_eq!(ratio_pair(0, 7).expect("zero").thousandths, "0.000");
    assert_eq!(
        ratio_pair(u64::MAX, u64::MAX),
        None,
        "a * 1000 overflows u64"
    );
    assert_eq!(ratio_pair(u64::MAX, 1), None, "a * 1000 overflows u64");
    assert_eq!(
        ratio_pair(u64::MAX / 1000, 1)
            .expect("largest exact")
            .thousandths,
        "18446744073709551.000"
    );
    assert_eq!(ratio_pair(u64::MAX / 1000 + 1, 1), None);
    assert_eq!(
        ratio_pair(1_518_173, 124_045)
            .expect("twelvefold")
            .thousandths,
        "12.239"
    );
}

// ---------------------------------------------------------------------------
// Driver order and replay
// ---------------------------------------------------------------------------

#[test]
fn arms_complete_before_any_answer_lookup() {
    let answers = demo_file();
    let mut seen: Vec<String> = Vec::new();
    let rows = replay_cases(
        "demo",
        demo_corpus().expect("corpus"),
        &DEMO_LIMITS,
        &answers,
        |input, n| {
            seen.push(digest_of(input, n));
            Ok(seen.len())
        },
    )
    .expect("replay");
    assert_eq!(
        rows.iter().map(|row| row.arm).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        rows.iter().map(|row| row.label.kind()).collect::<Vec<_>>(),
        vec!["oracle"; 3]
    );
    // Remove the last case's answer: the arm recorder must still have run for
    // every case up to and including it before its lookup fails.
    let (s60, n) = parse_dimacs(S60).expect("s60");
    let mut partial = answers.clone();
    partial
        .entries
        .remove(&digest_of(&s60, n))
        .expect("s60 entry");
    let mut calls = 0;
    let failure = replay_cases(
        "demo",
        demo_corpus().expect("corpus"),
        &DEMO_LIMITS,
        &partial,
        |_, _| {
            calls += 1;
            Ok(())
        },
    )
    .expect_err("missing s60");
    assert_eq!(calls, 3);
    assert!(oracle_reason(&failure).contains("no answer"));
    // An extra committed answer is refused after the corpus.
    let mut extra = answers.clone();
    let (r60, n60) = parse_dimacs(R60).expect("r60");
    extra.entries.insert(
        digest_of(&r60, n60),
        AnswerEntry {
            dimacs_sha256: digest_of(&r60, n60),
            variables: n60,
            clauses: r60.len() as u64,
            conflict_limit: 100,
            answer: Answer::Unsat { lrat: P60.into() },
        },
    );
    let failure = replay_cases(
        "demo",
        demo_corpus().expect("corpus"),
        &DEMO_LIMITS,
        &extra,
        |_, _| Ok(()),
    )
    .expect_err("unused");
    assert!(
        matches!(&failure, Failure::Oracle { dimacs_sha256, .. } if *dimacs_sha256 == digest_of(&r60, n60))
    );
    assert!(oracle_reason(&failure).contains("not used"));
}

#[test]
fn demo_replay_reproduces_the_stored_artifact_without_a_solver() {
    let artifact = demo_replay(DEMO_ANSWERS).expect("replay");
    assert_eq!(artifact.as_bytes(), DEMO_ARTIFACT);
    assert!(artifact.contains(&format!("\"sha256\":\"{}\"", sha256_hex(DEMO_ANSWERS))));
    assert!(artifact.contains("\"oracle_sat\":1,\"oracle_unsat\":1,\"oracle_unknown\":1"));
    assert!(artifact.contains("\"reason\":\"conflict-limit\""));
    assert!(!artifact.contains("wall"), "no seconds in the artifact");
    // A one-byte change in the answer file changes the artifact's declared
    // digest, and a damaged proof stops the replay.
    let text = String::from_utf8(DEMO_ANSWERS.to_vec()).expect("utf8");
    let damaged = text.replacen("83 0 82 79 75 51 0", "83 0 82 79 75 15 0", 1);
    let failure = demo_replay(damaged.as_bytes()).expect_err("digest disagrees");
    assert!(oracle_reason(&failure).contains("lrat_sha256"));
    // With its digest updated, the damaged proof is well-formed and fails
    // in the checker on its last lemma.
    let original = include_str!("lrat/p16-1.lrat");
    let edited = original.replacen("83 0 82 79 75 51 0", "83 0 82 79 75 15 0", 1);
    let consistent = damaged.replacen(
        &sha256_hex(original.as_bytes()),
        &sha256_hex(edited.as_bytes()),
        1,
    );
    let failure = demo_replay(consistent.as_bytes()).expect_err("damaged proof");
    let reason = oracle_reason(&failure);
    assert!(reason.contains("proof invalid at line 17"), "{reason}");
}

// ---------------------------------------------------------------------------
// Live checks against the pinned binary
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires the pinned CaDiCaL 3.0.1 binary"]
fn live_identity_matches_the_pins() {
    let identity = identity().expect("pinned binary");
    assert_eq!(identity, Identity::pinned());
    assert_eq!(identity.sha256, CADICAL.sha256);
}

#[test]
#[ignore = "requires the pinned CaDiCaL 3.0.1 binary"]
fn live_pigeonhole_is_unsat_with_a_checked_proof_and_unknown_at_zero_conflicts() {
    let (php, n) = parse_dimacs(PHP87).expect("php87");
    assert_eq!((php.len(), n), (204, 56));
    let (entry, _) = solve(&php, n, &Limits { conflicts: 200_000 }).expect("solve");
    let Answer::Unsat { lrat } = &entry.answer else {
        panic!("expected unsat, got {:?}", entry.answer.label())
    };
    let check = check_lrat(&php, n, lrat).expect("valid proof");
    assert_eq!(
        (check.lemmas, check.deletions, check.lrat_bytes),
        (6891, 6110, 725_182)
    );
    let (entry, _) = solve(&php, n, &Limits { conflicts: 0 }).expect("solve");
    assert_eq!(
        entry.answer,
        Answer::Unknown {
            reason: UnknownReason::ConflictLimit
        }
    );
}

#[test]
#[ignore = "requires the pinned CaDiCaL 3.0.1 binary"]
fn live_sat_model_passes_check_model_and_solving_is_deterministic() {
    let (r30, n) = parse_dimacs(R30).expect("r30");
    let (first, _) = solve(&r30, n, &DEMO_LIMITS).expect("solve");
    let Answer::Sat { model } = &first.answer else {
        panic!("expected sat")
    };
    assert!(check_model(&r30, n, model).expect("checked").valid);
    let (second, _) = solve(&r30, n, &DEMO_LIMITS).expect("solve again");
    assert_eq!(first, second);
    let (r16, n16) = parse_dimacs(R16).expect("r16");
    let (a, _) = solve(&r16, n16, &DEMO_LIMITS).expect("solve");
    let (b, _) = solve(&r16, n16, &DEMO_LIMITS).expect("solve again");
    assert_eq!(a, b);
    assert_eq!(
        a.answer,
        Answer::Unsat {
            lrat: include_str!("lrat/p16-1.lrat").into()
        }
    );
    // The committed demo answer file is exactly what the binary answers today.
    let demo = demo_file();
    assert_eq!(demo.entries[&a.dimacs_sha256], a);
    assert_eq!(demo.entries[&first.dimacs_sha256], first);
}

#[test]
#[ignore = "requires the pinned CaDiCaL 3.0.1 binary"]
fn live_empty_clause_input_is_decided_by_definition_not_sent() {
    // The binary answers such inputs with an empty proof file, which the
    // checker would reject; the definition path must therefore come first.
    let input: Cnf = vec![vec![1, 2], vec![], vec![-13]];
    let empty = AnswerFile {
        experiment: "none".into(),
        identity: Identity::pinned(),
        conflict_limit: 100,
        entries: BTreeMap::new(),
    };
    let mut used = BTreeSet::new();
    let label = label(&input, 13, &DEMO_LIMITS, &empty, &mut used).expect("definition");
    assert!(matches!(
        label,
        Label::Definition(Definition::EmptyClause {
            clause_index: 1,
            ..
        })
    ));
    // Sent anyway, the binary's empty proof is refused by the checker and the
    // scratch files are kept and named.
    match solve(&input, 13, &DEMO_LIMITS).expect_err("empty proof is refused") {
        OracleError::Kept { cnf, lrat, cause } => {
            assert!(
                matches!(*cause, OracleError::ProofInvalid { .. }),
                "{cause}"
            );
            assert_eq!(std::fs::read(&lrat).expect("kept proof"), b"");
            std::fs::remove_file(cnf).expect("kept cnf");
            std::fs::remove_file(lrat).expect("kept lrat");
        }
        other => panic!("{other}"),
    }
}

#[test]
fn answer_file_for_another_experiment_is_refused() {
    let mut answers = demo_file();
    answers.experiment = "other".to_string();
    let failure = replay_cases(
        "demo",
        demo_corpus().expect("corpus"),
        &DEMO_LIMITS,
        &answers,
        |_, _| Ok(()),
    )
    .expect_err("the answer file must name the experiment being replayed");
    assert!(format!("{failure:?}").contains("experiment"), "{failure:?}");
}
