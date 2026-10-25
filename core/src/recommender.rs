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

    /// Fold-in: the owner's factors given the fixed movie factors.
    pub fn profile(&self, notas: &BTreeMap<u32, Vec<NotaEntry>>) -> Profile {
        let current: Vec<(u32, f32)> =
            notas.iter().filter_map(|(&id, h)| Some((id, h.last()?.nota))).collect();
        let mean = if current.is_empty() {
            self.global_mean
        } else {
            current.iter().map(|&(_, n)| n).sum::<f32>() / current.len() as f32
        };
        let rows = current.iter().filter_map(|(id, nota)| {
            let (bias, factors) = self.movies.get(id)?;
            Some((factors.as_slice(), nota - mean - bias))
        });
        Profile { mean, factors: ridge(rows, self.dim, self.lambda) }
    }

    pub fn predict(&self, profile: &Profile, tmdb_id: u32) -> Option<f32> {
        let (bias, factors) = self.movies.get(&tmdb_id)?;
        let dot: f32 = profile.factors.iter().zip(factors).map(|(p, q)| p * q).sum();
        Some((profile.mean + bias + dot).clamp(0.5, 5.0))
    }
}

/// Minimizes `Σ (y − w·x)² + λ·n·|w|²` over the `n` rows `(x, y)`, each `x` of length `dim`
/// (ALS-WR regularization). Shared with the training job.
pub fn ridge<'a>(rows: impl IntoIterator<Item = (&'a [f32], f32)>, dim: usize, lambda: f32) -> Vec<f32> {
    let mut a = vec![0f64; dim * dim];
    let mut b = vec![0f64; dim];
    let mut n = 0;
    for (x, y) in rows {
        n += 1;
        for i in 0..dim {
            let xi = x[i] as f64;
            b[i] += xi * y as f64;
            for j in 0..=i {
                a[i * dim + j] += xi * x[j] as f64;
            }
        }
    }
    if n == 0 {
        return vec![0.0; dim];
    }
    for i in 0..dim {
        a[i * dim + i] += lambda as f64 * n as f64;
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
