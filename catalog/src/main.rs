//! `cinelens-catalog --out catalog.json.gz --today YYYY-MM-DD --recent-since YYYY-MM-DD [--extra-ids FILE] [--omdb-cache FILE] [--previous FILE --delta-dir DIR]`
//!
//! Reads the TMDB API Read Access Token from `TMDB_TOKEN` and the OMDb key from `OMDB_KEY`
//! (without it, no critics' scores are fetched). `--extra-ids` is a file with one TMDB id per
//! line (the training data); blank and non-numeric lines are ignored. `--omdb-cache` is a JSON
//! file of what OMDb said in earlier runs, read if it exists and written back. `--previous` is
//! the catálogo published before (gzipped); if it exists, the delta from it to the new one is
//! written to `--delta-dir` as `catalog-delta-<its data version>.json.gz`.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::time::Duration;

use cinelens_catalog::{build, Config};
use serde_json::Value;

const API: &str = "https://api.themoviedb.org/3";
const OMDB_API: &str = "https://www.omdbapi.com/";
/// OMDb's free keys allow 1000 a day; the rest is left for retries and manual runs.
const OMDB_BUDGET: usize = 900;
const ATTEMPTS: u32 = 6;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };
    let required = |name: &str| arg(name).ok_or_else(|| format!("missing {name}"));
    let out = required("--out")?;
    let extra_ids: Vec<u32> = match arg("--extra-ids") {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| format!("{path}: {e}"))?
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect(),
        None => Vec::new(),
    };
    let omdb_key = std::env::var("OMDB_KEY").ok().filter(|k| !k.is_empty());
    if omdb_key.is_none() {
        eprintln!("::warning::OMDB_KEY is not set: no new critics' scores, see docs/guides/omdb.md");
    }
    let config = Config {
        today: required("--today")?,
        recent_since: required("--recent-since")?,
        extra_ids: &extra_ids,
        omdb_budget: if omdb_key.is_some() { OMDB_BUDGET } else { 0 },
    };
    let cache_path = arg("--omdb-cache");
    let mut critica = match cache_path.map(std::fs::read) {
        Some(Ok(bytes)) => serde_json::from_slice(&bytes).map_err(|e| format!("OMDb cache: {e}"))?,
        Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("OMDb cache: {e}")),
        _ => BTreeMap::new(),
    };

    let token = std::env::var("TMDB_TOKEN").map_err(|_| "TMDB_TOKEN is not set")?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .into();
    let fetch = |request: &str| {
        let call = || agent.get(format!("{API}{request}")).header("Authorization", format!("Bearer {token}")).call();
        get(request, false, call)
    };
    let omdb = |imdb_id: &str| {
        let key = omdb_key.as_deref().unwrap_or_default();
        get(imdb_id, true, || agent.get(OMDB_API).query("apikey", key).query("i", imdb_id).call())
    };

    let artifact = build(&fetch, &omdb, &mut critica, &config)?;
    if let Some(path) = cache_path {
        std::fs::write(path, serde_json::to_vec(&critica).map_err(|e| e.to_string())?).map_err(|e| format!("{path}: {e}"))?;
    }
    let json = serde_json::to_vec(&artifact).map_err(|e| e.to_string())?;
    write_gz(out, &json)?;
    if let (Some(previous), Some(dir)) = (arg("--previous"), arg("--delta-dir")) {
        write_delta(previous, dir, &json)?;
    }
    Ok(())
}

fn write_gz(path: &str, bytes: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::best());
    gz.write_all(bytes).and_then(|_| gz.finish()).and_then(|mut f| f.flush()).map_err(|e| format!("{path}: {e}"))
}

/// The delta from the `previous` catálogo (gzipped; none on the first run) to `new`.
fn write_delta(previous: &str, dir: &str, new: &[u8]) -> Result<(), String> {
    let gz = match std::fs::read(previous) {
        Ok(gz) => gz,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("{previous}: {e}")),
    };
    let mut old = Vec::new();
    flate2::read::GzDecoder::new(&gz[..]).read_to_end(&mut old).map_err(|e| format!("{previous}: {e}"))?;
    let delta = cinelens_core::catalog_delta(&old, new).map_err(|e| format!("{previous}: {e}"))?;
    let version = |bytes: &[u8]| -> Option<String> {
        Some(serde_json::from_slice::<Value>(bytes).ok()?.get("data_version")?.as_str()?.to_string())
    };
    let from = version(&old).ok_or_else(|| format!("{previous}: no data version"))?;
    // A same-day re-run: a delta from today to today would send the app round in circles.
    if Some(&from) == version(new).as_ref() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
    write_gz(&format!("{dir}/catalog-delta-{from}.json.gz"), &delta)
}

/// GET (`call`, named `request` in errors) with retries on rate limiting, server errors and
/// network failures. `body_on_401` for OMDb, which says in a 401's body why: a bad key or no
/// requests left today.
fn get(request: &str, body_on_401: bool, call: impl Fn() -> Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Option<Value>, String> {
    let mut last = String::new();
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(500 << attempt));
        }
        match call() {
            Ok(mut response) => match response.status().as_u16() {
                status if status == 200 || (status == 401 && body_on_401) => {
                    let body = response.body_mut().read_to_string().map_err(|e| format!("{request}: {e}"))?;
                    return serde_json::from_str(&body).map(Some).map_err(|e| format!("{request}: {e}"));
                }
                404 => return Ok(None),
                429 | 500..=599 => last = format!("HTTP {}", response.status()),
                status => return Err(format!("{request}: HTTP {status}")),
            },
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("{request}: {last} after {ATTEMPTS} attempts"))
}
