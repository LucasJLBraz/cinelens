//! Speed budgets from design §8, measured at the core facade.
//! `cargo bench -p cinelens-core` exits non-zero when any budget is exceeded.

use std::time::{Duration, Instant};

use cinelens_core::Core;

/// Realistic catálogo size for v1 (feature films above a vote threshold).
const CATALOG_SIZE: usize = 60_000;

fn main() {
    let catalog = synthetic_catalog(CATALOG_SIZE);
    let mut failed = false;

    failed |= !budget("startup: load catálogo", Duration::from_secs(1), || {
        Core::open(&catalog).unwrap();
    });

    if failed {
        std::process::exit(1);
    }
}

/// Runs `f` several times and checks the median against `limit`.
fn budget(name: &str, limit: Duration, mut f: impl FnMut()) -> bool {
    let mut runs: Vec<Duration> = (0..9)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .collect();
    runs.sort();
    let median = runs[runs.len() / 2];
    let ok = median <= limit;
    println!("{} {name}: median {median:?} (budget {limit:?})", if ok { "ok  " } else { "FAIL" });
    ok
}

/// Movies shaped like the catalog job's output, so startup also pays for the fields
/// the core skips.
fn synthetic_catalog(n: usize) -> Vec<u8> {
    let person = |id: usize| format!(r#"{{"id":{id},"name":"Pessoa Com Nome {id}"}}"#);
    let movies: Vec<String> = (0..n)
        .map(|i| {
            let cast: Vec<String> = (0..8).map(|k| person(i * 8 + k)).collect();
            format!(
                r#"{{"tmdb_id":{i},"work_type":"movie","title":"Filme número {i}","original_title":"Movie number {i}","year":{year},"release_date":"{year}-05-17","runtime":112,"genre_ids":[18,35,10749],"original_language":"en","countries":["US","GB"],"overview":"{overview}","poster_path":"/abcdefghijklmnopqrstuvwxyz{i}.jpg","backdrop_path":"/zyxwvutsrqponmlkjihgfedcba{i}.jpg","imdb_id":"tt{i:07}","vote_count":{i},"vote_average":6.9,"popularity":12.3,"directors":[{director}],"composers":[{composer}],"cast":[{cast}],"trailer_youtube":"dQw4w9WgXcQ"}}"#,
                year = 1920 + i % 106,
                overview = "Uma sinopse de tamanho típico, com algumas frases sobre a história. ".repeat(4),
                director = person(1_000_000 + i),
                composer = person(2_000_000 + i),
                cast = cast.join(","),
            )
        })
        .collect();
    format!(
        r#"{{"format_version":1,"data_version":"bench","genres":[],"trending":[],"movies":[{}]}}"#,
        movies.join(",")
    )
    .into_bytes()
}
