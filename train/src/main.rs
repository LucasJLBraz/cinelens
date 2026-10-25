//! `cinelens-train --ratings ratings.csv --links links.csv --out model.json.gz --data-version YYYY-MM-DD`
//!
//! Reads MovieLens `ml-latest` and writes the gzipped model artifact.

use std::fs::File;
use std::io::{BufReader, Write};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let required = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
            .ok_or_else(|| format!("missing {name}"))
    };
    let open = |path: &str| File::open(path).map(BufReader::new).map_err(|e| format!("{path}: {e}"));
    let out = required("--out")?;

    let model = cinelens_train::train(open(required("--ratings")?)?, open(required("--links")?)?, required("--data-version")?)?;
    let file = File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::best());
    serde_json::to_writer(&mut gz, &model).map_err(|e| e.to_string())?;
    gz.finish().and_then(|mut f| f.flush()).map_err(|e| format!("{out}: {e}"))?;
    Ok(())
}
