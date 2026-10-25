//! Cinelens core: the single facade the UI talks to.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

mod display;
mod filtro;
mod movielens;
mod recommender;

pub use display::{DisplayMode, Faixa, Exibicao};
pub use filtro::{Filtro, Mostrar};
pub use recommender::{ridge, GENOME_ASPECTS, MODEL_FORMAT_VERSION};
use recommender::Model;

/// The catálogo artifact format this core reads; the catalog job writes the same.
pub const CATALOG_FORMAT_VERSION: u32 = 1;
const PAGE_SIZE: usize = 30;
const POSTER_BASE: &str = "https://image.tmdb.org/t/p/w342";
const BACKDROP_BASE: &str = "https://image.tmdb.org/t/p/w780";
const PARECIDOS: usize = 12;
/// The most alike movies "parecidos, mas…" picks its parecidos from.
const PARECIDOS_MAS_POOL: usize = 100;
/// Notas at least this high count as loved for the reason line.
const LOVED: f32 = 4.0;
/// How alike (factor cosine) a loved movie must be to give the reason line.
// ponytail: fixed cut; check against the published model's cosines and tune.
const PORQUE_MIN_SIMILARITY: f32 = 0.3;
/// Movies named in the reason line, at most.
const PORQUE_MAX: usize = 2;
/// Stars a movie with a person the owner is fã of gains in the ranking.
// ponytail: flat boost whatever the person's role; weigh director above cast if it feels off.
const FA_BOOST: f32 = 1.0;
/// Stars a movie gains in the ranking per unit its genome score is above the mean, in the
/// aspects the owner said weighed in their notas (each aspect by its share of all those marked).
// ponytail: fixed weight; tune once the owner has tagged a few dozen notas.
const PESOU_BOOST: f32 = 2.0;
/// Only movies whose nota prevista is among this top share of the owner's predictions (Aposta
/// forte and Boa chance) are pulled by o que pesou: an aspect never lifts a movie they would
/// probably not like.
const PESOU_TOP: f32 = 0.35;
/// How long agora não keeps a movie out of the destaque.
const AGORA_NAO_SECS: i64 = 28 * 86_400;

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
    #[serde(default)]
    genres: Vec<Genero>,
    #[serde(default)]
    providers: Vec<Provedor>,
    movies: Vec<Movie>,
}

/// A TMDB genre, named in Portuguese.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genero {
    pub id: u32,
    pub name: String,
}

/// What Explorar's filters can choose from.
#[derive(Debug, Serialize)]
pub struct Opcoes {
    pub generos: Vec<Genero>,
    /// Original languages (ISO 639-1), most movies first.
    pub idiomas: Vec<String>,
    /// Production countries (ISO 3166-1), most movies first.
    pub paises: Vec<String>,
}

/// A streaming, rental or purchase service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provedor {
    pub id: u32,
    pub name: String,
}

/// Where a movie is in Brazil, as provider ids (TMDB's data from JustWatch).
#[derive(Deserialize)]
struct Watch {
    link: Option<String>,
    #[serde(default)]
    flatrate: Vec<u32>,
    #[serde(default)]
    rent: Vec<u32>,
    #[serde(default)]
    buy: Vec<u32>,
}

#[derive(Deserialize)]
struct Movie {
    tmdb_id: u32,
    title: String,
    original_title: String,
    year: Option<u16>,
    poster_path: Option<String>,
    #[serde(default)]
    genre_ids: Vec<u32>,
    #[serde(default)]
    directors: Vec<Person>,
    #[serde(default)]
    cast: Vec<Person>,
    #[serde(default)]
    original_language: Option<String>,
    #[serde(default)]
    watch: Option<Watch>,
    #[serde(default)]
    runtime: Option<u32>,
    #[serde(default)]
    countries: Vec<String>,
    #[serde(default)]
    vote_count: u32,
    #[serde(default)]
    overview: String,
    #[serde(default)]
    backdrop_path: Option<String>,
    #[serde(default)]
    composers: Vec<Person>,
    #[serde(default)]
    trailer_youtube: Option<String>,
}

#[derive(Deserialize)]
struct Person {
    id: u32,
    #[serde(default)]
    name: String,
}

impl Movie {
    /// Directors, cast and composers.
    fn people(&self) -> impl Iterator<Item = &Person> {
        self.directors.iter().chain(&self.cast).chain(&self.composers)
    }

    /// `g<genre id>`, `d<director id>`, `c<cast member id>`, `l<original language>`: the keys of
    /// the content mapping in the model.
    // ponytail: no TMDB keywords (the catalog job does not fetch them) nor synopsis words; add
    // them as features if content-only predictions of new releases miss.
    fn content_features(&self) -> Vec<String> {
        let genres = self.genre_ids.iter().map(|g| format!("g{g}"));
        let directors = self.directors.iter().map(|p| format!("d{}", p.id));
        let cast = self.cast.iter().map(|p| format!("c{}", p.id));
        let language = self.original_language.iter().map(|l| format!("l{l}"));
        genres
            .chain(directors)
            .chain(cast)
            .chain(language)
            .collect()
    }
}

/// Every catálogo movie's content features, as the core reads them; the training job learns
/// the content mapping from the same.
pub fn content_features(catalog_artifact: &[u8]) -> Result<Vec<(u32, Vec<String>)>, Error> {
    let artifact: CatalogArtifact = serde_json::from_slice(catalog_artifact)
        .map_err(|e| Error::InvalidArtifact(e.to_string()))?;
    Ok(artifact
        .movies
        .iter()
        .map(|m| (m.tmdb_id, m.content_features()))
        .collect())
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
    /// 0–1, alongside every nota prevista; content-only predictions stay low.
    pub confianca: Option<f32>,
    /// The nota prevista as the owner chose to see it; `None` without one or in Oculto.
    pub exibicao: Option<Exibicao>,
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
    /// The nota prevista when the nota was given in the app, revealed right after it.
    #[serde(default)]
    pub prevista: Option<f32>,
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

/// Where to watch a movie in Brazil; shown credited to JustWatch.
#[derive(Debug, Serialize)]
pub struct OndeAssistir {
    /// TMDB's watch page, which links on to each service.
    pub link: Option<String>,
    pub assinatura: Vec<Provedor>,
    pub aluguel: Vec<Provedor>,
    pub compra: Vec<Provedor>,
}

/// A director, actor or composer.
#[derive(Debug, Serialize)]
pub struct Pessoa {
    pub id: u32,
    pub name: String,
    /// The owner is fã: their movies rank higher.
    pub fa: bool,
}

/// A person's movies, the ones the owner would like most first.
#[derive(Debug, Serialize)]
pub struct Filmografia {
    #[serde(flatten)]
    pub pessoa: Pessoa,
    pub movies: Vec<MovieCard>,
}

#[derive(Debug, Serialize)]
pub struct MoviePage {
    #[serde(flatten)]
    pub card: MovieCard,
    pub backdrop_url: Option<String>,
    /// Minutes.
    pub runtime: Option<u32>,
    pub generos: Vec<String>,
    /// Original language, ISO 639-1.
    pub idioma: Option<String>,
    pub sinopse: String,
    pub trailer_youtube: Option<String>,
    pub direcao: Vec<Pessoa>,
    pub elenco: Vec<Pessoa>,
    pub trilha: Vec<Pessoa>,
    /// Titles of movies the owner loved that are most like this one: "porque você amou X".
    pub porque: Vec<String>,
    /// People who liked this movie also liked these, most alike first.
    pub parecidos: Vec<MovieCard>,
    pub nota: Option<f32>,
    pub history: Vec<NotaEntry>,
    pub quero_assistir: bool,
    pub ja_vi: bool,
    pub nao_me_interessa: bool,
    pub onde_assistir: Option<OndeAssistir>,
    /// What the owner said weighed in their nota.
    pub pesou: Vec<Aspecto>,
}

/// What the owner can mark a movie with, and unmark.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum Marca {
    QueroAssistir,
    /// Watched, no nota: out of the recomendações, teaches nothing about taste.
    JaVi,
    /// Hidden everywhere for good; a weak negative signal.
    NaoMeInteressa,
    /// Out of the destaque for a few weeks.
    AgoraNao,
}

/// What can weigh in a nota (o que pesou), in `GENOME_ASPECTS` order after "leve".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Aspecto {
    Fotografia,
    Atuacoes,
    Roteiro,
    Trilha,
    Ritmo,
    Dialogos,
}

impl Aspecto {
    /// Its `GENOME_ASPECTS` index.
    fn genome(self) -> usize {
        self as usize + 1
    }
}

/// The ways "parecidos, mas…" refines parecidos.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum Direcao {
    MaisLeve,
    MaisCurto,
    MaisRecente,
    MelhorFotografia,
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
    #[serde(default)]
    decay: Decay,
    #[serde(default)]
    display_mode: DisplayMode,
    /// Provider ids of the services the owner subscribes to.
    #[serde(default)]
    servicos: BTreeSet<u32>,
    /// The owner never wants to be asked o que pesou.
    #[serde(default)]
    pesou_oculto: bool,
}

/// Everything the owner has told the app; saved and loaded by the shell as one blob.
#[derive(Default, Serialize, Deserialize)]
struct OwnerData {
    notas: BTreeMap<u32, Vec<NotaEntry>>,
    quero_assistir: BTreeSet<u32>,
    #[serde(default)]
    ja_vi: BTreeSet<u32>,
    #[serde(default)]
    nao_me_interessa: BTreeSet<u32>,
    /// Movie → until when (Unix seconds) it stays out of the destaque.
    #[serde(default)]
    agora_nao: BTreeMap<u32, i64>,
    /// Person ids the owner is fã of.
    #[serde(default)]
    fas: BTreeSet<u32>,
    /// Movie → what weighed in its nota.
    #[serde(default)]
    pesou: BTreeMap<u32, BTreeSet<Aspecto>>,
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
    /// tmdb_id → index into `movies`.
    index: HashMap<u32, usize>,
    generos: Vec<Genero>,
    provedores: Vec<Provedor>,
    /// Aligned with `movies`: what search matches against.
    busca: Vec<String>,
    /// Aligned with `movies`: 0 (no votes) to 1 (the most voted), on a log scale.
    popularidade: Vec<f32>,
    owner: OwnerData,
    model: Option<Model>,
    /// Aligned with `movies`: what the model knows of each, refreshed when either changes.
    items: Vec<Option<recommender::Item>>,
    /// Aligned with `movies`: (nota prevista, confiança).
    previstas: Vec<Option<(f32, f32)>>,
    /// Aligned with `movies`: what sou fã and o que pesou add to the nota prevista in the ranking.
    boost: Vec<f32>,
    /// Explorar order: indexes into `movies`, recomendações first (highest nota prevista
    /// first), then the rest in catálogo order; não me interessa left out.
    para_voce: Vec<usize>,
    /// Every current nota prevista, highest first: what Faixa is relative to.
    ranked: Vec<f32>,
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
        self.busca = artifact.movies.iter().map(filtro::searchable).collect();
        let log_votes = |m: &Movie| (m.vote_count as f32).ln_1p();
        let most = artifact.movies.iter().map(log_votes).fold(0f32, f32::max).max(f32::MIN_POSITIVE);
        self.popularidade = artifact.movies.iter().map(|m| log_votes(m) / most).collect();
        self.movies = artifact.movies;
        self.index = self.movies.iter().enumerate().map(|(i, m)| (m.tmdb_id, i)).collect();
        self.generos = artifact.genres;
        self.provedores = artifact.providers;
        self.refresh_items();
        Ok(())
    }

    /// Replaces the model. On error the current one stays in place.
    pub fn load_model(&mut self, model_artifact: &[u8]) -> Result<(), Error> {
        self.model = Some(Model::parse(model_artifact)?);
        self.refresh_items();
        Ok(())
    }

    pub fn model_version(&self) -> Option<&str> {
        self.model.as_ref().map(|m| m.data_version.as_str())
    }

    fn refresh_items(&mut self) {
        self.items = match &self.model {
            None => vec![None; self.movies.len()],
            Some(model) => self
                .movies
                .iter()
                .map(|m| model.item(m.tmdb_id, &m.content_features()))
                .collect(),
        };
        self.recompute();
    }

    /// Folds the owner's notas into the model and re-ranks the catálogo; called after every
    /// change to the notas or settings, and by `refresh_items` after the catálogo or model.
    fn recompute(&mut self) {
        self.half_life_days = self.half_life(&self.owner.notas);
        self.previstas = match &self.model {
            None => vec![None; self.movies.len()],
            Some(model) => {
                let profile = model.profile(&self.owner.notas, &self.owner.nao_me_interessa, self.half_life_days);
                let previstas = self.movies.iter().zip(&self.items).map(|(m, item)| {
                    if self.rated(m.tmdb_id) {
                        None
                    } else {
                        item.as_ref()
                            .map(|item| (model.predict(&profile, item), item.confianca))
                    }
                });
                previstas.collect()
            }
        };
        self.ranked = self.previstas.iter().flatten().map(|(p, _)| *p).collect();
        self.ranked.sort_by(|a, b| b.total_cmp(a));
        let pesos = self.pesos();
        self.boost = (0..self.movies.len()).map(|i| self.fa_boost(i) + self.pesou_boost(i, &pesos)).collect();
        let mut para_voce: Vec<usize> = (0..self.movies.len()).filter(|&i| !self.dispensado(i)).collect();
        // Stable: the rest keep catálogo order, after the recomendações.
        para_voce.sort_by(|&a, &b| {
            let key = |i: usize| if self.recomendavel(i) { self.ranking(i) } else { f32::NEG_INFINITY };
            key(b).total_cmp(&key(a))
        });
        self.para_voce = para_voce;
    }

    /// A recomendação: predicted, neither já vi nor não me interessa.
    fn recomendavel(&self, i: usize) -> bool {
        let id = self.movies[i].tmdb_id;
        self.previstas[i].is_some() && !self.owner.ja_vi.contains(&id) && !self.dispensado(i)
    }

    /// Marked não me interessa.
    fn dispensado(&self, i: usize) -> bool {
        self.owner.nao_me_interessa.contains(&self.movies[i].tmdb_id)
    }

    fn rated(&self, tmdb_id: u32) -> bool {
        self.owner.notas.get(&tmdb_id).is_some_and(|h| !h.is_empty())
    }

    /// What recomendações are ranked by: the nota prevista plus `boost`. Only for movies with a
    /// nota prevista.
    fn ranking(&self, i: usize) -> f32 {
        self.previstas[i].map_or(0.0, |(p, _)| p) + self.boost[i]
    }

    /// `FA_BOOST` for a movie with a person the owner is fã of.
    fn fa_boost(&self, i: usize) -> f32 {
        let fa = self.movies[i].people().any(|p| self.owner.fas.contains(&p.id));
        if fa { FA_BOOST } else { 0.0 }
    }

    /// Per `GENOME_ASPECTS` aspect, its share of all the aspects the owner said weighed.
    fn pesos(&self) -> Vec<f32> {
        let mut pesos = vec![0f32; GENOME_ASPECTS.len()];
        let marked: Vec<usize> = self.owner.pesou.values().flatten().map(|a| a.genome()).collect();
        marked.iter().for_each(|&a| pesos[a] += 1.0 / marked.len() as f32);
        pesos
    }

    /// Movies without genome scores (the genome stops in 2023) get none: a weaker effect. Nor do
    /// those below the `PESOU_TOP` share of the owner's predictions.
    fn pesou_boost(&self, i: usize, pesos: &[f32]) -> f32 {
        let (Some(model), Some(item), Some((prevista, _))) = (&self.model, &self.items[i], self.previstas[i]) else { return 0.0 };
        let top = self.ranked.get((self.ranked.len() as f32 * PESOU_TOP) as usize).copied().unwrap_or(f32::NEG_INFINITY);
        if prevista < top {
            return 0.0;
        }
        let shift = |(a, peso): (usize, &f32)| Some(peso * (item.genome_score(a)? - model.genome_means[a]));
        PESOU_BOOST * pesos.iter().enumerate().filter_map(shift).sum::<f32>()
    }

    /// Replaces what the owner said weighed in the movie's nota.
    pub fn set_pesou(&mut self, tmdb_id: u32, aspectos: &[Aspecto]) {
        if aspectos.is_empty() {
            self.owner.pesou.remove(&tmdb_id);
        } else {
            self.owner.pesou.insert(tmdb_id, aspectos.iter().copied().collect());
        }
        self.recompute();
    }

    /// The owner hid the o que pesou prompt for good.
    pub fn pesou_oculto(&self) -> bool {
        self.owner.settings.pesou_oculto
    }

    /// Hides or brings back the o que pesou prompt.
    pub fn set_pesou_oculto(&mut self, oculto: bool) {
        self.owner.settings.pesou_oculto = oculto;
    }

    pub fn set_fa(&mut self, person_id: u32, on: bool) {
        if on {
            self.owner.fas.insert(person_id);
        } else {
            self.owner.fas.remove(&person_id);
        }
        self.recompute();
    }

    /// The person's movies, highest nota prevista first (the owner's own nota for those rated),
    /// then those without either; not those marked não me interessa. `None` for no one in the
    /// catálogo.
    pub fn pessoa(&self, person_id: u32) -> Option<Filmografia> {
        let mut name = None;
        let mut found: Vec<usize> = (0..self.movies.len())
            .filter(|&i| {
                let m = &self.movies[i];
                let person = m.people().find(|p| p.id == person_id);
                if name.is_none() {
                    name = person.map(|p| p.name.clone());
                }
                person.is_some() && !self.dispensado(i)
            })
            .collect();
        let key = |i: usize| {
            let nota = self.owner.notas.get(&self.movies[i].tmdb_id).and_then(|h| h.last()).map(|e| e.nota);
            self.previstas[i].map(|(p, _)| p).or(nota).unwrap_or(f32::NEG_INFINITY)
        };
        found.sort_by(|&a, &b| key(b).total_cmp(&key(a)));
        Some(Filmografia {
            pessoa: self.pessoa_card(person_id, name?),
            movies: found.into_iter().map(|i| self.card(i)).collect(),
        })
    }

    fn pessoa_card(&self, id: u32, name: String) -> Pessoa {
        Pessoa { id, name, fa: self.owner.fas.contains(&id) }
    }

    /// Marks or unmarks a movie; `at` (Unix seconds) starts agora não's few weeks.
    pub fn marcar(&mut self, tmdb_id: u32, marca: Marca, on: bool, at: i64) {
        fn set(marks: &mut BTreeSet<u32>, id: u32, on: bool) {
            if on { marks.insert(id); } else { marks.remove(&id); }
        }
        match marca {
            Marca::QueroAssistir => set(&mut self.owner.quero_assistir, tmdb_id, on),
            Marca::JaVi => set(&mut self.owner.ja_vi, tmdb_id, on),
            Marca::NaoMeInteressa => set(&mut self.owner.nao_me_interessa, tmdb_id, on),
            Marca::AgoraNao if on => _ = self.owner.agora_nao.insert(tmdb_id, at + AGORA_NAO_SECS),
            Marca::AgoraNao => _ = self.owner.agora_nao.remove(&tmdb_id),
        }
        if matches!(marca, Marca::JaVi | Marca::NaoMeInteressa) {
            self.recompute();
        }
    }

    /// The destaque, Início's featured pick: the top recomendação not set aside with agora não.
    // ponytail: always the top one; rotate among the top few if the owner sees the same pick too often.
    pub fn destaque(&self, at: i64) -> Option<MovieCard> {
        let set_aside = |i: usize| self.owner.agora_nao.get(&self.movies[i].tmdb_id).is_some_and(|&until| at < until);
        let i = self.para_voce.iter().copied().take_while(|&i| self.recomendavel(i)).find(|&i| !set_aside(i))?;
        Some(self.card(i))
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

    pub fn set_display_mode(&mut self, mode: DisplayMode) {
        self.owner.settings.display_mode = mode;
    }

    pub fn display_mode(&self) -> DisplayMode {
        self.owner.settings.display_mode
    }

    /// Every service some catálogo movie is on in Brazil, by name.
    pub fn provedores(&self) -> &[Provedor] {
        &self.provedores
    }

    pub fn servicos(&self) -> Vec<u32> {
        self.owner.settings.servicos.iter().copied().collect()
    }

    pub fn set_servicos(&mut self, ids: &[u32]) {
        self.owner.settings.servicos = ids.iter().copied().collect();
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

    /// Gives or changes the owner's nota; the previous one stays in the history. Returns what
    /// the app predicted for it, in the chosen display mode, to reveal now that it is given.
    pub fn rate(&mut self, tmdb_id: u32, nota: f32, at: i64) -> Result<Option<Exibicao>, Error> {
        if !is_valid_nota(nota) {
            return Err(Error::InvalidNota(nota));
        }
        let card = self.index.get(&tmdb_id).map(|&i| self.card(i));
        let (prevista, revealed) = card.map_or((None, None), |c| (c.nota_prevista, c.exibicao));
        self.add_nota(tmdb_id, NotaEntry { nota, at, movielens_pred: None, prevista });
        self.recompute();
        Ok(revealed)
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
        let i = *self.index.get(&tmdb_id)?;
        let m = &self.movies[i];
        let history = self.owner.notas.get(&tmdb_id).cloned().unwrap_or_default();
        let people = |list: &[Person]| list.iter().map(|p| self.pessoa_card(p.id, p.name.clone())).collect();
        let genre = |id: &u32| self.generos.iter().find(|g| g.id == *id).map(|g| g.name.clone());
        Some(MoviePage {
            card: self.card(i),
            backdrop_url: m.backdrop_path.as_ref().map(|p| format!("{BACKDROP_BASE}{p}")),
            runtime: m.runtime,
            generos: m.genre_ids.iter().filter_map(genre).collect(),
            idioma: m.original_language.clone(),
            sinopse: m.overview.clone(),
            trailer_youtube: m.trailer_youtube.clone(),
            direcao: people(&m.directors),
            elenco: people(&m.cast),
            trilha: people(&m.composers),
            porque: self.porque(i),
            parecidos: self.parecidos(i),
            nota: history.last().map(|e| e.nota),
            history,
            quero_assistir: self.owner.quero_assistir.contains(&tmdb_id),
            ja_vi: self.owner.ja_vi.contains(&tmdb_id),
            nao_me_interessa: self.owner.nao_me_interessa.contains(&tmdb_id),
            onde_assistir: self.movies[i].watch.as_ref().map(|w| {
                let named = |ids: &[u32]| ids.iter().filter_map(|id| self.provedores.iter().find(|p| p.id == *id).cloned()).collect();
                OndeAssistir { link: w.link.clone(), assinatura: named(&w.flatrate), aluguel: named(&w.rent), compra: named(&w.buy) }
            }),
            pesou: self.owner.pesou.get(&tmdb_id).map_or_else(Vec::new, |a| a.iter().copied().collect()),
        })
    }

    fn parecidos(&self, i: usize) -> Vec<MovieCard> {
        self.alike(i, PARECIDOS).into_iter().map(|j| self.card(j)).collect()
    }

    /// "Parecidos, mas…": of the movies most alike to this one, those further in `direcao` than
    /// it, most alike first. A movie with no value to compare (no genome scores, no runtime)
    /// compares against its parecidos' median; parecidos without one are left out.
    pub fn parecidos_mas(&self, tmdb_id: u32, direcao: Direcao) -> Vec<MovieCard> {
        let Some(&i) = self.index.get(&tmdb_id) else { return Vec::new() };
        let genome = |j: usize, a: usize| self.items[j].as_ref()?.genome_score(a);
        let value = |j: usize| match direcao {
            Direcao::MaisLeve => genome(j, recommender::LEVE),
            Direcao::MelhorFotografia => genome(j, Aspecto::Fotografia.genome()),
            Direcao::MaisCurto => self.movies[j].runtime.map(|r| -(r as f32)),
            Direcao::MaisRecente => self.movies[j].year.map(f32::from),
        };
        let pool: Vec<(usize, f32)> =
            self.alike(i, PARECIDOS_MAS_POOL).into_iter().filter_map(|j| Some((j, value(j)?))).collect();
        let base = value(i).or_else(|| {
            let mut values: Vec<f32> = pool.iter().map(|&(_, v)| v).collect();
            values.sort_by(f32::total_cmp);
            values.get(values.len() / 2).copied()
        });
        let Some(base) = base else { return Vec::new() };
        pool.into_iter().filter(|&(_, v)| v > base).take(PARECIDOS).map(|(j, _)| self.card(j)).collect()
    }

    /// Item-item among movies with collaborative factors: the `n` most like this one's, weighed
    /// by confiança so sparsely rated movies do not crowd the top, most alike first. Not those
    /// marked não me interessa.
    fn alike(&self, i: usize, n: usize) -> Vec<usize> {
        let Some(item) = self.items[i].as_ref().filter(|item| item.collaborative) else { return Vec::new() };
        let mut scored: Vec<(f32, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i && !self.dispensado(j))
            .filter_map(|(j, other)| other.as_ref().filter(|o| o.collaborative).map(|o| (item.similarity(o) * o.confianca, j)))
            .collect();
        let n = n.min(scored.len());
        if n == 0 {
            return Vec::new();
        }
        scored.select_nth_unstable_by(n - 1, |a, b| b.0.total_cmp(&a.0));
        scored.truncate(n);
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.into_iter().map(|(_, j)| j).collect()
    }

    /// Titles of the loved movies most like this one.
    fn porque(&self, i: usize) -> Vec<String> {
        let Some(item) = &self.items[i] else { return Vec::new() };
        let mut loved: Vec<(f32, usize)> = self
            .owner
            .notas
            .iter()
            .filter(|(_, h)| h.last().is_some_and(|e| e.nota >= LOVED))
            .filter_map(|(id, _)| {
                let j = *self.index.get(id)?;
                let similarity = item.similarity(self.items[j].as_ref()?);
                (j != i && similarity >= PORQUE_MIN_SIMILARITY).then_some((similarity, j))
            })
            .collect();
        loved.sort_by(|a, b| b.0.total_cmp(&a.0));
        loved.into_iter().take(PORQUE_MAX).map(|(_, j)| self.movies[j].title.clone()).collect()
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

    /// The catálogo movies passing `filtro`, recomendações first ("Para você": highest nota
    /// prevista first, popularity weighed by `filtro.garimpo`), then the rest in catálogo order.
    pub fn explorar(&self, filtro: &Filtro, page: usize) -> ExplorarPage {
        let matcher = filtro::Matcher::new(filtro);
        let passes = |&i: &usize| matcher.passes(self, i);
        let mut found: Vec<usize> = if filtro.nao_me_interessa {
            (0..self.movies.len()).filter(|&i| self.dispensado(i)).filter(passes).collect()
        } else {
            self.para_voce.iter().copied().filter(passes).collect()
        };
        if filtro.garimpo != 0.0 {
            // ponytail: popularity is log TMDB votes, one star at either end of the slider; tune
            // the weight once the owner has used it.
            let score = |i: usize| self.ranking(i) - filtro.garimpo * self.popularidade[i];
            let n = found.partition_point(|&i| self.recomendavel(i));
            found[..n].sort_by(|&a, &b| score(b).total_cmp(&score(a)));
        }
        let start = (page * PAGE_SIZE).min(found.len());
        let end = (start + PAGE_SIZE).min(found.len());
        ExplorarPage { movies: found[start..end].iter().map(|&i| self.card(i)).collect(), has_more: end < found.len() }
    }

    pub fn opcoes(&self) -> Opcoes {
        fn by_count<'a>(values: impl Iterator<Item = &'a String>) -> Vec<String> {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            values.for_each(|v| *counts.entry(v).or_default() += 1);
            let mut sorted: Vec<(&str, usize)> = counts.into_iter().collect();
            // Stable: ties stay in code order.
            sorted.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
            sorted.into_iter().map(|(v, _)| v.to_string()).collect()
        }
        Opcoes {
            generos: self.generos.clone(),
            idiomas: by_count(self.movies.iter().filter_map(|m| m.original_language.as_ref())),
            paises: by_count(self.movies.iter().flat_map(|m| &m.countries)),
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
            nota_prevista: self.previstas[i].map(|(p, _)| p),
            confianca: self.previstas[i].map(|(_, c)| c),
            exibicao: self.previstas[i]
                .and_then(|(p, c)| display::exibicao(self.owner.settings.display_mode, p, c, &self.ranked)),
        }
    }
}
