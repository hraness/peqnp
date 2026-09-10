use std::{env, fs, path::PathBuf};

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str);
    if !matches!(command, Some("experiment" | "transfer")) || args.len() > 2 {
        return Err("usage: peqnp <experiment|transfer> [output.json]".into());
    }
    let path = PathBuf::from(args.get(1).map(String::as_str).unwrap_or(
        if command == Some("transfer") {
            "artifacts/clue-transfer.json"
        } else {
            "artifacts/calibration.json"
        },
    ));
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = if command == Some("transfer") {
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
