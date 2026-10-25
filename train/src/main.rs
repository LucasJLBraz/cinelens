//! `cinelens-train --ratings ratings.csv --links links.csv --out model.json.gz --data-version YYYY-MM-DD
//!  [--catalog catalog.json.gz [--letterboxd letterboxd_interactions.csv]]`
//!
//! Reads MovieLens `ml-latest` (and the Letterboxd sample, joined through the catálogo's IMDb
//! ids) and writes the gzipped model artifact. The catálogo also gives the content mapping.

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
    let optional = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let required = |name: &str| optional(name).ok_or_else(|| format!("missing {name}"));
    let open = |path: &str| File::open(path).map(BufReader::new).map_err(|e| format!("{path}: {e}"));
    let out = required("--out")?;

    let catalog = optional("--catalog").map(cinelens_train::read_maybe_gzipped).transpose()?;
    let content = match &catalog {
        Some(catalog) => cinelens_core::content_features(catalog).map_err(|e| e.to_string())?,
        None => Vec::new(),
    };
    let letterboxd = match optional("--letterboxd") {
        None => None,
        Some(path) => {
            let catalog = catalog.as_deref().ok_or("--letterboxd needs --catalog")?;
            Some(cinelens_train::Letterboxd {
                interactions: Box::new(open(path)?),
                imdb_to_tmdb: cinelens_train::catalog_imdb_ids(catalog)?,
            })
        }
    };
    let model = cinelens_train::train(
        open(required("--ratings")?)?,
        open(required("--links")?)?,
        letterboxd,
        &content,
        required("--data-version")?,
    )?;
    let file = File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::best());
    serde_json::to_writer(&mut gz, &model).map_err(|e| e.to_string())?;
    gz.finish().and_then(|mut f| f.flush()).map_err(|e| format!("{out}: {e}"))?;
    Ok(())
}
