use std::{env, fs, path::PathBuf};

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str);
    if !matches!(
        command,
        Some("experiment" | "transfer" | "indexed" | "implication")
    ) || args.len() > 2
    {
        return Err("usage: peqnp <experiment|transfer|indexed|implication> [output.json]".into());
    }
    let path = PathBuf::from(args.get(1).map(String::as_str).unwrap_or(match command {
        Some("transfer") => "artifacts/clue-transfer.json",
        Some("indexed") => "artifacts/indexed-transfer.json",
        Some("implication") => "artifacts/implication-calibration.json",
        _ => "artifacts/calibration.json",
    }));
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = if command == Some("indexed") {
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
