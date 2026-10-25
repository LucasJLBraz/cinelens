//! The monthly training job: movie factors from MovieLens `ml-latest` plus, optionally, the
//! Letterboxd April 2026 sample, by alternating least squares.
//!
//! Every user is fitted as the app fits the owner (`cinelens_core::ridge` on notas relative to
//! the user's own mean), so the movie factors are trained against the fold-in the app runs.
//! Letterboxd users also get a bias term against MovieLens's: the owner is folded in like a
//! MovieLens user, so a shift particular to the Letterboxd population must not leak into the
//! movie biases the app uses.
//!
//! The content mapping (feature → bias and factors) is learned from the published movies'
//! factors, for the movies the training data lacks.
//!
//! The tag genome subset (`add_genome`) keeps, per published movie, only the
//! `cinelens_core::GENOME_ASPECTS` scores: "parecidos, mas…" and o que pesou read them.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;

use cinelens_core::{ridge, GENOME_ASPECTS, MODEL_FORMAT_VERSION};
use serde::{Deserialize, Serialize};

const FACTORS: usize = 32;
const ITERATIONS: usize = 10;
const LAMBDA: f32 = 0.05;
/// Movies with fewer ratings are left out of the artifact: their factors are mostly noise.
const MIN_RATINGS: usize = 10;
/// Letterboxd rows whose IMDb match is less certain are left out (`confidence` is the share of
/// IMDb votes the matched title holds among the candidates).
// ponytail: one fixed cut; tune it with cinelens-accuracy if many recent movies fall below it.
const MIN_MATCH_CONFIDENCE: f32 = 0.9;
/// Content features on fewer published movies are left out.
const MIN_CONTENT_MOVIES: usize = 3;
/// A feature's factors are the sum over its movies divided by their count plus this, so rare
/// features stay near zero.
const CONTENT_SHRINK: f32 = 5.0;
/// Movie keys at or above this are MovieLens movies without a TMDB id; below it, TMDB ids.
const NO_TMDB: u64 = 1 << 32;
const MOVIELENS: u8 = 0;
const LETTERBOXD: u8 = 1;

#[derive(Serialize)]
pub struct Model {
    format_version: u32,
    data_version: String,
    global_mean: f32,
    lambda: f32,
    movies: Vec<MovieFactors>,
    content: Vec<ContentFactors>,
    #[serde(skip_serializing_if = "Option::is_none")]
    genome: Option<Genome>,
}

#[derive(Serialize)]
struct Genome {
    aspects: Vec<String>,
    movies: Vec<GenomeScores>,
}

#[derive(Serialize)]
struct GenomeScores {
    tmdb_id: u32,
    scores: Vec<f32>,
}

#[derive(Serialize)]
struct MovieFactors {
    tmdb_id: u32,
    bias: f32,
    factors: Vec<f32>,
    ratings: u32,
}

#[derive(Serialize)]
struct ContentFactors {
    feature: String,
    bias: f32,
    factors: Vec<f32>,
}

/// The Letterboxd sample's `letterboxd_interactions.csv`, with the IMDb → TMDB ids that join
/// its movies to the rest (`catalog_imdb_ids`); MovieLens's `links.csv` is used as well.
pub struct Letterboxd<'a> {
    pub interactions: Box<dyn BufRead + 'a>,
    /// Numeric IMDb id (`tt0068646` → 68646) → TMDB id.
    pub imdb_to_tmdb: HashMap<u32, u32>,
}

/// IMDb → TMDB ids of a catálogo artifact, which covers the releases MovieLens lacks.
pub fn catalog_imdb_ids(catalog_artifact: &[u8]) -> Result<HashMap<u32, u32>, String> {
    #[derive(Deserialize)]
    struct Catalog {
        movies: Vec<Movie>,
    }
    #[derive(Deserialize)]
    struct Movie {
        tmdb_id: u32,
        imdb_id: Option<String>,
    }
    let catalog: Catalog = serde_json::from_slice(catalog_artifact).map_err(|e| format!("catálogo: {e}"))?;
    Ok(catalog
        .movies
        .into_iter()
        .filter_map(|m| Some((parse_imdb(&m.imdb_id?)?, m.tmdb_id)))
        .collect())
}

fn parse_imdb(id: &str) -> Option<u32> {
    id.trim_start_matches("tt").parse().ok()
}

/// `ratings` is ml-latest's `ratings.csv` (`userId,movieId,rating,timestamp`), `links` its
/// `links.csv` (`movieId,imdbId,tmdbId`), `content` the catálogo's
/// (`cinelens_core::content_features`); without it the model has no content mapping.
pub fn train(
    ratings: impl BufRead,
    links: impl BufRead,
    letterboxd: Option<Letterboxd>,
    content: &[(u32, Vec<String>)],
    data_version: &str,
) -> Result<Model, String> {
    let links = read_links(links)?;
    let mut r = Ratings::default();
    r.read_movielens(ratings, &links)?;
    if let Some(letterboxd) = letterboxd {
        r.read_letterboxd(letterboxd, &links)?;
    }
    let (n_users, n_movies) = (r.user_source.len(), r.movie_keys.len());
    let by_user = group(&r.user, n_users);
    let by_movie = group(&r.movie, n_movies);

    let user_mean: Vec<f32> = by_user
        .iter()
        .map(|rs| rs.iter().map(|&i| r.rating[i as usize]).sum::<f32>() / rs.len() as f32)
        .collect();
    // Refitted after every iteration; MovieLens's is 0, the reference the app predicts in.
    let mut letterboxd_bias = 0f32;
    let centered = |i: usize, letterboxd_bias: f32| {
        let u = r.user[i] as usize;
        let bias = if r.user_source[u] == LETTERBOXD { letterboxd_bias } else { 0.0 };
        r.rating[i] - user_mean[u] - bias
    };

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
                (&m[1..], centered(i as usize, letterboxd_bias) - m[0])
            });
            row[0] = 1.0;
            row[1..].copy_from_slice(&ridge(rows, FACTORS, LAMBDA));
        });
        parallel_rows(&mut movies, W, |m, row| {
            let rows = by_movie[m].iter().map(|&i| {
                let u = r.user[i as usize] as usize;
                (&users[u * W..][..W], centered(i as usize, letterboxd_bias))
            });
            row.copy_from_slice(&ridge(rows, W, LAMBDA));
        });
        let (mut sum, mut count) = (0f64, 0u64);
        for i in (0..r.rating.len()).filter(|&i| r.user_source[r.user[i] as usize] == LETTERBOXD) {
            let (u, m) = (r.user[i] as usize, r.movie[i] as usize);
            let (user, movie) = (&users[u * W..][..W], &movies[m * W..][..W]);
            let fitted: f32 = user.iter().zip(movie).map(|(a, b)| a * b).sum();
            sum += (r.rating[i] - user_mean[u] - fitted) as f64;
            count += 1;
        }
        letterboxd_bias = if count == 0 { 0.0 } else { (sum / count as f64) as f32 };
    }

    let mut published: Vec<MovieFactors> = (0..n_movies)
        .filter(|&m| by_movie[m].len() >= MIN_RATINGS && r.movie_keys[m] < NO_TMDB)
        .map(|m| {
            let row = &movies[m * W..][..W];
            MovieFactors {
                tmdb_id: r.movie_keys[m] as u32,
                bias: row[0],
                factors: row[1..].to_vec(),
                ratings: by_movie[m].len() as u32,
            }
        })
        .collect();
    published.sort_by_key(|m| m.tmdb_id);
    let content = content_mapping(&published, content);

    Ok(Model {
        format_version: MODEL_FORMAT_VERSION,
        data_version: data_version.to_string(),
        global_mean: r.rating.iter().map(|&x| x as f64).sum::<f64>() as f32 / r.rating.len().max(1) as f32,
        lambda: LAMBDA,
        movies: published,
        content,
        genome: None,
    })
}

/// Adds the tag genome subset from ml-latest's `genome-tags.csv` (`tagId,tag`) and
/// `genome-scores.csv` (`movieId,tagId,relevance`), joined through `links.csv`. Fails when the
/// genome has none of an aspect's tags, so a renamed tag never ships an empty aspect.
pub fn add_genome(model: &mut Model, tags: impl BufRead, scores: impl BufRead, links: impl BufRead) -> Result<(), String> {
    let links = read_links(links)?;
    let mut tag_ids: HashMap<String, u32> = HashMap::new();
    for line in tags.lines().skip(1) {
        let line = line.map_err(|e| e.to_string())?;
        let fields = split_csv_line(&line);
        let id = fields.first().and_then(|id| id.parse().ok()).ok_or_else(|| format!("genome tags: `{line}`"))?;
        tag_ids.insert(fields.get(1).cloned().unwrap_or_default(), id);
    }
    // Aspect kept → its (tag id, sign) pairs.
    let aspects: Vec<(&str, Vec<(u32, f32)>)> = GENOME_ASPECTS
        .iter()
        .map(|(name, tags)| {
            let found: Vec<(u32, f32)> = tags.iter().filter_map(|(t, sign)| Some((*tag_ids.get(*t)?, *sign))).collect();
            if found.is_empty() { Err(format!("genome tags: none of aspect {name}'s")) } else { Ok((*name, found)) }
        })
        .collect::<Result<_, _>>()?;
    let wanted: HashSet<u32> = aspects.iter().flat_map(|(_, t)| t.iter().map(|&(id, _)| id)).collect();
    let published: HashSet<u32> = model.movies.iter().map(|m| m.tmdb_id).collect();
    // TMDB id → tag id → relevance, only for wanted tags of published movies.
    let mut relevance: HashMap<u32, HashMap<u32, f32>> = HashMap::new();
    for (n, line) in scores.lines().enumerate().skip(1) {
        let line = line.map_err(|e| e.to_string())?;
        let bad = || format!("genome scores line {}: `{line}`", n + 1);
        let mut fields = line.split(',');
        let mut next = || fields.next().ok_or_else(bad);
        let (movie, tag, score) = (next()?, next()?, next()?);
        let movie: u32 = movie.parse().map_err(|_| bad())?;
        let tag: u32 = tag.parse().map_err(|_| bad())?;
        let score: f32 = score.parse().map_err(|_| bad())?;
        let Some(&tmdb) = links.tmdb.get(&movie) else { continue };
        if wanted.contains(&tag) && published.contains(&tmdb) {
            relevance.entry(tmdb).or_default().insert(tag, score);
        }
    }
    let mut movies: Vec<GenomeScores> = relevance
        .into_iter()
        .map(|(tmdb_id, r)| {
            let scores = aspects.iter().map(|(_, tags)| {
                let found: Vec<f32> = tags.iter().filter_map(|(t, sign)| Some(sign * r.get(t)?)).collect();
                let mean = if found.is_empty() { 0.0 } else { found.iter().sum::<f32>() / found.len() as f32 };
                // Three decimals keep the artifact small.
                (mean * 1000.0).round() / 1000.0
            });
            GenomeScores { tmdb_id, scores: scores.collect() }
        })
        .collect();
    movies.sort_by_key(|m| m.tmdb_id);
    model.genome = Some(Genome { aspects: aspects.iter().map(|(name, _)| name.to_string()).collect(), movies });
    Ok(())
}

/// Each content feature's shrunk mean bias and factors over the published movies that have it.
fn content_mapping(published: &[MovieFactors], content: &[(u32, Vec<String>)]) -> Vec<ContentFactors> {
    let by_id: HashMap<u32, &MovieFactors> = published.iter().map(|m| (m.tmdb_id, m)).collect();
    let mut sums: HashMap<&str, (usize, f32, Vec<f32>)> = HashMap::new();
    for (id, features) in content {
        let Some(m) = by_id.get(id) else { continue };
        for f in features {
            let (n, bias, factors) = sums.entry(f).or_insert_with(|| (0, 0.0, vec![0.0; FACTORS]));
            *n += 1;
            *bias += m.bias;
            factors.iter_mut().zip(&m.factors).for_each(|(a, b)| *a += b);
        }
    }
    let mut mapping: Vec<ContentFactors> = sums
        .into_iter()
        .filter(|(_, (n, _, _))| *n >= MIN_CONTENT_MOVIES)
        .map(|(feature, (n, bias, factors))| {
            let d = n as f32 + CONTENT_SHRINK;
            ContentFactors { feature: feature.to_string(), bias: bias / d, factors: factors.iter().map(|f| f / d).collect() }
        })
        .collect();
    mapping.sort_by(|a, b| a.feature.cmp(&b.feature));
    mapping
}

#[derive(Default)]
struct Ratings {
    /// Dense user and movie indexes per rating.
    user: Vec<u32>,
    movie: Vec<u32>,
    rating: Vec<f32>,
    /// Source of each dense user.
    user_source: Vec<u8>,
    /// Movie key (TMDB id, or `NO_TMDB + MovieLens id`) by dense index.
    movie_keys: Vec<u64>,
    users: HashMap<(u8, u32), u32>,
    movies: HashMap<u64, u32>,
}

impl Ratings {
    /// Movies are joined across sources by TMDB id, so two MovieLens movies sharing one count
    /// as one.
    fn push(&mut self, source: u8, user: u32, movie_key: u64, rating: f32) {
        let user = *self.users.entry((source, user)).or_insert_with(|| {
            self.user_source.push(source);
            self.user_source.len() as u32 - 1
        });
        let movie = *self.movies.entry(movie_key).or_insert_with(|| {
            self.movie_keys.push(movie_key);
            self.movie_keys.len() as u32 - 1
        });
        self.user.push(user);
        self.movie.push(movie);
        self.rating.push(rating);
    }

    fn read_movielens(&mut self, input: impl BufRead, links: &Links) -> Result<(), String> {
        for (n, line) in input.lines().enumerate().skip(1) {
            let line = line.map_err(|e| e.to_string())?;
            let bad = || format!("ratings line {}: `{line}`", n + 1);
            let mut fields = line.split(',');
            let mut next = || fields.next().ok_or_else(bad);
            let (user, movie, rating) = (next()?, next()?, next()?);
            let user: u32 = user.parse().map_err(|_| bad())?;
            let movie: u32 = movie.parse().map_err(|_| bad())?;
            let rating = parse_rating(rating).ok_or_else(bad)?;
            let key = links.tmdb.get(&movie).map_or(NO_TMDB + movie as u64, |&t| t as u64);
            self.push(MOVIELENS, user, key, rating);
        }
        Ok(())
    }

    /// Unrated rows, unmatched or low-confidence IMDb matches and movies with no TMDB id are
    /// skipped.
    // ponytail: movies with no TMDB id are dropped although their ratings could still shape the
    // users' factors; keep them under NO_TMDB keys if the measured MAE ever needs it.
    fn read_letterboxd(&mut self, letterboxd: Letterboxd, links: &Links) -> Result<(), String> {
        let mut lines = letterboxd.interactions.lines();
        let header = lines.next().ok_or("letterboxd: empty file")?.map_err(|e| e.to_string())?;
        let header = split_csv_line(&header);
        let column = |name: &str| {
            header.iter().position(|h| h == name).ok_or_else(|| format!("letterboxd: no `{name}` column in {header:?}"))
        };
        let (user_col, rating_col, tconst_col, confidence_col) =
            (column("user_id")?, column("rating")?, column("tconst")?, column("confidence")?);
        for (n, line) in lines.enumerate() {
            let line = line.map_err(|e| e.to_string())?;
            let bad = || format!("letterboxd line {}: `{line}`", n + 2);
            let fields = split_csv_line(&line);
            let field = |i: usize| fields.get(i).map(String::as_str).ok_or_else(bad);
            let (rating, tconst) = (field(rating_col)?, field(tconst_col)?);
            if rating.is_empty() || tconst.is_empty() {
                continue;
            }
            let rating = parse_rating(rating).ok_or_else(bad)?;
            let confidence: f32 = field(confidence_col)?.parse().map_err(|_| bad())?;
            let user: u32 = field(user_col)?.trim_start_matches("u_").parse().map_err(|_| bad())?;
            let imdb = parse_imdb(tconst).ok_or_else(bad)?;
            let tmdb = links.imdb.get(&imdb).or_else(|| letterboxd.imdb_to_tmdb.get(&imdb));
            if let (Some(&tmdb), true) = (tmdb, confidence >= MIN_MATCH_CONFIDENCE) {
                self.push(LETTERBOXD, user, tmdb as u64, rating);
            }
        }
        Ok(())
    }
}

fn parse_rating(s: &str) -> Option<f32> {
    s.parse().ok().filter(|r| (0.5..=5.0).contains(r))
}

/// One CSV line; quoted fields may hold commas and `""` (not newlines).
fn split_csv_line(line: &str) -> Vec<String> {
    let (mut fields, mut field, mut quoted) = (Vec::new(), String::new(), false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (_, '"') => quoted = !quoted,
            (false, ',') => fields.push(std::mem::take(&mut field)),
            (_, c) => field.push(c),
        }
    }
    fields.push(field);
    fields
}

struct Links {
    /// MovieLens id → TMDB id; movies TMDB does not know are absent.
    tmdb: HashMap<u32, u32>,
    /// Numeric IMDb id → TMDB id.
    imdb: HashMap<u32, u32>,
}

fn read_links(input: impl BufRead) -> Result<Links, String> {
    let mut links = Links { tmdb: HashMap::new(), imdb: HashMap::new() };
    for line in input.lines().skip(1) {
        let line = line.map_err(|e| e.to_string())?;
        let mut fields = line.split(',');
        let (movie, imdb, tmdb) = (fields.next(), fields.next(), fields.next());
        if let (Some(Ok(movie)), Some(Ok(tmdb))) = (movie.map(str::parse), tmdb.map(|t| t.trim().parse())) {
            links.tmdb.insert(movie, tmdb);
            if let Some(imdb) = imdb.and_then(parse_imdb) {
                links.imdb.insert(imdb, tmdb);
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

/// A whole file, gunzipped when it starts with the gzip magic bytes.
pub fn read_maybe_gzipped(path: &str) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(bytes);
    }
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(bytes.as_slice()), &mut out)
        .map_err(|e| format!("{path}: {e}"))?;
    Ok(out)
}
