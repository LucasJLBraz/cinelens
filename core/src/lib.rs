//! Cinelens core: the single facade the UI talks to.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

mod movielens;
mod recommender;

pub use recommender::{ridge, MODEL_FORMAT_VERSION};
use recommender::Model;

/// The catálogo artifact format this core reads; the catalog job writes the same.
pub const CATALOG_FORMAT_VERSION: u32 = 1;
const PAGE_SIZE: usize = 30;
const POSTER_BASE: &str = "https://image.tmdb.org/t/p/w342";

#[derive(Debug)]
pub enum Error {
    InvalidArtifact(String),
    InvalidModel(String),
    InvalidImport(String),
    InvalidNota(f32),
    InvalidOwnerData(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidArtifact(msg) => write!(f, "invalid catálogo artifact: {msg}"),
            Error::InvalidModel(msg) => write!(f, "invalid model artifact: {msg}"),
            Error::InvalidImport(msg) => write!(f, "invalid MovieLens export: {msg}"),
            Error::InvalidNota(nota) => write!(f, "nota {nota} is not 0.5–5 in half stars"),
            Error::InvalidOwnerData(msg) => write!(f, "invalid owner data: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Deserialize)]
struct CatalogArtifact {
    format_version: u32,
    data_version: String,
    movies: Vec<Movie>,
}

#[derive(Deserialize)]
struct Movie {
    tmdb_id: u32,
    title: String,
    original_title: String,
    year: Option<u16>,
    poster_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MovieCard {
    pub tmdb_id: u32,
    pub title: String,
    pub original_title: String,
    pub year: Option<u16>,
    pub poster_url: Option<String>,
    /// `None` for a movie the owner rated or one the model does not know.
    pub nota_prevista: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct ExplorarPage {
    pub movies: Vec<MovieCard>,
    pub has_more: bool,
}

/// One nota as given at one moment; a movie's history is these in time order, the last one current.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotaEntry {
    pub nota: f32,
    /// Unix seconds.
    pub at: i64,
    /// What MovieLens predicted when the nota was given (imported notas only): the accuracy baseline.
    pub movielens_pred: Option<f64>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ImportSummary {
    pub notas: usize,
    /// Notas whose movie is in the catálogo.
    pub matched: usize,
    pub unmatched: usize,
    pub quero_assistir: usize,
}

/// How close the nota prevista came to the owner's most recent notas, hidden from the fold-in.
#[derive(Debug, Serialize)]
pub struct Accuracy {
    /// Most recent notas hidden from the fold-in.
    pub held_out: usize,
    /// Held-out notas the model could predict; those of movies outside it are left out.
    pub notas: usize,
    /// Mean absolute error over those notas.
    pub mae: f32,
    /// MovieLens's own error on those of the notas it logged a prediction for when each was
    /// given; `None` when none of them came from the importação.
    pub movielens_mae: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct MoviePage {
    #[serde(flatten)]
    pub card: MovieCard,
    pub nota: Option<f32>,
    pub history: Vec<NotaEntry>,
    pub quero_assistir: bool,
}

/// How much older notas weigh less in the fold-in.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Decay {
    /// The half-life that best predicts the owner's most recent notas.
    #[default]
    Auto,
    Off,
    HalfLifeDays(u32),
}

#[derive(Debug, PartialEq, Serialize)]
pub struct DecayStatus {
    pub setting: Decay,
    /// The half-life in effect; `None` is no decay.
    pub half_life_days: Option<u32>,
}

#[derive(Default, Serialize, Deserialize)]
struct Settings {
    decay: Decay,
}

/// Everything the owner has told the app; saved and loaded by the shell as one blob.
#[derive(Default, Serialize, Deserialize)]
struct OwnerData {
    notas: BTreeMap<u32, Vec<NotaEntry>>,
    quero_assistir: BTreeSet<u32>,
    #[serde(default)]
    settings: Settings,
}

pub(crate) fn is_valid_nota(nota: f32) -> bool {
    (0.5..=5.0).contains(&nota) && (nota * 2.0).fract() == 0.0
}

/// A core with no catálogo yet (first run) is `Core::default()`.
#[derive(Default)]
pub struct Core {
    data_version: Option<String>,
    movies: Vec<Movie>,
    owner: OwnerData,
    model: Option<Model>,
    /// Aligned with `movies`.
    previstas: Vec<Option<f32>>,
    /// Explorar order: indexes into `movies`, highest nota prevista first.
    para_voce: Vec<usize>,
    /// In effect in the last fold-in.
    half_life_days: Option<u32>,
}

impl Core {
    pub fn open(catalog_artifact: &[u8]) -> Result<Core, Error> {
        let mut core = Core::default();
        core.load_catalog(catalog_artifact)?;
        Ok(core)
    }

    /// Replaces the catálogo. On error the current one stays in place.
    pub fn load_catalog(&mut self, catalog_artifact: &[u8]) -> Result<(), Error> {
        let artifact: CatalogArtifact = serde_json::from_slice(catalog_artifact)
            .map_err(|e| Error::InvalidArtifact(e.to_string()))?;
        if artifact.format_version != CATALOG_FORMAT_VERSION {
            return Err(Error::InvalidArtifact(format!(
                "unknown format version {}",
                artifact.format_version
            )));
        }
        self.data_version = Some(artifact.data_version);
        self.movies = artifact.movies;
        self.recompute();
        Ok(())
    }

    /// Replaces the model. On error the current one stays in place.
    pub fn load_model(&mut self, model_artifact: &[u8]) -> Result<(), Error> {
        self.model = Some(Model::parse(model_artifact)?);
        self.recompute();
        Ok(())
    }

    pub fn model_version(&self) -> Option<&str> {
        self.model.as_ref().map(|m| m.data_version.as_str())
    }

    /// Folds the owner's notas into the model and re-ranks the catálogo; called after every
    /// change to the catálogo, the model or the notas.
    fn recompute(&mut self) {
        self.half_life_days = self.half_life(&self.owner.notas);
        self.previstas = match &self.model {
            None => vec![None; self.movies.len()],
            Some(model) => {
                let profile = model.profile(&self.owner.notas, self.half_life_days);
                let previstas = self.movies.iter().map(|m| {
                    if self.owner.notas.get(&m.tmdb_id).is_some_and(|h| !h.is_empty()) {
                        None
                    } else {
                        model.predict(&profile, m.tmdb_id)
                    }
                });
                previstas.collect()
            }
        };
        self.para_voce = (0..self.movies.len()).collect();
        // Stable: movies without a nota prevista keep catálogo order, after the others.
        self.para_voce.sort_by(|&a, &b| {
            let key = |i: usize| self.previstas[i].unwrap_or(f32::NEG_INFINITY);
            key(b).total_cmp(&key(a))
        });
    }

    fn half_life(&self, notas: &BTreeMap<u32, Vec<NotaEntry>>) -> Option<u32> {
        match (self.owner.settings.decay, &self.model) {
            (Decay::Off, _) | (Decay::Auto, None) => None,
            (Decay::HalfLifeDays(days), _) => Some(days),
            (Decay::Auto, Some(model)) => recommender::calibrate(model, notas),
        }
    }

    pub fn set_decay(&mut self, decay: Decay) {
        self.owner.settings.decay = decay;
        self.recompute();
    }

    pub fn decay(&self) -> DecayStatus {
        DecayStatus { setting: self.owner.settings.decay, half_life_days: self.half_life_days }
    }

    pub fn data_version(&self) -> Option<&str> {
        self.data_version.as_deref()
    }

    /// Imports the MovieLens export files (in any order). Safe to re-run: notas already
    /// imported are not added again, and a nota given in the app since stays current.
    pub fn import_movielens(&mut self, files: &[&str]) -> Result<ImportSummary, Error> {
        let export = movielens::read(files)?;
        let in_catalog: HashSet<u32> = self.movies.iter().map(|m| m.tmdb_id).collect();
        let mut summary = ImportSummary {
            notas: export.notas.len(),
            matched: 0,
            unmatched: 0,
            quero_assistir: export.quero_assistir.len(),
        };
        for nota in export.notas {
            let Some(tmdb_id) = nota.tmdb_id else {
                summary.unmatched += 1;
                continue;
            };
            if in_catalog.contains(&tmdb_id) {
                summary.matched += 1;
            } else {
                summary.unmatched += 1;
            }
            for entry in nota.history {
                self.add_nota(tmdb_id, entry);
            }
        }
        self.owner.quero_assistir.extend(export.quero_assistir);
        self.recompute();
        Ok(summary)
    }

    /// Gives or changes the owner's nota; the previous one stays in the history.
    pub fn rate(&mut self, tmdb_id: u32, nota: f32, at: i64) -> Result<(), Error> {
        if !is_valid_nota(nota) {
            return Err(Error::InvalidNota(nota));
        }
        self.add_nota(tmdb_id, NotaEntry { nota, at, movielens_pred: None });
        self.recompute();
        Ok(())
    }

    fn add_nota(&mut self, tmdb_id: u32, entry: NotaEntry) {
        let history = self.owner.notas.entry(tmdb_id).or_default();
        if !history.contains(&entry) {
            history.push(entry);
            // Stable: same-second notas keep the order they were given in.
            history.sort_by_key(|e| e.at);
        }
    }

    /// The movie page, or `None` if the movie is not in the catálogo.
    pub fn movie(&self, tmdb_id: u32) -> Option<MoviePage> {
        let i = self.movies.iter().position(|m| m.tmdb_id == tmdb_id)?;
        let history = self.owner.notas.get(&tmdb_id).cloned().unwrap_or_default();
        Some(MoviePage {
            card: self.card(i),
            nota: history.last().map(|e| e.nota),
            history,
            quero_assistir: self.owner.quero_assistir.contains(&tmdb_id),
        })
    }

    pub fn owner_data(&self) -> Vec<u8> {
        serde_json::to_vec(&self.owner).expect("owner data serializes")
    }

    /// Replaces the owner data. On error the current one stays in place.
    pub fn load_owner_data(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.owner =
            serde_json::from_slice(bytes).map_err(|e| Error::InvalidOwnerData(e.to_string()))?;
        self.recompute();
        Ok(())
    }

    /// Hides the `fraction` (0–1) of the owner's notas given most recently, folds in the rest
    /// and predicts the hidden ones. `None` without a model or when none of them can be predicted.
    pub fn accuracy(&self, fraction: f32) -> Option<Accuracy> {
        let model = self.model.as_ref()?;
        let (known, hidden) = recommender::hold_out(&self.owner.notas, fraction);
        // Calibrated on the notas left in, so the hidden ones never pick their own decay.
        let errors = recommender::errors(model, &known, &hidden, self.half_life(&known));
        let movielens: Vec<f32> =
            errors.iter().filter_map(|(_, e)| Some((e.movielens_pred? as f32 - e.nota).abs())).collect();
        let errors: Vec<f32> = errors.iter().map(|(err, _)| *err).collect();
        let mean = |e: &[f32]| (!e.is_empty()).then(|| e.iter().sum::<f32>() / e.len() as f32);
        Some(Accuracy { held_out: hidden.len(), notas: errors.len(), mae: mean(&errors)?, movielens_mae: mean(&movielens) })
    }

    /// The catálogo "Para você": highest nota prevista first.
    pub fn explorar(&self, page: usize) -> ExplorarPage {
        let start = (page * PAGE_SIZE).min(self.movies.len());
        let end = (start + PAGE_SIZE).min(self.movies.len());
        ExplorarPage {
            movies: self.para_voce[start..end].iter().map(|&i| self.card(i)).collect(),
            has_more: end < self.movies.len(),
        }
    }

    fn card(&self, i: usize) -> MovieCard {
        let m = &self.movies[i];
        MovieCard {
            tmdb_id: m.tmdb_id,
            title: m.title.clone(),
            original_title: m.original_title.clone(),
            year: m.year,
            poster_url: m.poster_path.as_ref().map(|p| format!("{POSTER_BASE}{p}")),
            nota_prevista: self.previstas[i],
        }
    }
}
