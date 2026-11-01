//! `cinelens-catalog --out catalog.json.gz --today YYYY-MM-DD --recent-since YYYY-MM-DD [--extra-ids FILE] [--omdb-cache FILE] [--previous FILE]`
//! `cinelens-catalog --series-up-to BYTES [--data-version VERSION] --out series.json.gz --today YYYY-MM-DD --recent-since YYYY-MM-DD`
//!
//! `cinelens-catalog --animes-up-to BYTES --out animes.json.gz --today YYYY-MM-DD --recent-since YYYY-MM-DD`
//!
//! `--series-up-to` and `--animes-up-to` build those assets instead, raising the vote cut until
//! each fits in BYTES. `--data-version` replaces the séries' version, which is otherwise `--today`:
//! an app downloads séries only when the version changes. Both read `Fribb/anime-lists`, and animes also AniList's popularity.
//! Reads the TMDB API Read Access Token from `TMDB_TOKEN` and the OMDb key from `OMDB_KEY`
//! (without it, no critics' scores are fetched). `--extra-ids` is a file with one TMDB id per
//! line (the training data); blank and non-numeric lines are ignored. `--omdb-cache` is a JSON
//! file of what OMDb said in earlier runs, read if it exists and written back. `--previous` is the
//! `catalog.json.gz` last published, whose details the filmes reuse; without a usable one, every
//! filme is fetched.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::time::Duration;

use cinelens_catalog::{build_animes, build_series, build_incremental, AniList, Anterior, Artifact, Config, ANILIST_MIN};
use serde_json::Value;

const API: &str = "https://api.themoviedb.org/3";
const OMDB_API: &str = "https://www.omdbapi.com/";
/// OMDb's free keys allow 1000 a day; the rest is left for retries and manual runs.
const OMDB_BUDGET: usize = 900;
const ATTEMPTS: u32 = 6;
const ANIME_LISTS: &str = "https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-full.json";
const ANILIST_API: &str = "https://graphql.anilist.co";
/// AniList allows 30 requests a minute.
const ANILIST_PAUSE: Duration = Duration::from_millis(2100);
const ANILIST_QUERY: &str = "query($page:Int){Page(page:$page,perPage:50){media(type:ANIME,isAdult:false,format_in:[TV,TV_SHORT,ONA],sort:POPULARITY_DESC){id popularity}}}";

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
    let series_up_to = arg("--series-up-to").map(|b| b.parse::<usize>().map_err(|e| format!("--series-up-to: {e}"))).transpose()?;
    let animes_up_to = arg("--animes-up-to").map(|b| b.parse::<usize>().map_err(|e| format!("--animes-up-to: {e}"))).transpose()?;
    let omdb_key = std::env::var("OMDB_KEY").ok().filter(|k| !k.is_empty());
    if omdb_key.is_none() && series_up_to.is_none() && animes_up_to.is_none() {
        eprintln!("::warning::OMDB_KEY is not set: no new critics' scores, see docs/guides/omdb.md");
    }
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

    let anilist = if series_up_to.is_some() || animes_up_to.is_some() {
        let mapping = get(ANIME_LISTS, false, || agent.get(ANIME_LISTS).call())?.ok_or("anime-lists: not found")?;
        let popularidade = if animes_up_to.is_some() { anilist_popularity(&agent)? } else { BTreeMap::new() };
        AniList::new(&serde_json::to_vec(&mapping).map_err(|e| e.to_string())?, &popularidade)?
    } else {
        AniList::default()
    };
    let config = Config {
        today: required("--today")?,
        recent_since: required("--recent-since")?,
        extra_ids: &extra_ids,
        omdb_budget: if omdb_key.is_some() { OMDB_BUDGET } else { 0 },
        anilist: &anilist,
    };

    let up_to = |nome: &str, mut artifact: Artifact, max: usize| {
        let size = |a: &Artifact| gz(&serde_json::to_vec(a).expect("artifact serializes")).map_or(usize::MAX, |g| g.len());
        let cut = artifact.raise_votes_to_fit(max, size);
        let bytes = gz(&serde_json::to_vec(&artifact).map_err(|e| e.to_string())?)?;
        println!("::notice::{nome}: {} no catálogo, corte de {cut} votos, {} bytes (limite {max})", artifact.obras(), bytes.len());
        std::fs::write(out, bytes).map_err(|e| format!("{out}: {e}"))
    };
    if let Some(max) = series_up_to {
        let mut series = build_series(&fetch, &config)?;
        if let Some(version) = arg("--data-version") {
            series.set_data_version(version);
        }
        return up_to("Séries", series, max);
    }
    if let Some(max) = animes_up_to {
        return up_to("Animes", build_animes(&fetch, &config)?, max);
    }
    let anterior = arg("--previous").and_then(|path| match read_anterior(path) {
        Ok(anterior) => Some(anterior),
        Err(e) => {
            eprintln!("::warning::{path}: {e}; every filme is fetched");
            None
        }
    });
    let artifact = build_incremental(&fetch, &omdb, &mut critica, &config, anterior)?;
    if let Some(path) = cache_path {
        std::fs::write(path, serde_json::to_vec(&critica).map_err(|e| e.to_string())?).map_err(|e| format!("{path}: {e}"))?;
    }
    let json = serde_json::to_vec(&artifact).map_err(|e| e.to_string())?;
    std::fs::write(out, gz(&json)?).map_err(|e| format!("{out}: {e}"))
}

fn read_anterior(path: &str) -> Result<Anterior, String> {
    let gz = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut json = Vec::new();
    flate2::read::GzDecoder::new(gz.as_slice()).read_to_end(&mut json).map_err(|e| e.to_string())?;
    Anterior::new(&json)
}

/// AniList id → popularity, of the most popular down to `ANILIST_MIN`: what is below it
/// brings nothing in.
fn anilist_popularity(agent: &ureq::Agent) -> Result<BTreeMap<u32, u32>, String> {
    let mut popularidade = BTreeMap::new();
    for page in 1.. {
        std::thread::sleep(ANILIST_PAUSE);
        let body = serde_json::json!({"query": ANILIST_QUERY, "variables": {"page": page}}).to_string();
        let call = || agent.post(ANILIST_API).header("Content-Type", "application/json").send(body.as_str());
        let answer = get(ANILIST_API, false, call)?.ok_or("AniList: not found")?;
        let media = answer["data"]["Page"]["media"].as_array().ok_or("AniList: no media in the answer")?;
        let pares = media.iter().filter_map(|m| Some((m["id"].as_u64()? as u32, m["popularity"].as_u64()? as u32)));
        let pares: Vec<(u32, u32)> = pares.collect();
        popularidade.extend(pares.iter().copied());
        if pares.last().is_none_or(|&(_, p)| p < ANILIST_MIN) {
            break;
        }
    }
    Ok(popularidade)
}

fn gz(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    gz.write_all(bytes).and_then(|_| gz.finish()).map_err(|e| e.to_string())
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
