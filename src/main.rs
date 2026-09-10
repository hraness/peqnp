use std::{env, fs, path::PathBuf};

const USAGE: &str = "usage: peqnp <experiment|transfer|indexed|implication|fragment|extraction|demo> [output.json] [--oracle-answers <path>]\n       peqnp oracle\n       peqnp oracle-answers <experiment> [path]";

const REPLAY_COMMANDS: [&str; 7] = [
    "experiment",
    "transfer",
    "indexed",
    "implication",
    "fragment",
    "extraction",
    "demo",
];

/// Positional arguments plus the one replay option, `--oracle-answers <path>`.
fn parse_arguments(args: &[String]) -> Result<(Vec<&str>, Option<PathBuf>), String> {
    let mut positional = Vec::new();
    let mut oracle_answers = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--oracle-answers" {
            let path = iter
                .next()
                .ok_or_else(|| format!("--oracle-answers requires a path\n{USAGE}"))?;
            if oracle_answers.replace(PathBuf::from(path)).is_some() {
                return Err(format!("--oracle-answers given twice\n{USAGE}"));
            }
        } else if arg.starts_with("--") {
            return Err(format!("unknown option {arg}\n{USAGE}"));
        } else {
            positional.push(arg.as_str());
        }
    }
    Ok((positional, oracle_answers))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let (positional, oracle_answers) = parse_arguments(&args)?;
    let command = positional.first().copied();
    match command {
        Some("oracle") => {
            if positional.len() != 1 || oracle_answers.is_some() {
                return Err(USAGE.into());
            }
            let identity =
                peqnp::oracle::identity().map_err(|e| format!("oracle preflight failed: {e}"))?;
            println!("{}", identity.json());
            return Ok(());
        }
        Some("oracle-answers") => {
            let name = positional.get(1).copied().ok_or(USAGE)?;
            if positional.len() > 3 || oracle_answers.is_some() {
                return Err(USAGE.into());
            }
            let path = PathBuf::from(
                positional
                    .get(2)
                    .map(|p| (*p).to_owned())
                    .unwrap_or_else(|| peqnp::oracle::answers_path(name)),
            );
            let result = peqnp::oracle::regenerate(name, &path)
                .map_err(|e| format!("oracle regeneration failed: {e}"))?;
            println!(
                "Regenerated {} oracle answers for {name}: sat {}, unsat {}, unknown {}; solver wall time {} ms recorded in {}.",
                result.entries,
                result.sat,
                result.unsat,
                result.unknown,
                result.total_wall_milliseconds,
                result.timing_path.display()
            );
            println!("Wrote {}", result.answers_path.display());
            return Ok(());
        }
        Some(name) if REPLAY_COMMANDS.contains(&name) => {}
        _ => return Err(USAGE.into()),
    }
    if positional.len() > 2 {
        return Err(USAGE.into());
    }
    if oracle_answers.is_some() && command != Some("demo") {
        return Err(format!(
            "{} does not read an oracle-answer file\n{USAGE}",
            command.unwrap_or_default()
        ));
    }
    let path = PathBuf::from(positional.get(1).copied().unwrap_or(match command {
        Some("transfer") => "artifacts/clue-transfer.json",
        Some("indexed") => "artifacts/indexed-transfer.json",
        Some("implication") => "artifacts/implication-calibration.json",
        Some("fragment") => "artifacts/fragment-interface.json",
        Some("extraction") => "artifacts/extraction-cost.json",
        Some("demo") => "artifacts/demo-reference.json",
        _ => "artifacts/calibration.json",
    }));
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = if command == Some("demo") {
        let answers =
            oracle_answers.unwrap_or_else(|| PathBuf::from(peqnp::oracle::answers_path("demo")));
        let bytes = fs::read(&answers).map_err(|e| {
            format!(
                "oracle-answer file {} cannot be read ({e}); regenerate it with `cargo run --locked --release -- oracle-answers demo`",
                answers.display()
            )
        })?;
        let artifact =
            peqnp::oracle::demo_replay(&bytes).map_err(|e| format!("demo replay failed: {e:?}"))?;
        println!("{}", peqnp::oracle::demo_summary_line(&artifact));
        artifact
    } else if command == Some("extraction") {
        let result =
            peqnp::extraction::experiment().map_err(|e| format!("extraction failed: {e:?}"))?;
        println!("{}", peqnp::extraction::summary_line(&result));
        peqnp::extraction::json(&result)
    } else if command == Some("fragment") {
        let result =
            peqnp::fragment::experiment().map_err(|e| format!("fragment failed: {e:?}"))?;
        println!("{}", peqnp::fragment::summary_line(&result));
        peqnp::fragment::json(&result)
    } else if command == Some("indexed") {
        let result = peqnp::indexed::experiment().map_err(|e| format!("indexed failed: {e:?}"))?;
        println!(
            "Compiled {} rule instances into {} schema; checked {} fresh cases. Baseline: {}; generic: {}; indexed: {}.",
            result.compilation.library.source_rule_instances(),
            result.compilation.library.schemas(),
            result.summary.cases,
            result.summary.baseline.total_work_units,
            result.summary.generic.total_work_units,
            result.summary.indexed.total_work_units
        );
        peqnp::indexed::json(&result)
    } else if command == Some("implication") {
        let result =
            peqnp::implication::experiment().map_err(|e| format!("implication failed: {e:?}"))?;
        println!("{}", peqnp::implication::summary_line(&result));
        peqnp::implication::json(&result)
    } else if command == Some("transfer") {
        let result =
            peqnp::transfer::experiment().map_err(|e| format!("transfer failed: {e:?}"))?;
        println!("Mined {} rule instances; checked {} held-out cases. Baseline work: {}; transfer work: {}.",
            result.mining.library.rules().len(), result.summary.cases,
            result.summary.baseline_work_units, result.summary.transfer_work_units);
        peqnp::transfer::json(&result).map_err(|e| format!("report failed: {e:?}"))?
    } else {
        let records = peqnp::calibration()?;
        println!(
            "Checked 8 candidates × 65536 formulas; unique survivor: {}",
            peqnp::UNIT_RULE
        );
        peqnp::result_json(&records)?
    };
    fs::write(&path, json).map_err(|e| e.to_string())?;
    println!("Wrote {}", path.display());
    Ok(())
}

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
