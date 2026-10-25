//! Nota prevista: movie factors from the model artifact, the owner's profile folded in on-device.
//!
//! A nota is predicted as `owner mean + movie bias + profile · movie factors`. Working relative
//! to the owner's own mean (design §6.4) means high notas are never read as generosity. The
//! training job fits every MovieLens user the same way, so fold-in solves the very problem the
//! movie factors were trained against.

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;

use crate::{Error, NotaEntry};

/// The model artifact format this core reads; the training job writes the same.
pub const MODEL_FORMAT_VERSION: u32 = 1;

#[derive(Deserialize)]
struct ModelArtifact {
    format_version: u32,
    data_version: String,
    global_mean: f32,
    lambda: f32,
    movies: Vec<MovieFactors>,
}

#[derive(Deserialize)]
struct MovieFactors {
    tmdb_id: u32,
    bias: f32,
    factors: Vec<f32>,
}

pub(crate) struct Model {
    pub data_version: String,
    /// Stands in for the owner's mean before the first nota.
    global_mean: f32,
    lambda: f32,
    dim: usize,
    /// `tmdb_id` → (bias, factors).
    movies: HashMap<u32, (f32, Vec<f32>)>,
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
        if artifact.movies.iter().any(|m| m.factors.len() != dim) {
            return Err(invalid("movies with different factor counts".into()));
        }
        // Fold-in needs a positive λ to stay solvable.
        if artifact.lambda.is_nan() || artifact.lambda <= 0.0 {
            return Err(invalid(format!("lambda {} is not positive", artifact.lambda)));
        }
        Ok(Model {
            data_version: artifact.data_version,
            global_mean: artifact.global_mean,
            lambda: artifact.lambda,
            dim,
            movies: artifact.movies.into_iter().map(|m| (m.tmdb_id, (m.bias, m.factors))).collect(),
        })
    }

    /// Fold-in: the owner's factors given the fixed movie factors. With a half-life, a nota
    /// weighs half as much for every `half_life_days` it is older than the most recent one.
    pub fn profile(&self, notas: &BTreeMap<u32, Vec<NotaEntry>>, half_life_days: Option<u32>) -> Profile {
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
            let (bias, factors) = self.movies.get(id)?;
            Some((factors.as_slice(), e.nota - mean - bias, weight(e)))
        });
        Profile { mean, factors: weighted_ridge(rows, self.dim, self.lambda) }
    }

    pub fn predict(&self, profile: &Profile, tmdb_id: u32) -> Option<f32> {
        let (bias, factors) = self.movies.get(&tmdb_id)?;
        let dot: f32 = profile.factors.iter().zip(factors).map(|(p, q)| p * q).sum();
        Some((profile.mean + bias + dot).clamp(0.5, 5.0))
    }
}

/// The owner's most recent notas, hidden from the fold-in, and the rest. `fraction` is 0–1;
/// same-second notas are held out in tmdb_id order.
pub(crate) fn hold_out(
    notas: &BTreeMap<u32, Vec<NotaEntry>>,
    fraction: f32,
) -> (BTreeMap<u32, Vec<NotaEntry>>, Vec<(u32, NotaEntry)>) {
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
    known: &BTreeMap<u32, Vec<NotaEntry>>,
    hidden: &'a [(u32, NotaEntry)],
    half_life_days: Option<u32>,
) -> Vec<(f32, &'a NotaEntry)> {
    let profile = model.profile(known, half_life_days);
    hidden.iter().filter_map(|(id, e)| Some(((model.predict(&profile, *id)? - e.nota).abs(), e))).collect()
}

/// Half-lives tried by the calibration, in days; `None` is no decay.
const HALF_LIVES: [Option<u32>; 6] = [None, Some(90), Some(180), Some(365), Some(730), Some(1460)];
/// Below this many notas the most recent ones are too few to tell half-lives apart.
const MIN_NOTAS_TO_CALIBRATE: usize = 10;

/// The half-life that best predicts the owner's most recent fifth of notas from the rest; no
/// decay unless one does strictly better.
pub(crate) fn calibrate(model: &Model, notas: &BTreeMap<u32, Vec<NotaEntry>>) -> Option<u32> {
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
