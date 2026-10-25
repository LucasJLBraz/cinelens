//! The monthly training job: movie factors from MovieLens `ml-latest`, by alternating least
//! squares.
//!
//! Every user is fitted as the app fits the owner (`cinelens_core::ridge` on notas relative to
//! the user's own mean), so the movie factors are trained against the fold-in the device runs.

use std::collections::HashMap;
use std::io::BufRead;

use cinelens_core::{ridge, MODEL_FORMAT_VERSION};
use serde::Serialize;

const FACTORS: usize = 32;
const ITERATIONS: usize = 10;
const LAMBDA: f32 = 0.05;
/// Movies with fewer ratings are left out of the artifact: their factors are mostly noise.
const MIN_RATINGS: usize = 10;

#[derive(Serialize)]
pub struct Model {
    format_version: u32,
    data_version: String,
    global_mean: f32,
    lambda: f32,
    movies: Vec<MovieFactors>,
}

#[derive(Serialize)]
struct MovieFactors {
    tmdb_id: u32,
    bias: f32,
    factors: Vec<f32>,
}

/// `ratings` is ml-latest's `ratings.csv` (`userId,movieId,rating,timestamp`), `links` its
/// `links.csv` (`movieId,imdbId,tmdbId`).
pub fn train(ratings: impl BufRead, links: impl BufRead, data_version: &str) -> Result<Model, String> {
    let tmdb_ids = read_links(links)?;
    let r = read_ratings(ratings)?;
    let (n_users, n_movies) = (r.user_ids.len(), r.movie_ids.len());
    let by_user = group(&r.user, n_users);
    let by_movie = group(&r.movie, n_movies);

    let user_mean: Vec<f32> = by_user
        .iter()
        .map(|rs| rs.iter().map(|&i| r.rating[i as usize]).sum::<f32>() / rs.len() as f32)
        .collect();

    // Rows: users `[1, factors…]`, movies `[bias, factors…]`, so a movie's bias is fitted as the
    // weight of the user's constant 1.
    const W: usize = FACTORS + 1;
    let mut users = vec![0f32; n_users * W];
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut movies: Vec<f32> = (0..n_movies * W)
        .map(|i| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            if i % W == 0 { 0.0 } else { ((seed >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 0.1 }
        })
        .collect();

    for _ in 0..ITERATIONS {
        parallel_rows(&mut users, W, |u, row| {
            let rows = by_user[u].iter().map(|&i| {
                let m = &movies[r.movie[i as usize] as usize * W..][..W];
                (&m[1..], r.rating[i as usize] - user_mean[u] - m[0])
            });
            row[0] = 1.0;
            row[1..].copy_from_slice(&ridge(rows, FACTORS, LAMBDA));
        });
        parallel_rows(&mut movies, W, |m, row| {
            let rows = by_movie[m].iter().map(|&i| {
                let u = r.user[i as usize] as usize;
                (&users[u * W..][..W], r.rating[i as usize] - user_mean[u])
            });
            row.copy_from_slice(&ridge(rows, W, LAMBDA));
        });
    }

    let mut published: Vec<MovieFactors> = (0..n_movies)
        .filter(|&m| by_movie[m].len() >= MIN_RATINGS)
        .filter_map(|m| {
            let row = &movies[m * W..][..W];
            Some(MovieFactors { tmdb_id: *tmdb_ids.get(&r.movie_ids[m])?, bias: row[0], factors: row[1..].to_vec() })
        })
        .collect();
    published.sort_by_key(|m| m.tmdb_id);
    // ponytail: two MovieLens movies sharing a TMDB id keep the first; the core would keep one anyway.
    published.dedup_by_key(|m| m.tmdb_id);

    Ok(Model {
        format_version: MODEL_FORMAT_VERSION,
        data_version: data_version.to_string(),
        global_mean: r.rating.iter().map(|&x| x as f64).sum::<f64>() as f32 / r.rating.len().max(1) as f32,
        lambda: LAMBDA,
        movies: published,
    })
}

struct Ratings {
    /// Dense user and movie indexes per rating.
    user: Vec<u32>,
    movie: Vec<u32>,
    rating: Vec<f32>,
    /// MovieLens ids by dense index.
    user_ids: Vec<u32>,
    movie_ids: Vec<u32>,
}

fn read_ratings(input: impl BufRead) -> Result<Ratings, String> {
    let mut r = Ratings { user: vec![], movie: vec![], rating: vec![], user_ids: vec![], movie_ids: vec![] };
    let (mut users, mut movies) = (HashMap::new(), HashMap::new());
    for (n, line) in input.lines().enumerate().skip(1) {
        let line = line.map_err(|e| e.to_string())?;
        let bad = || format!("ratings line {}: `{line}`", n + 1);
        let mut fields = line.split(',');
        let mut next = || fields.next().ok_or_else(bad);
        let (user, movie, rating) = (next()?, next()?, next()?);
        let user: u32 = user.parse().map_err(|_| bad())?;
        let movie: u32 = movie.parse().map_err(|_| bad())?;
        let rating: f32 = rating.parse().map_err(|_| bad())?;
        if !(0.5..=5.0).contains(&rating) {
            return Err(bad());
        }
        r.user.push(*users.entry(user).or_insert_with(|| {
            r.user_ids.push(user);
            r.user_ids.len() as u32 - 1
        }));
        r.movie.push(*movies.entry(movie).or_insert_with(|| {
            r.movie_ids.push(movie);
            r.movie_ids.len() as u32 - 1
        }));
        r.rating.push(rating);
    }
    Ok(r)
}

/// MovieLens id → TMDB id; movies TMDB does not know are absent.
fn read_links(input: impl BufRead) -> Result<HashMap<u32, u32>, String> {
    let mut links = HashMap::new();
    for line in input.lines().skip(1) {
        let line = line.map_err(|e| e.to_string())?;
        let mut fields = line.split(',');
        if let (Some(movie), Some(tmdb)) = (fields.next(), fields.nth(1)) {
            if let (Ok(movie), Ok(tmdb)) = (movie.parse(), tmdb.trim().parse()) {
                links.insert(movie, tmdb);
            }
        }
    }
    Ok(links)
}

/// Rating indexes grouped by `key` (user or movie).
fn group(key: &[u32], n: usize) -> Vec<Vec<u32>> {
    let mut groups = vec![Vec::new(); n];
    for (i, &k) in key.iter().enumerate() {
        groups[k as usize].push(i as u32);
    }
    groups
}

/// Calls `f(index, row)` for every `width`-long row of `rows`, spread over all cores.
fn parallel_rows(rows: &mut [f32], width: usize, f: impl Fn(usize, &mut [f32]) + Sync) {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let per_thread = (rows.len() / width).div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (t, chunk) in rows.chunks_mut(per_thread * width).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (k, row) in chunk.chunks_mut(width).enumerate() {
                    f(t * per_thread + k, row);
                }
            });
        }
    });
}
