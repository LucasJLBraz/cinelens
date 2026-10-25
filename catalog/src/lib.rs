//! The daily catalog job: builds the catálogo artifact from TMDB.
//!
//! `build` only sees TMDB through `fetch`, so the same code runs against the real API
//! (see `main.rs`) and against a fixture in tests.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Feature films: TMDB's discover keeps movies at least this long (minutes).
const MIN_RUNTIME: u32 = 40;
/// Older movies need this many TMDB votes to enter the catálogo.
const MIN_VOTES: u32 = 50;
/// Recent releases enter with far fewer votes; below this they are mostly placeholders.
const MIN_VOTES_RECENT: u32 = 5;
const FIRST_YEAR: u32 = 1900;
/// TMDB refuses pages beyond this.
const MAX_PAGES: u32 = 500;
const CAST_SIZE: usize = 8;
const DETAIL_WORKERS: usize = 8;

/// One TMDB GET, given as path plus query. `Ok(None)` is a 404.
pub trait Fetch: Fn(&str) -> Result<Option<Value>, String> + Sync {}
impl<F: Fn(&str) -> Result<Option<Value>, String> + Sync> Fetch for F {}

pub struct Config<'a> {
    /// `YYYY-MM-DD`; also the artifact's data version.
    pub today: &'a str,
    /// `YYYY-MM-DD`; releases from this day on enter with fewer votes.
    pub recent_since: &'a str,
    /// Movies that must be in the catálogo whatever their votes (the training data).
    pub extra_ids: &'a [u32],
}

#[derive(Serialize)]
pub struct Artifact {
    format_version: u32,
    data_version: String,
    genres: Vec<Genre>,
    /// TMDB trending this week, in TMDB's order.
    trending: Vec<u32>,
    /// Most voted first.
    movies: Vec<Movie>,
}

#[derive(Serialize, Deserialize)]
struct Genre {
    id: u32,
    name: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Person {
    id: u32,
    name: String,
}

#[derive(Serialize)]
struct Movie {
    tmdb_id: u32,
    work_type: &'static str,
    title: String,
    original_title: String,
    year: Option<u16>,
    release_date: Option<String>,
    runtime: Option<u32>,
    genre_ids: Vec<u32>,
    original_language: String,
    countries: Vec<String>,
    overview: String,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    imdb_id: Option<String>,
    vote_count: u32,
    vote_average: f32,
    popularity: f32,
    directors: Vec<Person>,
    composers: Vec<Person>,
    cast: Vec<Person>,
    trailer_youtube: Option<String>,
}

#[derive(Deserialize)]
struct Page {
    total_pages: u32,
    results: Vec<Id>,
}

#[derive(Deserialize)]
struct Id {
    id: u32,
}

#[derive(Deserialize)]
struct Details {
    id: u32,
    title: String,
    original_title: String,
    #[serde(default)]
    release_date: String,
    runtime: Option<u32>,
    genres: Vec<Genre>,
    original_language: String,
    production_countries: Vec<Country>,
    #[serde(default)]
    overview: String,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    imdb_id: Option<String>,
    vote_count: u32,
    vote_average: f32,
    popularity: f32,
    credits: Credits,
    videos: Videos,
}

#[derive(Deserialize)]
struct Country {
    iso_3166_1: String,
}

#[derive(Deserialize)]
struct Credits {
    cast: Vec<Person>,
    crew: Vec<Crew>,
}

#[derive(Deserialize)]
struct Crew {
    id: u32,
    name: String,
    job: String,
}

#[derive(Deserialize)]
struct Videos {
    results: Vec<Video>,
}

#[derive(Deserialize)]
struct Video {
    key: String,
    site: String,
    #[serde(rename = "type")]
    kind: String,
    iso_639_1: String,
}

pub fn build(fetch: &impl Fetch, config: &Config) -> Result<Artifact, String> {
    let genres: GenreList = get(fetch, "/genre/movie/list?language=pt-BR")?;
    let trending: Vec<u32> = get::<Page>(fetch, "/trending/movie/week?language=pt-BR")?
        .results
        .into_iter()
        .map(|m| m.id)
        .collect();

    let mut ids: BTreeSet<u32> = config.extra_ids.iter().copied().collect();
    ids.extend(&trending);
    let this_year: u32 = config.today.get(..4).and_then(|y| y.parse().ok()).ok_or("bad today date")?;
    // Discover caps results at MAX_PAGES, so older movies are listed one year at a time.
    for year in FIRST_YEAR..=this_year {
        ids.extend(discover(fetch, MIN_VOTES, &format!("{year}-01-01"), &format!("{year}-12-31"))?);
    }
    ids.extend(discover(fetch, MIN_VOTES_RECENT, config.recent_since, config.today)?);

    let mut movies = details(fetch, &ids.into_iter().collect::<Vec<_>>())?;
    movies.sort_by_key(|m| (std::cmp::Reverse(m.vote_count), m.tmdb_id));

    Ok(Artifact {
        format_version: cinelens_core::CATALOG_FORMAT_VERSION,
        data_version: config.today.to_string(),
        genres: genres.genres,
        trending,
        movies,
    })
}

#[derive(Deserialize)]
struct GenreList {
    genres: Vec<Genre>,
}

fn get<T: serde::de::DeserializeOwned>(fetch: &impl Fetch, request: &str) -> Result<T, String> {
    let body = fetch(request)?.ok_or_else(|| format!("{request}: not found"))?;
    serde_json::from_value(body).map_err(|e| format!("{request}: {e}"))
}

fn discover(fetch: &impl Fetch, min_votes: u32, from: &str, to: &str) -> Result<Vec<u32>, String> {
    let mut ids = Vec::new();
    let mut page = 1;
    loop {
        let request = format!(
            "/discover/movie?language=pt-BR&include_adult=false&with_runtime.gte={MIN_RUNTIME}\
             &vote_count.gte={min_votes}&primary_release_date.gte={from}&primary_release_date.lte={to}&page={page}"
        );
        let result: Page = get(fetch, &request)?;
        if result.total_pages > MAX_PAGES {
            return Err(format!("{from}..{to} has {} pages, more than TMDB serves", result.total_pages));
        }
        ids.extend(result.results.into_iter().map(|m| m.id));
        if page >= result.total_pages {
            return Ok(ids);
        }
        page += 1;
    }
}

/// Fetches every movie's details in parallel. A movie TMDB no longer knows is skipped;
/// any other failure fails the build, so a partial catálogo is never published.
fn details(fetch: &impl Fetch, ids: &[u32]) -> Result<Vec<Movie>, String> {
    let next = AtomicUsize::new(0);
    let movies = Mutex::new(Vec::with_capacity(ids.len()));
    let failure = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..DETAIL_WORKERS {
            s.spawn(|| {
                while let Some(&id) = ids.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let request = format!(
                        "/movie/{id}?language=pt-BR&append_to_response=credits,videos&include_video_language=pt,en"
                    );
                    match fetch(&request).and_then(|body| match body {
                        None => Ok(None),
                        Some(body) => serde_json::from_value::<Details>(body)
                            .map(|d| Some(movie(d)))
                            .map_err(|e| format!("{request}: {e}")),
                    }) {
                        Ok(Some(m)) => movies.lock().unwrap().push(m),
                        Ok(None) => {}
                        Err(e) => {
                            *failure.lock().unwrap() = Some(e);
                            next.store(ids.len(), Ordering::Relaxed);
                        }
                    }
                }
            });
        }
    });
    match failure.into_inner().unwrap() {
        Some(e) => Err(e),
        None => Ok(movies.into_inner().unwrap()),
    }
}

fn movie(d: Details) -> Movie {
    let crew = |job: &str| -> Vec<Person> {
        d.credits
            .crew
            .iter()
            .filter(|c| c.job == job)
            .map(|c| Person { id: c.id, name: c.name.clone() })
            .collect()
    };
    let trailer = |lang: &str| {
        d.videos
            .results
            .iter()
            .find(|v| v.site == "YouTube" && v.kind == "Trailer" && v.iso_639_1 == lang)
            .map(|v| v.key.clone())
    };
    Movie {
        tmdb_id: d.id,
        work_type: "movie",
        directors: crew("Director"),
        composers: crew("Original Music Composer"),
        cast: d.credits.cast.iter().take(CAST_SIZE).cloned().collect(),
        trailer_youtube: trailer("pt").or_else(|| trailer("en")),
        year: d.release_date.get(..4).and_then(|y| y.parse().ok()),
        release_date: Some(d.release_date).filter(|r| !r.is_empty()),
        title: d.title,
        original_title: d.original_title,
        runtime: d.runtime.filter(|&r| r > 0),
        genre_ids: d.genres.iter().map(|g| g.id).collect(),
        original_language: d.original_language,
        countries: d.production_countries.into_iter().map(|c| c.iso_3166_1).collect(),
        overview: d.overview,
        poster_path: d.poster_path,
        backdrop_path: d.backdrop_path,
        imdb_id: d.imdb_id.filter(|i| !i.is_empty()),
        vote_count: d.vote_count,
        vote_average: d.vote_average,
        popularity: d.popularity,
    }
}
