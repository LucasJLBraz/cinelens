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

fn synthetic_catalog(n: usize) -> Vec<u8> {
    let movies: Vec<String> = (0..n)
        .map(|i| {
            format!(
                r#"{{"tmdb_id":{i},"work_type":"movie","title":"Filme número {i}","original_title":"Movie number {i}","year":{},"poster_path":"/abcdefghijklmnopqrstuvwxyz{i}.jpg"}}"#,
                1920 + i % 106
            )
        })
        .collect();
    format!(
        r#"{{"format_version":1,"data_version":"bench","movies":[{}]}}"#,
        movies.join(",")
    )
    .into_bytes()
}
