//! `cinelens-catalog --out catalog.json.gz --today YYYY-MM-DD --recent-since YYYY-MM-DD [--extra-ids FILE]`
//!
//! Reads the TMDB API Read Access Token from `TMDB_TOKEN`. `--extra-ids` is a file with one
//! TMDB id per line (the training data); blank and non-numeric lines are ignored.

use std::io::Write;
use std::time::Duration;

use cinelens_catalog::{build, Config};
use serde_json::Value;

const API: &str = "https://api.themoviedb.org/3";
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
    let config = Config { today: required("--today")?, recent_since: required("--recent-since")?, extra_ids: &extra_ids };

    let token = std::env::var("TMDB_TOKEN").map_err(|_| "TMDB_TOKEN is not set")?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .into();
    let fetch = |request: &str| tmdb(&agent, &token, request);

    let artifact = build(&fetch, &config)?;
    let file = std::fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::best());
    serde_json::to_writer(&mut gz, &artifact).map_err(|e| e.to_string())?;
    gz.finish().and_then(|mut f| f.flush()).map_err(|e| format!("{out}: {e}"))?;
    Ok(())
}

/// GET with retries on rate limiting, server errors and network failures.
fn tmdb(agent: &ureq::Agent, token: &str, request: &str) -> Result<Option<Value>, String> {
    let mut last = String::new();
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(500 << attempt));
        }
        match agent.get(format!("{API}{request}")).header("Authorization", format!("Bearer {token}")).call() {
            Ok(mut response) => match response.status().as_u16() {
                200 => {
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
