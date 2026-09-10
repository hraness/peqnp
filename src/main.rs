use std::{env, fs, path::PathBuf};

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("experiment") || args.len() > 2 {
        return Err("usage: peqnp experiment [output.json]".into());
    }
    let path = PathBuf::from(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("artifacts/calibration.json"),
    );
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let records = peqnp::calibration()?;
    let json = peqnp::result_json(&records)?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    println!(
        "Checked 8 candidates × 65536 formulas; unique survivor: {}",
        peqnp::UNIT_RULE
    );
    println!(
        "Rejected false completeness claim; wrote {}",
        path.display()
    );
    Ok(())
}

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
