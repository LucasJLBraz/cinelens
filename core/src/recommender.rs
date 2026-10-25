//! Nota prevista: movie factors from the model artifact, the owner's profile folded in on-device.
//!
//! A nota is predicted as `owner mean + movie bias + profile · movie factors`. Working relative
//! to the owner's own mean (design §6.4) means high notas are never read as generosity. The
//! training job fits every MovieLens user the same way, so fold-in solves the very problem the
//! movie factors were trained against.
//!
//! A movie the training data lacks gets its bias and factors from its content (genres,
//! directors, cast, language) through the mapping the training job learns, with a lower
//! confiança.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Deserialize;

use crate::{Error, NotaEntry};

/// The owner's notas by movie, each history in time order.
type Notas = BTreeMap<u32, Vec<NotaEntry>>;

/// The model artifact format this core reads; the training job writes the same.
pub const MODEL_FORMAT_VERSION: u32 = 2;

#[derive(Deserialize)]
struct ModelArtifact {
    format_version: u32,
    data_version: String,
    global_mean: f32,
    lambda: f32,
    movies: Vec<MovieFactors>,
    content: Vec<ContentFactors>,
    #[serde(default)]
    genome: Option<GenomeArtifact>,
}

/// The tag genome subset: each movie's `GENOME_ASPECTS` scores, named in `aspects`.
#[derive(Deserialize)]
struct GenomeArtifact {
    aspects: Vec<String>,
    movies: Vec<GenomeScores>,
}

#[derive(Deserialize)]
struct GenomeScores {
    tmdb_id: u32,
    scores: Vec<f32>,
}

/// What the model artifact keeps of MovieLens's tag genome: per aspect, the genome tags whose
/// relevance (0–1) it averages, a negative sign for those that count against it. "leve" steers
/// "parecidos, mas…"; the rest are the o que pesou aspects, in `crate::Aspecto` order.
// ponytail: hand-picked tags, ritmo read as fast paced; swap tags if an aspect steers oddly.
pub const GENOME_ASPECTS: [(&str, &[(&str, f32)]); 7] = [
    ("leve", &[("feel-good", 1.0), ("light", 1.0), ("heartwarming", 1.0), ("funny", 1.0), ("dark", -1.0), ("bleak", -1.0), ("depressing", -1.0)]),
    ("fotografia", &[("cinematography", 1.0), ("great cinematography", 1.0), ("beautifully filmed", 1.0), ("visually stunning", 1.0)]),
    ("atuacoes", &[("great acting", 1.0), ("exceptional acting", 1.0), ("good acting", 1.0)]),
    ("roteiro", &[("excellent script", 1.0), ("original plot", 1.0), ("storytelling", 1.0), ("good story", 1.0)]),
    ("trilha", &[("great soundtrack", 1.0), ("awesome soundtrack", 1.0), ("good soundtrack", 1.0), ("notable soundtrack", 1.0)]),
    ("ritmo", &[("fast paced", 1.0), ("slow paced", -1.0)]),
    ("dialogos", &[("great dialogue", 1.0), ("good dialogue", 1.0), ("highly quotable", 1.0)]),
];
/// `GENOME_ASPECTS` index of "leve".
pub(crate) const LEVE: usize = 0;

#[derive(Deserialize)]
struct MovieFactors {
    tmdb_id: u32,
    bias: f32,
    factors: Vec<f32>,
    /// Training ratings behind the factors; the confiança grows with them.
    ratings: u32,
}

/// One content feature (`crate::content_features`) mapped into the factor space.
#[derive(Deserialize)]
struct ContentFactors {
    feature: String,
    bias: f32,
    factors: Vec<f32>,
}

/// Training ratings at which a collaborative movie's confiança is half way from
/// `CONTENT_CONFIANCA` to 1.
const CONFIANCA_RATINGS: f32 = 100.0;
/// The most a content-only prediction gets (all of the movie's features mapped), and the least
/// a collaborative one does.
const CONTENT_CONFIANCA: f32 = 0.3;

/// A movie the owner marked não me interessa counts as a nota this far below what they would
/// usually give it (their mean plus its bias)...
const NEGATIVE_OFFSET: f32 = 1.0;
/// ...weighing this much of a nota: a weak signal.
// ponytail: fixed offset and weight; tune against held-out notas if dismissals skew the ranking.
const NEGATIVE_WEIGHT: f32 = 0.25;

/// What the model knows of a movie: collaborative factors, or ones guessed from its content.
#[derive(Clone)]
pub(crate) struct Item {
    bias: f32,
    factors: Vec<f32>,
    pub confianca: f32,
    /// Factors learned from who rated it, not guessed from its content.
    pub collaborative: bool,
    /// `GENOME_ASPECTS` scores, NaN for an aspect the artifact lacks; `None` outside the genome.
    pub genome: Option<Vec<f32>>,
}

impl Item {
    /// How alike two movies are in who liked them: the cosine of their factors.
    pub fn similarity(&self, other: &Item) -> f32 {
        let dot: f32 = self.factors.iter().zip(&other.factors).map(|(a, b)| a * b).sum();
        let norm = |f: &[f32]| f.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norms = norm(&self.factors) * norm(&other.factors);
        if norms > 0.0 { dot / norms } else { 0.0 }
    }

    /// Its score in the `GENOME_ASPECTS` aspect `a`, if the genome has one.
    pub fn genome_score(&self, a: usize) -> Option<f32> {
        self.genome.as_ref().map(|g| g[a]).filter(|s| !s.is_nan())
    }

    fn collaborative(m: MovieFactors) -> Item {
        let n = m.ratings as f32;
        let confianca = CONTENT_CONFIANCA + (1.0 - CONTENT_CONFIANCA) * n / (n + CONFIANCA_RATINGS);
        Item { bias: m.bias, factors: m.factors, confianca, collaborative: true, genome: None }
    }
}

pub(crate) struct Model {
    pub data_version: String,
    /// Stands in for the owner's mean before the first nota.
    global_mean: f32,
    lambda: f32,
    dim: usize,
    movies: HashMap<u32, Item>,
    /// Content feature → (bias, factors).
    content: HashMap<String, (f32, Vec<f32>)>,
    /// Per `GENOME_ASPECTS` aspect, its mean over the movies with a score; 0 when none has one.
    pub genome_means: Vec<f32>,
}

/// The owner's side of the prediction, recomputed after every change to the notas.
pub(crate) struct Profile {
    mean: f32,
    factors: Vec<f32>,
}

impl Model {
    pub fn parse(bytes: &[u8]) -> Result<Model, Error> {
        let invalid = Error::InvalidModel;
        let artifact: ModelArtifact = serde_json::from_slice(bytes).map_err(|e| invalid(e.to_string()))?;
        if artifact.format_version != MODEL_FORMAT_VERSION {
            return Err(invalid(format!("unknown format version {}", artifact.format_version)));
        }
        let dim = artifact.movies.first().map_or(0, |m| m.factors.len());
        let content = artifact.content.iter().map(|c| &c.factors);
        if artifact.movies.iter().map(|m| &m.factors).chain(content).any(|f| f.len() != dim) {
            return Err(invalid("movies with different factor counts".into()));
        }
        // Fold-in needs a positive λ to stay solvable.
        if artifact.lambda.is_nan() || artifact.lambda <= 0.0 {
            return Err(invalid(format!("lambda {} is not positive", artifact.lambda)));
        }
        let mut movies: HashMap<u32, Item> =
            artifact.movies.into_iter().map(|m| (m.tmdb_id, Item::collaborative(m))).collect();
        if let Some(genome) = artifact.genome {
            // Matched by name, so an artifact with fewer or reordered aspects still loads.
            let columns: Vec<Option<usize>> =
                GENOME_ASPECTS.iter().map(|(name, _)| genome.aspects.iter().position(|a| a == name)).collect();
            for row in genome.movies {
                if let Some(item) = movies.get_mut(&row.tmdb_id) {
                    let score = |c: &Option<usize>| c.and_then(|c| row.scores.get(c).copied()).unwrap_or(f32::NAN);
                    item.genome = Some(columns.iter().map(score).collect());
                }
            }
        }
        let genome_means = (0..GENOME_ASPECTS.len())
            .map(|a| {
                let scores: Vec<f32> = movies.values().filter_map(|m| m.genome_score(a)).collect();
                if scores.is_empty() { 0.0 } else { scores.iter().sum::<f32>() / scores.len() as f32 }
            })
            .collect();
        Ok(Model {
            data_version: artifact.data_version,
            global_mean: artifact.global_mean,
            lambda: artifact.lambda,
            dim,
            movies,
            content: artifact.content.into_iter().map(|c| (c.feature, (c.bias, c.factors))).collect(),
            genome_means,
        })
    }

    /// Fold-in: the owner's factors given the fixed movie factors. With a half-life, a nota
    /// weighs half as much for every `half_life_days` it is older than the most recent one.
    /// `negatives` (não me interessa) are weak low notas that leave the owner's mean alone.
    pub fn profile(&self, notas: &Notas, negatives: &BTreeSet<u32>, half_life_days: Option<u32>) -> Profile {
        let current: Vec<(u32, &NotaEntry)> = notas.iter().filter_map(|(&id, h)| Some((id, h.last()?))).collect();
        let latest = current.iter().map(|(_, e)| e.at).max().unwrap_or(0);
        let weight = |e: &NotaEntry| match half_life_days {
            None => 1.0,
            Some(days) => 0.5f32.powf((latest - e.at) as f32 / (days as f32 * 86_400.0)),
        };
        let total: f32 = current.iter().map(|(_, e)| weight(e)).sum();
        let mean = if current.is_empty() {
            self.global_mean
        } else {
            current.iter().map(|(_, e)| weight(e) * e.nota).sum::<f32>() / total
        };
        let rows = current.iter().filter_map(|(id, e)| {
            let item = self.movies.get(id)?;
            Some((item.factors.as_slice(), e.nota - mean - item.bias, weight(e)))
        });
        let negatives = negatives.iter().filter(|id| !notas.contains_key(id)).filter_map(|id| {
            let item = self.movies.get(id)?;
            Some((item.factors.as_slice(), -NEGATIVE_OFFSET, NEGATIVE_WEIGHT))
        });
        let rows = rows.chain(negatives);
        Profile { mean, factors: weighted_ridge(rows, self.dim, self.lambda) }
    }

    /// The movie's collaborative factors, else the mean of its mapped content features';
    /// `None` when neither is known.
    pub fn item(&self, tmdb_id: u32, features: &[String]) -> Option<Item> {
        if let Some(item) = self.movies.get(&tmdb_id) {
            return Some(item.clone());
        }
        // ponytail: plain mean of feature centroids; a learned weight per feature kind (genre vs
        // director) if content-only predictions turn out poor.
        let mapped: Vec<&(f32, Vec<f32>)> = features.iter().filter_map(|f| self.content.get(f)).collect();
        if mapped.is_empty() {
            return None;
        }
        let k = mapped.len() as f32;
        let mut factors = vec![0f32; self.dim];
        for (_, f) in &mapped {
            factors.iter_mut().zip(f).for_each(|(a, b)| *a += b / k);
        }
        Some(Item {
            bias: mapped.iter().map(|(b, _)| b).sum::<f32>() / k,
            factors,
            confianca: CONTENT_CONFIANCA * k / features.len() as f32,
            collaborative: false,
            genome: None,
        })
    }

    pub fn predict(&self, profile: &Profile, item: &Item) -> f32 {
        let dot: f32 = profile.factors.iter().zip(&item.factors).map(|(p, q)| p * q).sum();
        (profile.mean + item.bias + dot).clamp(0.5, 5.0)
    }
}

/// The owner's most recent notas, hidden from the fold-in, and the rest. `fraction` is 0–1;
/// same-second notas are held out in tmdb_id order.
pub(crate) fn hold_out(
    notas: &Notas,
    fraction: f32,
) -> (Notas, Vec<(u32, NotaEntry)>) {
    let mut by_recency: Vec<(u32, NotaEntry)> =
        notas.iter().filter_map(|(&id, h)| Some((id, h.last()?.clone()))).collect();
    by_recency.sort_by_key(|(_, e)| std::cmp::Reverse(e.at));
    let n = ((by_recency.len() as f32 * fraction).round() as usize).min(by_recency.len());
    let hidden: Vec<_> = by_recency.drain(..n).collect();
    let mut known = notas.clone();
    known.retain(|id, _| !hidden.iter().any(|(h, _)| h == id));
    (known, hidden)
}

/// Absolute errors on the `hidden` notas the model knows, folding in `known`.
pub(crate) fn errors<'a>(
    model: &Model,
    known: &Notas,
    hidden: &'a [(u32, NotaEntry)],
    half_life_days: Option<u32>,
) -> Vec<(f32, &'a NotaEntry)> {
    let profile = model.profile(known, &BTreeSet::new(), half_life_days);
    // ponytail: collaborative movies only; pass content features if held-out recent releases matter.
    let error = |id: u32, e: &NotaEntry| Some((model.predict(&profile, &model.item(id, &[])?) - e.nota).abs());
    hidden.iter().filter_map(|(id, e)| Some((error(*id, e)?, e))).collect()
}

/// Half-lives tried by the calibration, in days; `None` is no decay.
const HALF_LIVES: [Option<u32>; 6] = [None, Some(90), Some(180), Some(365), Some(730), Some(1460)];
/// Below this many notas the most recent ones are too few to tell half-lives apart.
const MIN_NOTAS_TO_CALIBRATE: usize = 10;

/// The half-life that best predicts the owner's most recent fifth of notas from the rest; no
/// decay unless one does strictly better.
pub(crate) fn calibrate(model: &Model, notas: &Notas) -> Option<u32> {
    if notas.len() < MIN_NOTAS_TO_CALIBRATE {
        return None;
    }
    let (known, hidden) = hold_out(notas, 0.2);
    let mae = |half_life| {
        let e = errors(model, &known, &hidden, half_life);
        if e.is_empty() { f32::INFINITY } else { e.iter().map(|(err, _)| err).sum::<f32>() / e.len() as f32 }
    };
    let mut best = (None, mae(None));
    for half_life in &HALF_LIVES[1..] {
        let m = mae(*half_life);
        if m < best.1 {
            best = (*half_life, m);
        }
    }
    best.0
}

/// Minimizes `Σ (y − w·x)² + λ·n·|w|²` over the `n` rows `(x, y)`, each `x` of length `dim`
/// (ALS-WR regularization). Shared with the training job.
pub fn ridge<'a>(rows: impl IntoIterator<Item = (&'a [f32], f32)>, dim: usize, lambda: f32) -> Vec<f32> {
    weighted_ridge(rows.into_iter().map(|(x, y)| (x, y, 1.0)), dim, lambda)
}

/// `ridge` with a weight `c` per row: minimizes `Σ c·(y − w·x)² + λ·Σc·|w|²`.
fn weighted_ridge<'a>(rows: impl IntoIterator<Item = (&'a [f32], f32, f32)>, dim: usize, lambda: f32) -> Vec<f32> {
    let mut a = vec![0f64; dim * dim];
    let mut b = vec![0f64; dim];
    let mut n = 0f64;
    for (x, y, c) in rows {
        let c = c as f64;
        n += c;
        for i in 0..dim {
            let xi = c * x[i] as f64;
            b[i] += xi * y as f64;
            for j in 0..=i {
                a[i * dim + j] += xi * x[j] as f64;
            }
        }
    }
    if n == 0.0 {
        return vec![0.0; dim];
    }
    for i in 0..dim {
        a[i * dim + i] += lambda as f64 * n;
    }
    // Cholesky, lower triangle in place: the matrix is symmetric positive definite.
    for j in 0..dim {
        let d = (a[j * dim + j] - (0..j).map(|k| a[j * dim + k].powi(2)).sum::<f64>()).sqrt();
        a[j * dim + j] = d;
        for i in j + 1..dim {
            let s = a[i * dim + j] - (0..j).map(|k| a[i * dim + k] * a[j * dim + k]).sum::<f64>();
            a[i * dim + j] = s / d;
        }
    }
    for i in 0..dim {
        b[i] = (b[i] - (0..i).map(|k| a[i * dim + k] * b[k]).sum::<f64>()) / a[i * dim + i];
    }
    for i in (0..dim).rev() {
        b[i] = (b[i] - (i + 1..dim).map(|k| a[k * dim + i] * b[k]).sum::<f64>()) / a[i * dim + i];
    }
    b.into_iter().map(|w| w as f32).collect()
}
