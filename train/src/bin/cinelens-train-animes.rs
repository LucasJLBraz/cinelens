//! `cinelens-train-animes --ratings anime_user_ratings.csv --titles anime_titles.csv
//!  --mapping anime-list-full.json --animes animes.json.gz --out anime-model.json.gz --data-version YYYY-MM-DD`
//!
//! Reads the MyAnimeList dump and writes the gzipped anime model: factors only (ADR 0004). The
//! animes' catálogo gives the content mapping.

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
    let required = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str).ok_or_else(|| format!("missing {name}"));
    let open = |path: &str| File::open(path).map(BufReader::new).map_err(|e| format!("{path}: {e}"));
    let out = required("--out")?;
    let mapping = cinelens_train::read_maybe_gzipped(required("--mapping")?)?;
    let mapping = cinelens_core::read_mapeamento(&mapping)?;
    let animes = cinelens_train::read_maybe_gzipped(required("--animes")?)?;
    let content = cinelens_core::content_features(&animes).map_err(|e| e.to_string())?;
    let model = cinelens_train::train_animes(open(required("--ratings")?)?, open(required("--titles")?)?, &mapping, &content, required("--data-version")?)?;
    let file = File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::best());
    serde_json::to_writer(&mut gz, &model).map_err(|e| e.to_string())?;
    gz.finish().and_then(|mut f| f.flush()).map_err(|e| format!("{out}: {e}"))
}
