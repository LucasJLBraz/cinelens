//! Cinelens core: the single facade the UI talks to.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

mod display;
mod estatisticas;
mod filtro;
mod listas;
mod movielens;
mod obra;
mod recommender;
mod sync;

pub use display::{DisplayMode, Faixa, Exibicao};
pub use estatisticas::{Acerto, Ano, Discordancia, Estatisticas, Grupo, MinhaNota, Mudanca};
pub use filtro::{Filtro, Mostrar, Tipo};
pub use obra::{read_mapeamento, Mapeamento, ObraId};
pub use recommender::{ridge, ANIME_MODEL_FORMAT_VERSION, GENOME_ASPECTS, MODEL_FORMAT_VERSION, SERIE_WEIGHT};
pub use sync::SyncStorage;
use recommender::Model;

/// The catálogo artifact format this core reads; the catalog job writes the same.
pub const CATALOG_FORMAT_VERSION: u32 = 1;
const PAGE_SIZE: usize = 30;
const POSTER_BASE: &str = "https://image.tmdb.org/t/p/w342";
const BACKDROP_BASE: &str = "https://image.tmdb.org/t/p/w780";
const LOGO_BASE: &str = "https://image.tmdb.org/t/p/w92";
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
/// Destaques to swipe through on Início.
const DESTAQUES: usize = 5;
/// Movies in each Início row.
const FILEIRA: usize = 12;
/// The top recomendações an Início refresh draws its destaques and each row from: Explorar's
/// first page.
const SORTEIO: usize = PAGE_SIZE;
/// How long agora não keeps a movie out of the destaque.
const AGORA_NAO_SECS: i64 = 28 * 86_400;
/// Votes a série needs for its nota do público to count: to be shown, and to face the floor.
const PISO_MIN_VOTOS: u32 = 50;
/// A new model must predict the owner's notas within this MAE to replace the current one.
const MAE_LIMITE: f32 = 0.65;
/// Share of the owner's most recent notas a new model is measured on (ADR 0002 uses 20%).
const MAE_FRACAO: f32 = 0.2;
/// The notification about a new release comes at most this often...
const NOTIFICACAO_SECS: i64 = 7 * 86_400;
/// ...about a movie released at most this many days ago.
const NOVO_DIAS: i64 = 30;
/// Notas on séries before a série can be notified: until then its evidence is almost all filmes.
const NOTIFICACAO_MIN_NOTAS_SERIES: usize = 10;
/// The same for animes.
const NOTIFICACAO_MIN_NOTAS_ANIMES: usize = 10;
/// A verdict from here up, Gostei included, makes the owner want the next temporada.
// ponytail: fixed cut; tune if the next-temporada row shows animes the owner dropped.
const VEREDITO_BOM: f32 = 4.0;

#[derive(Debug)]
pub enum Error {
    InvalidArtifact(String),
    InvalidModel(String),
    InvalidImport(String),
    InvalidNota(f32),
    ReacaoEmFilme,
    InvalidOwnerData(String),
    Sync(String),
    TemporadaSemNota,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidArtifact(msg) => write!(f, "invalid catálogo artifact: {msg}"),
            Error::InvalidModel(msg) => write!(f, "invalid model artifact: {msg}"),
            Error::InvalidImport(msg) => write!(f, "invalid MovieLens export: {msg}"),
            Error::InvalidNota(nota) => write!(f, "nota {nota} is not 0.5–5 in half stars"),
            Error::ReacaoEmFilme => write!(f, "a reação is only for séries"),
            Error::InvalidOwnerData(msg) => write!(f, "invalid owner data: {msg}"),
            Error::Sync(msg) => write!(f, "sync: {msg}"),
            Error::TemporadaSemNota => write!(f, "this temporada takes no nota of its own"),
        }
    }
}

impl std::error::Error for Error {}

/// `f` over `items` on every core, in order. The launch runs this over the whole catálogo.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = items.len().div_ceil(cores).max(1);
    std::thread::scope(|s| {
        let parts: Vec<_> = items.chunks(chunk).map(|c| s.spawn(|| c.iter().map(&f).collect::<Vec<_>>())).collect();
        parts.into_iter().flat_map(|p| p.join().expect("par_map worker panicked")).collect()
    })
}

/// The filmes', séries' or animes' asset; all have this shape, and `tmdb_id` is the `ObraId`.
#[derive(Deserialize)]
struct CatalogArtifact {
    format_version: u32,
    data_version: String,
    #[serde(default)]
    genres: Vec<Genero>,
    #[serde(default)]
    providers: Vec<Provider>,
    #[serde(default)]
    trending: Vec<ObraId>,
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

#[derive(Deserialize)]
struct Provider {
    id: u32,
    name: String,
    logo_path: Option<String>,
}

/// A streaming, rental or purchase service.
#[derive(Debug, Clone, Serialize)]
pub struct Provedor {
    pub id: u32,
    pub name: String,
    pub logo_url: Option<String>,
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

/// What the asset says an obra is; the only way to tell an anime from a série, which share the id's bit.
#[derive(Deserialize, Default, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
enum WorkType {
    #[default]
    Movie,
    Tv,
    Anime,
}

/// One of an anime's temporadas, as TMDB lists it.
#[derive(Deserialize)]
struct Temporada {
    numero: u32,
    nome: String,
    episodios: Option<u32>,
    ano: Option<u16>,
    /// `YYYY-MM-DD`; an asset built before the next-temporada row has none.
    #[serde(default)]
    estreia: Option<String>,
    /// MyAnimeList has an entry for it: there are ratings behind it.
    entrada: bool,
}

#[derive(Deserialize)]
struct Movie {
    #[serde(rename = "tmdb_id")]
    id: ObraId,
    #[serde(default)]
    work_type: WorkType,
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
    /// `YYYY-MM-DD`.
    #[serde(default)]
    release_date: Option<String>,
    /// TMDB's public score, 0–10.
    #[serde(default)]
    vote_average: f32,
    /// Brazilian age rating.
    #[serde(default)]
    certification: Option<String>,
    /// Critics' score (OMDb: Metacritic, else Rotten Tomatoes), 0–100.
    #[serde(default)]
    critic_score: Option<u8>,
    /// Séries only.
    #[serde(default)]
    number_of_seasons: Option<u32>,
    /// Séries only: `em_andamento` or `terminada`.
    #[serde(default)]
    situacao: Option<String>,
    /// Animes only.
    #[serde(default)]
    temporadas: Vec<Temporada>,
}

#[derive(Deserialize)]
struct Person {
    id: u32,
    #[serde(default)]
    name: String,
}

fn parse_artifact(bytes: &[u8], tipo: Tipo) -> Result<CatalogArtifact, Error> {
    let artifact: CatalogArtifact =
        serde_json::from_slice(bytes).map_err(|e| Error::InvalidArtifact(e.to_string()))?;
    if artifact.format_version != CATALOG_FORMAT_VERSION {
        return Err(Error::InvalidArtifact(format!("unknown format version {}", artifact.format_version)));
    }
    if artifact.movies.iter().any(|m| m.tipo() != tipo) {
        return Err(Error::InvalidArtifact("an obra of the wrong type".into()));
    }
    Ok(artifact)
}

impl Movie {
    fn tipo(&self) -> Tipo {
        match (self.work_type, self.id.is_serie()) {
            (WorkType::Anime, _) => Tipo::Anime,
            (_, true) => Tipo::Serie,
            (_, false) => Tipo::Filme,
        }
    }

    /// Directors, cast and composers.
    fn people(&self) -> impl Iterator<Item = &Person> {
        self.directors.iter().chain(&self.cast).chain(&self.composers)
    }

    /// `g<genre id>`, `d<director id>`, `c<cast member id>`, `l<original language>`: the keys of
    /// the content mapping in the model.
    // ponytail: no synopsis words; add them as features if content-only predictions of new
    // releases miss. TMDB keywords, the public score and the decade were measured and left out
    // (`docs/guides/accuracy.md`).
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
        .map(|m| (m.id.tmdb_id(), m.content_features()))
        .collect())
}

#[derive(Debug, Serialize)]
pub struct MovieCard {
    #[serde(rename = "tmdb_id")]
    pub id: ObraId,
    pub title: String,
    pub tipo: Tipo,
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
    /// The Reação it was given as; `None` for stars.
    #[serde(default)]
    pub reacao: Option<Reacao>,
}

/// A quick verdict on a whole série; it counts as a fixed nota.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Reacao {
    NaoGostei,
    Gostei,
    Amei,
}

impl Reacao {
    /// The owner's average notas on filmes in the groups ≤3, 3.5–4 and 4.5–5, in half stars.
    fn nota(self) -> f32 {
        match self {
            Reacao::NaoGostei => 2.5,
            Reacao::Gostei => 4.0,
            Reacao::Amei => 5.0,
        }
    }
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
    /// Held-out notas of movies outside the model, predicted from their content in the
    /// catálogo; 0 without one.
    pub content_notas: usize,
    /// Mean absolute error over those, kept apart so content changes are measured on their own.
    pub content_mae: Option<f32>,
}

/// The error on some held-out notas on séries, and the notas themselves.
#[derive(Debug, Serialize)]
pub struct ErroSeries {
    pub notas: usize,
    /// Mean absolute error.
    pub mae: f32,
    /// Mean nota the owner gave them: whether the owner enjoys what is below the floor.
    pub media: f32,
}

/// How well the owner's most recent notas on séries are predicted. Informational: séries have no
/// accuracy gate.
#[derive(Debug, Serialize)]
pub struct AccuracySeries {
    pub geral: ErroSeries,
    /// Séries not below the floor.
    pub acima: Option<ErroSeries>,
    /// Séries below the floor.
    pub abaixo: Option<ErroSeries>,
}

/// A new model kept out because it predicts the owner worse; shown in Perfil.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rejeicao {
    pub data_version: String,
    /// Its MAE on the owner's most recent notas; `None` when it could predict none of them.
    pub mae: Option<f32>,
    /// The current model's, on the same notas.
    pub mae_atual: Option<f32>,
    /// Why, in Portuguese, for the owner.
    pub motivo: String,
}

/// Início's featured pick, one of a few to swipe through.
#[derive(Debug, Serialize)]
pub struct Destaque {
    #[serde(flatten)]
    pub card: MovieCard,
    /// As on the movie page: "porque você amou X".
    pub porque: Vec<String>,
    pub quero_assistir: bool,
}

/// The weekly notification: a new release the owner would likely love.
#[derive(Debug, Serialize)]
pub struct Notificacao {
    #[serde(rename = "tmdb_id")]
    pub id: ObraId,
    /// "Saiu X, Aposta forte", shown as the display mode shows a nota prevista.
    pub texto: String,
}

/// One of Início's rows: the first movies Buscar shows with a lista pronta's filtro.
#[derive(Debug, Serialize)]
pub struct Fileira {
    pub nome: String,
    /// What "ver tudo" opens Buscar with.
    pub filtro: Filtro,
    pub movies: Vec<MovieCard>,
}

const PORQUE_AMOU_TENTATIVAS: usize = 5;

/// A row with no lista pronta behind it, so nothing for "ver tudo" to open.
#[derive(Debug, Serialize)]
pub struct PorqueAmou {
    pub nome: String,
    pub movies: Vec<MovieCard>,
}

#[derive(Debug, Serialize)]
pub struct Inicio {
    pub destaques: Vec<Destaque>,
    /// Every lista pronta, in order; some may have no movies.
    pub fileiras: Vec<Fileira>,
    /// Parecidos of what the owner loved last; absent with nothing to show.
    pub porque_amou: Option<PorqueAmou>,
    /// A plain reminder, the latest premiere first; empty, the row is absent.
    pub proximas_temporadas: Vec<ProximaTemporada>,
}

/// The temporada after what the owner has seen of an anime they rated well or are Assistindo.
#[derive(Debug, Serialize)]
pub struct ProximaTemporada {
    /// With no prediction: a reminder of what the owner already chose.
    #[serde(flatten)]
    pub card: MovieCard,
    pub temporada: String,
    /// `YYYY-MM-DD`, maybe still to come.
    pub estreia: String,
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
    /// The nota prevista's faixa, shown on the page whatever the display mode.
    pub faixa: Option<Faixa>,
    /// A rated movie's nota prevista with its own nota left out; `None` if unrated or unknown to
    /// the model.
    pub prevista_sem_a_nota: Option<f32>,
    /// TMDB's public score, in stars; `None` with too few votes to tell.
    pub publico: Option<f32>,
    /// Critics' score (Metacritic, else Rotten Tomatoes, from OMDb), 0–100.
    pub critica: Option<u8>,
    pub backdrop_url: Option<String>,
    /// Its page on TMDB.
    pub tmdb_url: String,
    /// Minutes; of an episode, on a série.
    pub runtime: Option<u32>,
    /// Séries only.
    pub temporadas: Option<u32>,
    /// Séries only: `em_andamento` or `terminada`.
    pub situacao: Option<String>,
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
    /// The Reação the current nota was given as; `None` for stars.
    pub reacao: Option<Reacao>,
    pub history: Vec<NotaEntry>,
    pub quero_assistir: bool,
    pub ja_vi: bool,
    pub assistindo: bool,
    pub nao_me_interessa: bool,
    pub onde_assistir: Option<OndeAssistir>,
    /// What the owner said weighed in their nota.
    pub pesou: Vec<Aspecto>,
    /// Animes only: TMDB's temporadas.
    pub lista_temporadas: Vec<TemporadaPagina>,
}

/// One of an anime's temporadas on its page.
#[derive(Debug, Serialize)]
pub struct TemporadaPagina {
    pub numero: u32,
    pub nome: String,
    pub episodios: Option<u32>,
    pub ano: Option<u16>,
    /// Takes a nota of its own: there are ratings behind it, and the anime has more than one entry.
    pub avaliavel: bool,
    pub nota: Option<f32>,
    pub reacao: Option<Reacao>,
    pub history: Vec<NotaEntry>,
}

/// What the owner can mark a movie with, and unmark.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum Marca {
    QueroAssistir,
    /// Watched, no nota: out of the recomendações, teaches nothing about taste.
    JaVi,
    /// Partway through a série (séries only): out of the recomendações and of quero assistir,
    /// teaches nothing about taste.
    Assistindo,
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
    /// The owner turned the weekly notification off.
    #[serde(default)]
    sem_notificacao: bool,
}

/// Everything the owner has told the app; saved and loaded by the shell as one blob.
#[derive(Default, Serialize, Deserialize)]
struct OwnerData {
    notas: BTreeMap<ObraId, Vec<NotaEntry>>,
    /// Anime → temporada → its notas; a nota here wins over the whole for that temporada.
    #[serde(default)]
    notas_temporada: BTreeMap<ObraId, BTreeMap<u32, Vec<NotaEntry>>>,
    quero_assistir: BTreeSet<ObraId>,
    #[serde(default)]
    ja_vi: BTreeSet<ObraId>,
    #[serde(default)]
    assistindo: BTreeSet<ObraId>,
    #[serde(default)]
    nao_me_interessa: BTreeSet<ObraId>,
    /// Movie → until when (Unix seconds) it stays out of the destaque.
    #[serde(default)]
    agora_nao: BTreeMap<ObraId, i64>,
    /// Person ids the owner is fã of.
    #[serde(default)]
    fas: BTreeSet<u32>,
    /// Movie → what weighed in its nota.
    #[serde(default)]
    pesou: BTreeMap<ObraId, BTreeSet<Aspecto>>,
    /// Movies skipped in the rodada rápida as não vi: never asked again.
    #[serde(default)]
    nao_vi: BTreeSet<ObraId>,
    /// Movie → when (Unix seconds) the weekly notification announced it.
    #[serde(default)]
    notificados: BTreeMap<ObraId, i64>,
    #[serde(default)]
    settings: Settings,
    /// When (Unix seconds) each mark or setting last changed on any device, removals included,
    /// keyed `<field>/<id>` or `settings/<name>`: what sync settles conflicts by.
    #[serde(default)]
    changed: BTreeMap<String, i64>,
}

impl OwnerData {
    /// When the owner last removed the movie's nota; notas given by then stay gone.
    fn removida(&self, id: ObraId) -> Option<i64> {
        self.changed.get(&sync::key("notas", id)).copied()
    }

    fn add_nota(&mut self, id: ObraId, entry: NotaEntry) {
        if self.removida(id).is_some_and(|at| entry.at <= at) {
            return;
        }
        let history = self.notas.entry(id).or_default();
        if !history.contains(&entry) {
            history.push(entry);
            // Stable: same-second notas keep the order they were given in.
            history.sort_by_key(|e| e.at);
        }
    }

    fn add_nota_temporada(&mut self, id: ObraId, temporada: u32, entry: NotaEntry) {
        if self.changed.get(&sync::key_temporada(id, temporada)).is_some_and(|&at| entry.at <= at) {
            return;
        }
        let history = self.notas_temporada.entry(id).or_default().entry(temporada).or_default();
        if !history.contains(&entry) {
            history.push(entry);
            history.sort_by_key(|e| e.at);
        }
    }

    fn stamp(&mut self, key: String, at: i64) {
        self.changed.insert(key, at);
    }
}

pub(crate) fn is_valid_nota(nota: f32) -> bool {
    (0.5..=5.0).contains(&nota) && (nota * 2.0).fract() == 0.0
}

/// A core with no catálogo yet (first run) is `Core::default()`.
#[derive(Default)]
pub struct Core {
    data_version: Option<String>,
    series_version: Option<String>,
    animes_version: Option<String>,
    movies: Vec<Movie>,
    /// id → index into `movies`.
    index: HashMap<ObraId, usize>,
    generos: Vec<Genero>,
    provedores: Vec<Provedor>,
    /// Aligned with `movies`: what search matches against.
    busca: Vec<filtro::Busca>,
    /// Aligned with `movies`: 0 (no votes) to 1 (the most voted), on a log scale.
    popularidade: Vec<f32>,
    /// TMDB trending this week, per `Tipo`.
    em_alta: [HashSet<ObraId>; 3],
    /// Some movie has a critics' score.
    tem_critica: bool,
    /// The floor: séries with enough votes and a nota do público below this are no recomendações.
    piso_series: f32,
    owner: OwnerData,
    model: Option<Model>,
    /// Trained on MyAnimeList: predicts only animes, in factors of its own (ADR 0004).
    anime_model: Option<Model>,
    /// The owner's side of the model for filmes, from the last fold-in: only notas on filmes.
    profile: Option<recommender::Profile>,
    /// The same for séries, always predicted through content.
    profile_series: Option<recommender::Profile>,
    /// The same for animes: notas on animes and on filmes, none on séries.
    profile_animes: Option<recommender::Profile>,
    /// Aligned with `movies`: what the model knows of each, refreshed when either changes.
    items: Vec<Option<recommender::Item>>,
    /// Aligned with `movies`: (nota prevista, confiança).
    previstas: Vec<Option<(f32, f32)>>,
    /// Aligned with `movies`: what sou fã and o que pesou add to the nota prevista in the ranking.
    boost: Vec<f32>,
    /// Explorar order: indexes into `movies`, recomendações first (best percentile within their
    /// type first), then the rest in catálogo order; não me interessa left out.
    para_voce: Vec<usize>,
    /// Every current nota prevista of a filme, highest first: what a filme's Faixa is relative to.
    ranked: Vec<f32>,
    /// The same for séries.
    ranked_series: Vec<f32>,
    /// The same for animes.
    ranked_animes: Vec<f32>,
    /// In effect in the last fold-in.
    half_life_days: Option<u32>,
}

impl Core {
    pub fn open(catalog_artifact: &[u8]) -> Result<Core, Error> {
        let mut core = Core::default();
        core.load_catalog(catalog_artifact)?;
        Ok(core)
    }

    /// Replaces the filmes of the catálogo. On error the current ones stay in place.
    pub fn load_catalog(&mut self, catalog_artifact: &[u8]) -> Result<(), Error> {
        self.merge(parse_artifact(catalog_artifact, Tipo::Filme)?, Tipo::Filme);
        self.index_catalog();
        self.refresh_items();
        Ok(())
    }

    /// Replaces the séries of the catálogo. On error the current ones stay in place.
    pub fn load_series(&mut self, series_artifact: &[u8]) -> Result<(), Error> {
        self.merge(parse_artifact(series_artifact, Tipo::Serie)?, Tipo::Serie);
        self.index_catalog();
        self.refresh_items();
        Ok(())
    }

    /// Replaces the animes of the catálogo. On error the current ones stay in place.
    pub fn load_animes(&mut self, animes_artifact: &[u8]) -> Result<(), Error> {
        self.merge(parse_artifact(animes_artifact, Tipo::Anime)?, Tipo::Anime);
        self.index_catalog();
        self.refresh_items();
        Ok(())
    }

    /// What the catálogo says the id names; an id it lacks reads by its bit.
    pub fn tipo(&self, id: ObraId) -> Tipo {
        match self.index.get(&id) {
            Some(&i) => self.movies[i].tipo(),
            None if id.is_serie() => Tipo::Serie,
            None => Tipo::Filme,
        }
    }

    /// Swaps in one type's obras, leaving the other's.
    fn merge(&mut self, artifact: CatalogArtifact, tipo: Tipo) {
        match tipo {
            Tipo::Serie => self.series_version = Some(artifact.data_version),
            Tipo::Anime => self.animes_version = Some(artifact.data_version),
            Tipo::Filme => {
                self.data_version = Some(artifact.data_version);
                self.generos = artifact.genres;
            }
        }
        self.movies.retain(|m| m.tipo() != tipo);
        self.movies.extend(artifact.movies);
        // A série that became an anime stays in the séries' asset until its next build.
        let animes: HashSet<ObraId> = self.movies.iter().filter(|m| m.tipo() == Tipo::Anime).map(|m| m.id).collect();
        self.movies.retain(|m| m.tipo() != Tipo::Serie || !animes.contains(&m.id));
        // Stable, and each asset is most voted first: only needed once there are two.
        if self.movies.iter().any(|m| m.id.is_serie()) {
            self.movies.sort_by_key(|m| std::cmp::Reverse(m.vote_count));
        }
        self.em_alta[tipo as usize] = artifact.trending.into_iter().collect();
        for p in artifact.providers {
            if !self.provedores.iter().any(|known| known.id == p.id) {
                self.provedores.push(Provedor { id: p.id, name: p.name, logo_url: p.logo_path.map(|l| format!("{LOGO_BASE}{l}")) });
            }
        }
    }

    /// Everything aligned with or derived from `movies`, after they change.
    fn index_catalog(&mut self) {
        self.busca = par_map(&self.movies, filtro::searchable);
        let log_votes = |m: &Movie| (m.vote_count as f32).ln_1p();
        let most = self.movies.iter().map(log_votes).fold(0f32, f32::max).max(f32::MIN_POSITIVE);
        self.popularidade = self.movies.iter().map(|m| log_votes(m) / most).collect();
        self.index = self.movies.iter().enumerate().map(|(i, m)| (m.id, i)).collect();
        self.tem_critica = self.movies.iter().any(|m| m.critic_score.is_some());
        let com_votos = self.movies.iter().filter(|m| m.tipo() == Tipo::Serie && m.vote_count >= PISO_MIN_VOTOS);
        let mut medias: Vec<f32> = com_votos.map(|m| m.vote_average).collect();
        medias.sort_by(f32::total_cmp);
        self.piso_series = medias.get(medias.len() / 4).copied().unwrap_or(0.0);
        let mut na_assinatura: HashMap<u32, usize> = HashMap::new();
        for id in self.movies.iter().filter_map(|m| m.watch.as_ref()).flat_map(|w| &w.flatrate) {
            *na_assinatura.entry(*id).or_default() += 1;
        }
        self.provedores.sort_by_cached_key(|p| (std::cmp::Reverse(na_assinatura.get(&p.id).copied().unwrap_or(0)), p.name.clone()));
    }

    /// Loads what a launch finds on disk, parsing all of it at once to open faster. Whatever is
    /// missing or invalid stays out: the UI downloads it again.
    pub fn load_saved(&mut self, catalog_artifact: Option<&[u8]>, series_artifact: Option<&[u8]>, animes_artifact: Option<&[u8]>, model_artifact: Option<&[u8]>, anime_model_artifact: Option<&[u8]>) {
        let (catalog, series, animes, model, anime_model) = std::thread::scope(|s| {
            let anime_model = s.spawn(|| anime_model_artifact.and_then(|bytes| Model::parse_anime(bytes).ok()));
            let model = s.spawn(|| model_artifact.and_then(|bytes| Model::parse(bytes).ok()));
            let series = s.spawn(|| series_artifact.and_then(|bytes| parse_artifact(bytes, Tipo::Serie).ok()));
            let animes = s.spawn(|| animes_artifact.and_then(|bytes| parse_artifact(bytes, Tipo::Anime).ok()));
            let catalog = catalog_artifact.and_then(|bytes| parse_artifact(bytes, Tipo::Filme).ok());
            let joined = |what| format!("{what} parse does not panic");
            (catalog, series.join().expect(&joined("series")), animes.join().expect(&joined("animes")), model.join().expect(&joined("model")), anime_model.join().expect(&joined("anime model")))
        });
        if let Some(catalog) = catalog {
            self.merge(catalog, Tipo::Filme);
        }
        if let Some(series) = series {
            self.merge(series, Tipo::Serie);
        }
        if let Some(animes) = animes {
            self.merge(animes, Tipo::Anime);
        }
        if model.is_some() {
            self.model = model;
        }
        if anime_model.is_some() {
            self.anime_model = anime_model;
        }
        self.index_catalog();
        self.refresh_items();
    }

    /// Replaces the model. On error the current one stays in place.
    pub fn load_model(&mut self, model_artifact: &[u8]) -> Result<(), Error> {
        self.model = Some(Model::parse(model_artifact)?);
        self.refresh_items();
        Ok(())
    }

    /// Replaces the anime model. On error the current one stays in place. No accuracy gate: the
    /// owner's check is the informational MAE (ADR 0004).
    pub fn load_anime_model(&mut self, model_artifact: &[u8]) -> Result<(), Error> {
        self.anime_model = Some(Model::parse_anime(model_artifact)?);
        self.refresh_items();
        Ok(())
    }

    pub fn anime_model_version(&self) -> Option<&str> {
        self.anime_model.as_ref().map(|m| m.data_version.as_str())
    }

    /// Replaces the model with a new version only if it predicts the owner's most recent notas
    /// no worse than the current one, and within `MAE_LIMITE`; otherwise keeps the current one
    /// and says why. With nothing to measure on (no current model, no notas) it replaces it.
    pub fn trocar_modelo(&mut self, model_artifact: &[u8]) -> Result<Option<Rejeicao>, Error> {
        let novo = Model::parse(model_artifact)?;
        let atual = self.model.as_ref().and_then(|m| self.accuracy_with(m, MAE_FRACAO, false)).map(|a| a.mae);
        let mae = self.accuracy_with(&novo, MAE_FRACAO, false).map(|a| a.mae);
        let virgula = |x: f32| format!("{x:.2}").replace('.', ",");
        let motivo = match (mae, atual) {
            (None, Some(_)) => Some("o modelo novo não prevê nenhuma das suas notas recentes".to_string()),
            (Some(mae), _) if mae > MAE_LIMITE => Some(format!(
                "erro médio de {} nas suas notas recentes, acima do limite de {}",
                virgula(mae),
                virgula(MAE_LIMITE)
            )),
            (Some(mae), Some(atual)) if mae > atual => Some(format!(
                "erro médio de {} nas suas notas recentes, pior que o do modelo atual ({})",
                virgula(mae),
                virgula(atual)
            )),
            _ => None,
        };
        if let Some(motivo) = motivo {
            return Ok(Some(Rejeicao { data_version: novo.data_version, mae, mae_atual: atual, motivo }));
        }
        self.model = Some(novo);
        self.refresh_items();
        Ok(None)
    }

    pub fn model_version(&self) -> Option<&str> {
        self.model.as_ref().map(|m| m.data_version.as_str())
    }

    /// What predicts an obra of `tipo`.
    fn model_de(&self, tipo: Tipo) -> Option<&Model> {
        if tipo == Tipo::Anime { self.anime_model.as_ref() } else { self.model.as_ref() }
    }

    fn profile_de(&self, tipo: Tipo) -> Option<&recommender::Profile> {
        match tipo {
            Tipo::Filme => self.profile.as_ref(),
            Tipo::Serie => self.profile_series.as_ref(),
            Tipo::Anime => self.profile_animes.as_ref(),
        }
    }

    fn refresh_items(&mut self) {
        self.items = par_map(&self.movies, |m| self.model_de(m.tipo())?.item(m.id, &m.content_features()));
        self.recompute();
    }

    /// Folds the owner's notas into the model and re-ranks the catálogo; called after every
    /// change to the notas or settings, and by `refresh_items` after the catálogo or model.
    fn recompute(&mut self) {
        let notas = self.notas_de_filmes();
        self.half_life_days = self.half_life(self.model.as_ref(), &notas);
        self.profile = self.model.as_ref().map(|m| m.profile(&notas, &self.owner.nao_me_interessa, self.half_life_days));
        self.profile_series = self.model.as_ref().map(|m| self.fold_in_series(m, &self.owner.notas, SERIE_WEIGHT));
        self.profile_animes = self.anime_model.as_ref().map(|m| self.fold_in_animes(m, &BTreeSet::new()));
        self.previstas = self
            .movies
            .iter()
            .zip(&self.items)
            .map(|(m, item)| {
                if self.rated(m.id) {
                    return None;
                }
                let (tipo, item) = (m.tipo(), item.as_ref()?);
                Some((self.model_de(tipo)?.predict(self.profile_de(tipo)?, item), item.confianca))
            })
            .collect();
        let ranked = |tipo: Tipo| {
            let of_type = self.movies.iter().zip(&self.previstas).filter(|(m, _)| m.tipo() == tipo);
            let mut ranked: Vec<f32> = of_type.filter_map(|(_, p)| p.map(|(p, _)| p)).collect();
            ranked.sort_by(|a, b| b.total_cmp(a));
            ranked
        };
        (self.ranked, self.ranked_series, self.ranked_animes) = (ranked(Tipo::Filme), ranked(Tipo::Serie), ranked(Tipo::Anime));
        let pesos = self.pesos();
        self.boost = (0..self.movies.len()).map(|i| self.fa_boost(i) + self.pesou_boost(i, &pesos)).collect();
        // The rest keep catálogo order, after the recomendações.
        let (mut para_voce, resto): (Vec<usize>, Vec<usize>) =
            (0..self.movies.len()).filter(|&i| !self.dispensado(i)).partition(|&i| self.recomendavel(i));
        self.por_percentil(&mut para_voce, |i| self.ranking(i));
        para_voce.extend(resto);
        self.para_voce = para_voce;
    }

    /// Orders `obras` by `score` within each type, then by place within their type, so that
    /// filmes do not crowd out séries or animes only because content-based predictions sit closer to the
    /// mean. With one type it is the order by `score`; ties across types keep the order by score.
    fn por_percentil(&self, obras: &mut [usize], score: impl Fn(usize) -> f32) {
        obras.sort_by(|&a, &b| score(b).total_cmp(&score(a)));
        let mut total = [0usize; 3];
        obras.iter().for_each(|&i| total[self.movies[i].tipo() as usize] += 1);
        let mut contados = [0usize; 3];
        let mut lugares: Vec<(f32, usize)> = obras
            .iter()
            .map(|&i| {
                let tipo = self.movies[i].tipo() as usize;
                contados[tipo] += 1;
                ((contados[tipo] - 1) as f32 / total[tipo] as f32, i)
            })
            .collect();
        lugares.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (obra, (_, i)) in obras.iter_mut().zip(lugares) {
            *obra = i;
        }
    }

    /// Only notas on filmes reach what predicts filmes, the owner's mean and the accuracy gate
    /// included: a nota on a série never changes a prediction for a filme.
    fn notas_de_filmes(&self) -> BTreeMap<ObraId, Vec<NotaEntry>> {
        self.owner.notas.iter().filter(|(id, _)| !id.is_serie()).map(|(&id, h)| (id, h.clone())).collect()
    }

    /// How many séries the owner gave a nota.
    pub(crate) fn notas_em_series(&self) -> usize {
        self.owner.notas.iter().filter(|(&id, h)| self.tipo(id) == Tipo::Serie && !h.is_empty()).count()
    }

    /// The owner's current notas on animes, a temporada's counting as one.
    pub(crate) fn notas_em_animes(&self) -> usize {
        let inteiras = self.owner.notas.iter().filter(|(&id, h)| self.tipo(id) == Tipo::Anime && !h.is_empty()).count();
        inteiras + self.owner.notas_temporada.values().map(BTreeMap::len).sum::<usize>()
    }

    /// What predicts séries: notas on filmes and, through their content, notas on séries; an anime
    /// reads as nothing, so its notas teach nothing here.
    fn fold_in_series(&self, model: &Model, notas: &BTreeMap<ObraId, Vec<NotaEntry>>, peso: f32) -> recommender::Profile {
        let item = |id: ObraId| {
            let i = *self.index.get(&id)?;
            self.items.get(i)?.as_ref().filter(|_| self.movies[i].tipo() == Tipo::Serie)
        };
        model.profile_series(notas, &self.owner.nao_me_interessa, self.half_life_days, peso, item)
    }

    /// What predicts animes: notas on filmes, through the features shared with MyAnimeList or else
    /// their content, and notas on animes, leaving out `sem`'s. Nothing from séries.
    fn fold_in_animes(&self, model: &Model, sem: &BTreeSet<ObraId>) -> recommender::Profile {
        let mut rows: Vec<(recommender::Item, f32, i64, f32)> = Vec::new();
        for (&id, h) in self.owner.notas.iter().filter(|(id, _)| !id.is_serie()) {
            let Some(e) = h.last() else { continue };
            let item = model.item(id, &[]).or_else(|| model.item(id, &self.movies[*self.index.get(&id)?].content_features()));
            if let Some(item) = item {
                let peso = if item.collaborative { 1.0 } else { recommender::FILME_CONTEUDO_WEIGHT };
                rows.push((item, e.nota, e.at, peso));
            }
        }
        let vazio = BTreeMap::new();
        let animes = self.owner.notas.keys().chain(self.owner.notas_temporada.keys()).copied().collect::<BTreeSet<_>>();
        for id in animes.into_iter().filter(|&id| self.tipo(id) == Tipo::Anime && !sem.contains(&id)) {
            let Some(base) = self.index.get(&id).and_then(|&i| self.items[i].as_ref()) else { continue };
            let de_temporada = self.owner.notas_temporada.get(&id).unwrap_or(&vazio);
            let com_fatores = model.temporadas.get(&id);
            for (t, h) in de_temporada {
                let Some(e) = h.last() else { continue };
                // Past the dump, a temporada of an anime with factors teaches nothing: read as the
                // first, a weak late one would count against what the owner liked.
                let item = match com_fatores {
                    Some(f) => f.get(t),
                    None => Some(base),
                };
                rows.extend(item.map(|item| (item.clone(), e.nota, e.at, recommender::ANIME_WEIGHT)));
            }
            let Some(e) = self.owner.notas.get(&id).and_then(|h| h.last()) else { continue };
            // The whole speaks for the temporadas with no nota of their own, as one nota in all.
            let resto: Vec<&recommender::Item> = com_fatores.iter().flat_map(|f| f.iter()).filter(|(t, _)| !de_temporada.contains_key(t)).map(|(_, item)| item).collect();
            match (resto.len(), com_fatores) {
                (0, Some(_)) => {}
                (0, None) if !de_temporada.is_empty() => {}
                (0, None) => rows.push((base.clone(), e.nota, e.at, recommender::ANIME_WEIGHT)),
                (n, _) => rows.extend(resto.into_iter().map(|item| (item.clone(), e.nota, e.at, recommender::ANIME_WEIGHT / n as f32))),
            }
        }
        model.profile_anime(&rows, self.half_life_days)
    }

    /// What the Faixa of obra `i` is relative to: its own type's predictions.
    pub(crate) fn ranked_de(&self, i: usize) -> &[f32] {
        match self.movies[i].tipo() {
            Tipo::Filme => &self.ranked,
            Tipo::Serie => &self.ranked_series,
            Tipo::Anime => &self.ranked_animes,
        }
    }

    /// A recomendação: predicted, not já vi, assistindo or não me interessa, not below the floor.
    fn recomendavel(&self, i: usize) -> bool {
        let id = self.movies[i].id;
        let marcada = self.owner.ja_vi.contains(&id) || self.owner.assistindo.contains(&id);
        self.previstas[i].is_some() && !marcada && !self.dispensado(i) && !self.abaixo_do_piso(i)
    }

    /// A série or anime under the floor of the séries: the lowest quarter of their nota do
    /// público, among those with enough votes.
    pub(crate) fn abaixo_do_piso(&self, i: usize) -> bool {
        let m = &self.movies[i];
        m.id.is_serie() && m.vote_count >= PISO_MIN_VOTOS && m.vote_average < self.piso_series
    }

    /// Marked não me interessa.
    fn dispensado(&self, i: usize) -> bool {
        self.owner.nao_me_interessa.contains(&self.movies[i].id)
    }

    fn rated(&self, id: ObraId) -> bool {
        self.owner.notas.get(&id).is_some_and(|h| !h.is_empty()) || self.owner.notas_temporada.contains_key(&id)
    }

    /// What recomendações are ranked by: the nota prevista plus `boost`. Only for movies with a
    /// nota prevista.
    fn ranking(&self, i: usize) -> f32 {
        self.encolhida(i).unwrap_or(0.0) + self.boost[i]
    }

    /// The nota prevista shrunk toward the owner's mean by confiança², so a sparsely rated 5.0
    /// does not outrank a well-known 4.6; only the order uses it, never what is shown.
    fn encolhida(&self, i: usize) -> Option<f32> {
        let (p, c) = self.previstas[i]?;
        let mean = self.profile_de(self.movies[i].tipo()).map_or(p, |profile| profile.mean);
        Some(mean + (p - mean) * c * c)
    }

    /// `FA_BOOST` for a movie with a person the owner is fã of.
    fn fa_boost(&self, i: usize) -> f32 {
        let fa = self.movies[i].people().any(|p| self.owner.fas.contains(&p.id));
        if fa { FA_BOOST } else { 0.0 }
    }

    /// Per `GENOME_ASPECTS` aspect, its share of all the aspects the owner said weighed.
    fn pesos(&self) -> Vec<f32> {
        let mut pesos = vec![0f32; GENOME_ASPECTS.len()];
        let filmes = self.owner.pesou.iter().filter(|(id, _)| !id.is_serie());
        let marked: Vec<usize> = filmes.flat_map(|(_, aspectos)| aspectos).map(|a| a.genome()).collect();
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
    pub fn set_pesou(&mut self, id: ObraId, aspectos: &[Aspecto], at: i64) {
        self.owner.stamp(sync::key("pesou", id), at);
        if aspectos.is_empty() {
            self.owner.pesou.remove(&id);
        } else {
            self.owner.pesou.insert(id, aspectos.iter().copied().collect());
        }
        self.recompute();
    }

    /// The owner hid the o que pesou prompt for good.
    pub fn pesou_oculto(&self) -> bool {
        self.owner.settings.pesou_oculto
    }

    /// Hides or brings back the o que pesou prompt.
    pub fn set_pesou_oculto(&mut self, oculto: bool, at: i64) {
        self.owner.stamp("settings/pesou_oculto".into(), at);
        self.owner.settings.pesou_oculto = oculto;
    }

    pub fn set_fa(&mut self, person_id: u32, on: bool, at: i64) {
        self.owner.stamp(sync::key("fas", person_id), at);
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
            let nota = self.owner.notas.get(&self.movies[i].id).and_then(|h| h.last()).map(|e| e.nota);
            self.encolhida(i).or(nota).unwrap_or(f32::NEG_INFINITY)
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

    /// Marks or unmarks a movie at `at` (Unix seconds), which also starts agora não's few weeks.
    pub fn marcar(&mut self, id: ObraId, marca: Marca, on: bool, at: i64) {
        if marca == Marca::Assistindo && !id.is_serie() {
            return;
        }
        fn set(marks: &mut BTreeSet<ObraId>, id: ObraId, on: bool) {
            if on { marks.insert(id); } else { marks.remove(&id); }
        }
        let field = match marca {
            Marca::QueroAssistir => "quero_assistir",
            Marca::JaVi => "ja_vi",
            Marca::Assistindo => "assistindo",
            Marca::NaoMeInteressa => "nao_me_interessa",
            Marca::AgoraNao => "agora_nao",
        };
        self.owner.stamp(sync::key(field, id), at);
        match marca {
            Marca::QueroAssistir => set(&mut self.owner.quero_assistir, id, on),
            Marca::JaVi => set(&mut self.owner.ja_vi, id, on),
            Marca::Assistindo => set(&mut self.owner.assistindo, id, on),
            Marca::NaoMeInteressa => set(&mut self.owner.nao_me_interessa, id, on),
            Marca::AgoraNao if on => _ = self.owner.agora_nao.insert(id, at + AGORA_NAO_SECS),
            Marca::AgoraNao => _ = self.owner.agora_nao.remove(&id),
        }
        // An obra is in one of the two at most.
        if on && marca == Marca::Assistindo && self.owner.quero_assistir.remove(&id) {
            self.owner.stamp(sync::key("quero_assistir", id), at);
        }
        if on && marca == Marca::QueroAssistir && self.owner.assistindo.remove(&id) {
            self.owner.stamp(sync::key("assistindo", id), at);
            self.recompute();
        }
        if matches!(marca, Marca::JaVi | Marca::NaoMeInteressa | Marca::Assistindo) {
            self.recompute();
        }
    }

    /// The destaque, Início's featured pick: the top recomendação not set aside with agora não.
    pub fn destaque(&self, at: i64) -> Option<MovieCard> {
        self.destaques(at, 1, None).next().map(|i| self.card(i))
    }

    /// The top `n` recomendações of `tipo` (`None` is Tudo) not set aside with agora não, best first.
    fn destaques(&self, at: i64, n: usize, tipo: Option<Tipo>) -> impl Iterator<Item = usize> + '_ {
        let set_aside = move |i: usize| self.owner.agora_nao.get(&self.movies[i].id).is_some_and(|&until| at < until);
        self.para_voce.iter().copied().take_while(|&i| self.recomendavel(i)).filter(move |&i| !set_aside(i) && tipo.is_none_or(|t| t == self.movies[i].tipo())).take(n)
    }

    /// Início: the destaques to swipe through and a row per lista pronta; `at` is now (Unix
    /// seconds). `sorteio` 0 shows the best of each; any other draws others from the top
    /// `SORTEIO`, so a refresh shows different movies that are still good bets. `tipo` is the
    /// type chip; `None` is Tudo.
    pub fn inicio(&self, at: i64, sorteio: u64, tipo: Option<Tipo>) -> Inicio {
        let id = |&i: &usize| self.movies[i].id;
        let destaques = sortear(self.destaques(at, SORTEIO, tipo).collect(), DESTAQUES, sorteio, id);
        let destaques = destaques.into_iter().map(|i| Destaque {
            card: self.card(i),
            porque: self.porque(i),
            quero_assistir: self.owner.quero_assistir.contains(&self.movies[i].id),
        });
        let fileiras = listas::listas_de(at, self.tem_critica, tipo).into_iter().map(|(nome, filtro)| {
            let mut movies = self.explorar(&filtro, 0).movies;
            // Only recomendações are drawn: past them, Explorar lists the rest in catálogo order.
            let recomendacoes = movies.iter().take_while(|m| m.nota_prevista.is_some()).count();
            movies.truncate(recomendacoes.max(FILEIRA));
            let movies = sortear(movies, FILEIRA, sorteio, |m| m.id);
            Fileira { nome: nome.to_string(), filtro, movies }
        });
        Inicio {
            destaques: destaques.collect(),
            fileiras: fileiras.collect(),
            porque_amou: self.porque_amou(tipo),
            proximas_temporadas: if tipo.is_none_or(|t| t == Tipo::Anime) { self.proximas_temporadas() } else { Vec::new() },
        }
    }

    /// The recomendações most like the obra of `tipo` the owner loved last that has any; parecidos
    /// are of the loved obra's own type.
    fn porque_amou(&self, tipo: Option<Tipo>) -> Option<PorqueAmou> {
        let mut amadas: Vec<(i64, usize)> = self
            .owner
            .notas
            .iter()
            .filter_map(|(id, h)| {
                let e = h.last().filter(|e| e.nota >= LOVED)?;
                Some((e.at, *self.index.get(id)?))
            })
            .filter(|&(_, i)| tipo.is_none_or(|t| t == self.movies[i].tipo()))
            .collect();
        amadas.sort_by_key(|&(at, i)| std::cmp::Reverse((at, self.movies[i].id)));
        // ponytail: a few tries, each a scan of every obra; more if the row is often absent.
        amadas.into_iter().take(PORQUE_AMOU_TENTATIVAS).find_map(|(_, i)| {
            // More than a row: what the owner has already seen is left out after.
            let movies: Vec<MovieCard> =
                self.alike(i, FILEIRA * 4).into_iter().filter(|&j| self.recomendavel(j)).take(FILEIRA).map(|j| self.card(j)).collect();
            (!movies.is_empty()).then(|| PorqueAmou { nome: format!("Porque você amou {}", self.movies[i].title), movies })
        })
    }

    /// From the owner's own notas and marks and TMDB's dates only. A nota on a temporada has seen
    /// up to it; without one, a nota on the whole or Assistindo has seen what had premiered when
    /// given. The latest verdict says whether the anime was rated well.
    // ponytail: with no episode progress, Assistindo alone cannot tell which temporada the owner
    // is on, so only a premiere after the mark counts; a nota on a temporada settles it.
    fn proximas_temporadas(&self) -> Vec<ProximaTemporada> {
        let vazio = BTreeMap::new();
        let ids: BTreeSet<ObraId> = self.owner.notas.keys().chain(self.owner.notas_temporada.keys()).chain(&self.owner.assistindo).copied().collect();
        let mut proximas: Vec<ProximaTemporada> = ids
            .into_iter()
            .filter_map(|id| {
                let i = *self.index.get(&id)?;
                let m = &self.movies[i];
                let de_temporada = self.owner.notas_temporada.get(&id).unwrap_or(&vazio);
                let inteira = self.owner.notas.get(&id).and_then(|h| h.last());
                let veredito = inteira.into_iter().chain(de_temporada.values().filter_map(|h| h.last())).max_by_key(|e| e.at);
                let assistindo = self.owner.assistindo.contains(&id);
                if m.tipo() != Tipo::Anime || self.dispensado(i) || !(assistindo || veredito.is_some_and(|e| e.nota >= VEREDITO_BOM)) {
                    return None;
                }
                let marcada = assistindo.then(|| self.owner.changed.get(&sync::key("assistindo", id)).copied()).flatten();
                let ate = inteira.map(|e| e.at).max(marcada).filter(|_| de_temporada.is_empty()).map(listas::date);
                let ultima = de_temporada.iter().filter(|(_, h)| !h.is_empty()).map(|(&t, _)| t).max().unwrap_or(0);
                let proxima = m
                    .temporadas
                    .iter()
                    .filter(|t| t.numero > ultima && t.estreia.as_ref().is_some_and(|e| ate.as_ref().is_none_or(|v| e > v)))
                    .min_by_key(|t| t.numero)?;
                let card = MovieCard { nota_prevista: None, confianca: None, exibicao: None, ..self.card(i) };
                Some(ProximaTemporada { card, temporada: proxima.nome.clone(), estreia: proxima.estreia.clone()? })
            })
            .collect();
        proximas.sort_by(|a, b| b.estreia.cmp(&a.estreia));
        proximas.truncate(FILEIRA);
        proximas
    }

    /// The rodada rápida's next `n` obras of `tipo`: those the owner most likely saw and whose
    /// nota would move the owner's profile of that type most. Only filmes and animes with
    /// collaborative factors (a nota on any other teaches the fold-in little), any série the model
    /// can read, and none rated, já vi, assistindo, não me interessa or skipped as não vi. Always
    /// about the whole of an anime.
    // ponytail: TMDB votes, linear, stand in for the chance the owner has seen it; learn it from
    // the não vi answers if the rodada still asks about too many unseen movies.
    pub fn rodada(&self, tipo: Tipo, n: usize) -> Vec<MovieCard> {
        let Some(profile) = self.profile_de(tipo) else { return Vec::new() };
        let mut scored: Vec<(f32, usize)> = (0..self.movies.len())
            .filter_map(|i| {
                let id = self.movies[i].id;
                let item = self.items[i].as_ref().filter(|item| self.movies[i].tipo() == tipo && (tipo == Tipo::Serie || item.collaborative))?;
                let marcada = self.owner.ja_vi.contains(&id) || self.owner.assistindo.contains(&id);
                let asked = self.rated(id) || marcada || self.owner.nao_vi.contains(&id);
                (!asked && !self.dispensado(i)).then(|| (profile.uncertainty(item) * self.movies[i].vote_count as f32, i))
            })
            .collect();
        // Stable: ties keep catálogo order.
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.into_iter().take(n).map(|(_, i)| self.card(i)).collect()
    }

    /// Skips the movie in the rodada rápida for good; it teaches nothing about taste.
    pub fn nao_vi(&mut self, id: ObraId) {
        self.owner.nao_vi.insert(id);
    }

    fn half_life(&self, model: Option<&Model>, notas: &BTreeMap<ObraId, Vec<NotaEntry>>) -> Option<u32> {
        match (self.owner.settings.decay, model) {
            (Decay::Off, _) | (Decay::Auto, None) => None,
            (Decay::HalfLifeDays(days), _) => Some(days),
            (Decay::Auto, Some(model)) => recommender::calibrate(model, notas),
        }
    }

    pub fn set_decay(&mut self, decay: Decay, at: i64) {
        self.owner.stamp("settings/decay".into(), at);
        self.owner.settings.decay = decay;
        self.recompute();
    }

    pub fn set_display_mode(&mut self, mode: DisplayMode, at: i64) {
        self.owner.stamp("settings/display_mode".into(), at);
        self.owner.settings.display_mode = mode;
    }

    pub fn display_mode(&self) -> DisplayMode {
        self.owner.settings.display_mode
    }

    /// Whether the weekly notification is on.
    pub fn notificacao(&self) -> bool {
        !self.owner.settings.sem_notificacao
    }

    pub fn set_notificacao(&mut self, on: bool, at: i64) {
        self.owner.stamp("settings/sem_notificacao".into(), at);
        self.owner.settings.sem_notificacao = !on;
    }

    /// The weekly notification, if one is due at `at` (Unix seconds): the best recomendação
    /// released in the last `NOVO_DIAS` days, in Aposta forte and never announced before. A série
    /// competes once the owner has `NOTIFICACAO_MIN_NOTAS_SERIES` notas on séries; its release is
    /// its first episode, so a new season announces nothing. An anime likewise, from
    /// `NOTIFICACAO_MIN_NOTAS_ANIMES` notas on animes.
    pub fn notificacao_devida(&self, at: i64) -> Option<Notificacao> {
        let last = self.owner.notificados.values().max();
        if self.owner.settings.sem_notificacao || last.is_some_and(|&t| at - t < NOTIFICACAO_SECS) {
            return None;
        }
        let (desde, hoje) = (listas::date(at - NOVO_DIAS * 86_400), listas::date(at));
        let novo = |m: &Movie| m.release_date.as_ref().is_some_and(|d| *d >= desde && *d <= hoje);
        let series_competem = self.notas_em_series() >= NOTIFICACAO_MIN_NOTAS_SERIES;
        let animes_competem = self.notas_em_animes() >= NOTIFICACAO_MIN_NOTAS_ANIMES;
        let i = self.para_voce.iter().copied().take_while(|&i| self.recomendavel(i)).find(|&i| {
            let m = &self.movies[i];
            let alta = self.previstas[i].is_some_and(|(p, _)| display::faixa(p, self.ranked_de(i)) == Faixa::ApostaForte);
            let compete = match m.tipo() {
                Tipo::Filme => true,
                Tipo::Serie => series_competem,
                Tipo::Anime => animes_competem,
            };
            alta && novo(m) && compete && !self.owner.notificados.contains_key(&m.id)
        })?;
        let m = &self.movies[i];
        let texto = match self.owner.settings.display_mode {
            DisplayMode::Faixa => format!("Saiu {}, Aposta forte", m.title),
            DisplayMode::Oculto => format!("Saiu {}", m.title),
        };
        Some(Notificacao { id: m.id, texto })
    }

    /// The owner was shown the notification about the movie at `at`: the week starts over.
    pub fn notificado(&mut self, id: ObraId, at: i64) {
        self.owner.notificados.insert(id, at);
    }

    /// Every service some catálogo movie is on in Brazil, those with the most movies on
    /// subscription first (the main services), then by name.
    pub fn provedores(&self) -> &[Provedor] {
        &self.provedores
    }

    pub fn servicos(&self) -> Vec<u32> {
        self.owner.settings.servicos.iter().copied().collect()
    }

    pub fn set_servicos(&mut self, ids: &[u32], at: i64) {
        self.owner.stamp("settings/servicos".into(), at);
        self.owner.settings.servicos = ids.iter().copied().collect();
    }

    pub fn decay(&self) -> DecayStatus {
        DecayStatus { setting: self.owner.settings.decay, half_life_days: self.half_life_days }
    }

    pub fn data_version(&self) -> Option<&str> {
        self.data_version.as_deref()
    }

    pub fn series_version(&self) -> Option<&str> {
        self.series_version.as_deref()
    }

    pub fn animes_version(&self) -> Option<&str> {
        self.animes_version.as_deref()
    }

    /// Imports the MovieLens export files (in any order). Safe to re-run: notas already
    /// imported are not added again, and a nota given in the app since stays current.
    pub fn import_movielens(&mut self, files: &[&str]) -> Result<ImportSummary, Error> {
        let export = movielens::read(files)?;
        let in_catalog: HashSet<ObraId> = self.movies.iter().map(|m| m.id).collect();
        let mut summary = ImportSummary {
            notas: export.notas.len(),
            matched: 0,
            unmatched: 0,
            quero_assistir: export.quero_assistir.len(),
        };
        for nota in export.notas {
            let Some(id) = nota.tmdb_id.map(ObraId::filme) else {
                summary.unmatched += 1;
                continue;
            };
            if in_catalog.contains(&id) {
                summary.matched += 1;
            } else {
                summary.unmatched += 1;
            }
            for entry in nota.history {
                self.owner.add_nota(id, entry);
            }
        }
        self.owner.quero_assistir.extend(export.quero_assistir.into_iter().map(ObraId::filme));
        self.recompute();
        Ok(summary)
    }

    /// Gives or changes the owner's nota; the previous one stays in the history. Returns what
    /// the app predicted for it, in the chosen display mode, to reveal now that it is given.
    pub fn rate(&mut self, id: ObraId, nota: f32, at: i64) -> Result<Option<Exibicao>, Error> {
        if !is_valid_nota(nota) {
            return Err(Error::InvalidNota(nota));
        }
        Ok(self.dar_nota(id, nota, None, at))
    }

    /// Gives or changes the owner's nota on a série as a Reação; otherwise as `rate`.
    pub fn reagir(&mut self, id: ObraId, reacao: Reacao, at: i64) -> Result<Option<Exibicao>, Error> {
        if !id.is_serie() {
            return Err(Error::ReacaoEmFilme);
        }
        Ok(self.dar_nota(id, reacao.nota(), Some(reacao), at))
    }

    /// Gives or changes the owner's nota on one temporada of an anime; it wins over the whole for
    /// that temporada. Only a temporada with ratings behind it, of an anime with more than one.
    pub fn rate_temporada(&mut self, id: ObraId, temporada: u32, nota: f32, at: i64) -> Result<Option<Exibicao>, Error> {
        if !is_valid_nota(nota) {
            return Err(Error::InvalidNota(nota));
        }
        self.dar_nota_temporada(id, temporada, nota, None, at)
    }

    /// `rate_temporada`, as a Reação.
    pub fn reagir_temporada(&mut self, id: ObraId, temporada: u32, reacao: Reacao, at: i64) -> Result<Option<Exibicao>, Error> {
        self.dar_nota_temporada(id, temporada, reacao.nota(), Some(reacao), at)
    }

    fn dar_nota_temporada(&mut self, id: ObraId, temporada: u32, nota: f32, reacao: Option<Reacao>, at: i64) -> Result<Option<Exibicao>, Error> {
        let Some(&i) = self.index.get(&id).filter(|&&i| self.avaliavel(i, temporada)) else { return Err(Error::TemporadaSemNota) };
        // Given after a removal even if this device's clock runs behind.
        let at = self.owner.changed.get(&sync::key_temporada(id, temporada)).map_or(at, |removida| at.max(removida + 1));
        let card = self.card(i);
        self.owner.add_nota_temporada(id, temporada, NotaEntry { nota, at, movielens_pred: None, prevista: None, reacao });
        self.recompute();
        Ok(card.exibicao)
    }

    /// Forgets the nota on a temporada and its history, on every device.
    pub fn remover_nota_temporada(&mut self, id: ObraId, temporada: u32, at: i64) {
        let Some(de_anime) = self.owner.notas_temporada.get_mut(&id) else { return };
        let Some(history) = de_anime.remove(&temporada) else { return };
        if de_anime.is_empty() {
            self.owner.notas_temporada.remove(&id);
        }
        let last = history.last().map_or(at, |e| e.at);
        self.owner.stamp(sync::key_temporada(id, temporada), at.max(last));
        self.recompute();
    }

    /// An anime with more than one entry, one of them this temporada's. Parts MyAnimeList splits a
    /// temporada into count as one, and a long run has no entry per temporada.
    fn avaliavel(&self, i: usize, temporada: u32) -> bool {
        let t = &self.movies[i].temporadas;
        t.iter().filter(|t| t.entrada).count() > 1 && t.iter().any(|t| t.numero == temporada && t.entrada)
    }

    fn dar_nota(&mut self, id: ObraId, nota: f32, reacao: Option<Reacao>, at: i64) -> Option<Exibicao> {
        // Given after a removal even if this device's clock runs behind.
        let at = self.owner.removida(id).map_or(at, |removida| at.max(removida + 1));
        let card = self.index.get(&id).map(|&i| self.card(i));
        let (prevista, revealed) = card.map_or((None, None), |c| (c.nota_prevista, c.exibicao));
        self.owner.add_nota(id, NotaEntry { nota, at, movielens_pred: None, prevista, reacao });
        self.recompute();
        revealed
    }

    /// Forgets the movie's nota and its whole history, on every device, as if never given.
    pub fn remover_nota(&mut self, id: ObraId, at: i64) {
        let Some(history) = self.owner.notas.remove(&id) else { return };
        let last = history.last().map_or(at, |e| e.at);
        self.owner.stamp(sync::key("notas", id), at.max(last));
        self.set_pesou(id, &[], at);
    }

    /// The nota prevista of a rated movie, its own nota left out of the fold-in, so the answer
    /// is what the app would expect and not an echo of the nota.
    fn prevista_sem_a_nota(&self, i: usize) -> Option<f32> {
        let id = self.movies[i].id;
        let tipo = self.movies[i].tipo();
        let (model, item) = (self.model_de(tipo)?, self.items[i].as_ref()?);
        if !self.rated(id) {
            return None;
        }
        if tipo == Tipo::Anime {
            return Some(model.predict(&self.fold_in_animes(model, &BTreeSet::from([id])), item));
        }
        if tipo == Tipo::Serie {
            let mut notas = self.owner.notas.clone();
            notas.remove(&id);
            return Some(model.predict(&self.fold_in_series(model, &notas, SERIE_WEIGHT), item));
        }
        let mut notas = self.notas_de_filmes();
        notas.remove(&id);
        Some(model.predict(&model.profile(&notas, &self.owner.nao_me_interessa, self.half_life_days), item))
    }

    /// The movie page, or `None` if the movie is not in the catálogo.
    pub fn movie(&self, id: ObraId) -> Option<MoviePage> {
        let i = *self.index.get(&id)?;
        let m = &self.movies[i];
        let history = self.owner.notas.get(&id).cloned().unwrap_or_default();
        let people = |list: &[Person]| list.iter().map(|p| self.pessoa_card(p.id, p.name.clone())).collect();
        let genre = |id: &u32| self.generos.iter().find(|g| g.id == *id).map(|g| g.name.clone());
        let sem_a_nota = self.prevista_sem_a_nota(i);
        Some(MoviePage {
            card: self.card(i),
            faixa: self.previstas[i].map(|(p, _)| p).or(sem_a_nota).map(|p| display::faixa(p, self.ranked_de(i))),
            prevista_sem_a_nota: sem_a_nota,
            publico: (m.vote_count >= if id.is_serie() { PISO_MIN_VOTOS } else { filtro::PUBLICO_MIN_VOTOS }).then_some(m.vote_average / 2.0),
            critica: m.critic_score,
            backdrop_url: m.backdrop_path.as_ref().map(|p| format!("{BACKDROP_BASE}{p}")),
            tmdb_url: format!("https://www.themoviedb.org/{}/{}", if id.is_serie() { "tv" } else { "movie" }, id.tmdb_id()),
            runtime: m.runtime,
            temporadas: m.number_of_seasons,
            situacao: m.situacao.clone(),
            generos: m.genre_ids.iter().filter_map(genre).collect(),
            idioma: m.original_language.clone(),
            sinopse: m.overview.clone(),
            trailer_youtube: m.trailer_youtube.clone(),
            direcao: people(&m.directors),
            elenco: people(&m.cast),
            trilha: people(&m.composers),
            porque: self.porque(i),
            parecidos: self.parecidos(id, 0),
            nota: history.last().map(|e| e.nota),
            reacao: history.last().and_then(|e| e.reacao),
            history,
            quero_assistir: self.owner.quero_assistir.contains(&id),
            ja_vi: self.owner.ja_vi.contains(&id),
            assistindo: self.owner.assistindo.contains(&id),
            nao_me_interessa: self.owner.nao_me_interessa.contains(&id),
            onde_assistir: self.movies[i].watch.as_ref().map(|w| {
                let named = |ids: &[u32]| ids.iter().filter_map(|id| self.provedores.iter().find(|p| p.id == *id).cloned()).collect();
                OndeAssistir { link: w.link.clone(), assinatura: named(&w.flatrate), aluguel: named(&w.rent), compra: named(&w.buy) }
            }),
            pesou: self.owner.pesou.get(&id).map_or_else(Vec::new, |a| a.iter().copied().collect()),
            lista_temporadas: m
                .temporadas
                .iter()
                .map(|t| {
                    let history = self.owner.notas_temporada.get(&id).and_then(|n| n.get(&t.numero)).cloned().unwrap_or_default();
                    TemporadaPagina {
                        numero: t.numero,
                        nome: t.nome.clone(),
                        episodios: t.episodios,
                        ano: t.ano,
                        avaliavel: self.avaliavel(i, t.numero),
                        nota: history.last().map(|e| e.nota),
                        reacao: history.last().and_then(|e| e.reacao),
                        history,
                    }
                })
                .collect(),
        })
    }

    /// The next parecidos after the first `desde`, most alike first: "ver mais" pages through them.
    pub fn parecidos(&self, id: ObraId, desde: usize) -> Vec<MovieCard> {
        let Some(&i) = self.index.get(&id) else { return Vec::new() };
        self.alike(i, desde + PARECIDOS).into_iter().skip(desde).map(|j| self.card(j)).collect()
    }

    /// "Parecidos, mas…": of the movies most alike to this one, those further in `direcao` than
    /// it, most alike first. A movie with no value to compare (no genome scores, no runtime)
    /// compares against its parecidos' median; parecidos without one are left out.
    pub fn parecidos_mas(&self, id: ObraId, direcao: Direcao) -> Vec<MovieCard> {
        let Some(&i) = self.index.get(&id).filter(|_| !id.is_serie()) else { return Vec::new() };
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

    /// Item-item among movies with collaborative factors, or among séries or among animes through
    /// their factors, of content or their own: the `n` most like this one's, weighed by confiança so sparsely rated movies
    /// do not crowd the top, most alike first. Not those marked não me interessa.
    fn alike(&self, i: usize, n: usize) -> Vec<usize> {
        let tipo = self.movies[i].tipo();
        let Some(item) = self.items[i].as_ref().filter(|item| tipo != Tipo::Filme || item.collaborative) else { return Vec::new() };
        let mut scored: Vec<(f32, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i && !self.dispensado(j))
            .filter_map(|(j, other)| {
                let same = other.as_ref().filter(|o| self.movies[j].tipo() == tipo && (tipo != Tipo::Filme || o.collaborative))?;
                Some((item.similarity(same) * same.confianca, j))
            })
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

    /// Titles of the loved movies most like this one; a série never explains a filme, and animes
    /// read factors of their own, so only they explain each other.
    fn porque(&self, i: usize) -> Vec<String> {
        let Some(item) = &self.items[i] else { return Vec::new() };
        let tipo = self.movies[i].tipo();
        let explica = |id: ObraId| match (tipo, self.tipo(id)) {
            (Tipo::Serie, Tipo::Anime) | (Tipo::Anime, Tipo::Filme | Tipo::Serie) | (Tipo::Filme, Tipo::Serie | Tipo::Anime) => false,
            _ => true,
        };
        let mut loved: Vec<(f32, usize)> = self
            .owner
            .notas
            .iter()
            .filter(|(id, h)| h.last().is_some_and(|e| e.nota >= LOVED) && explica(**id))
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
        self.accuracy_with(self.model.as_ref()?, fraction, true)
    }

    /// `content`: also measure the movies outside the model; the accuracy gate never reads it.
    fn accuracy_with(&self, model: &Model, fraction: f32, content: bool) -> Option<Accuracy> {
        let (known, hidden) = recommender::hold_out(&self.notas_de_filmes(), fraction);
        // Calibrated on the notas left in, so the hidden ones never pick their own decay.
        let half_life = self.half_life(Some(model), &known);
        let errors = recommender::errors(model, &known, &hidden, half_life, |id| model.item(id, &[]));
        let movielens: Vec<f32> =
            errors.iter().filter_map(|(_, e)| Some((e.movielens_pred? as f32 - e.nota).abs())).collect();
        let errors: Vec<f32> = errors.iter().map(|(err, _)| *err).collect();
        // Movies outside the model, predicted from their catálogo content, if loaded.
        let content_only = |id| match model.item(id, &[]) {
            Some(_) => None,
            None => model.item(id, &self.movies[*self.index.get(&id)?].content_features()),
        };
        let content: Vec<f32> = match content {
            true => recommender::errors(model, &known, &hidden, half_life, content_only).iter().map(|(err, _)| *err).collect(),
            false => Vec::new(),
        };
        let mean = |e: &[f32]| (!e.is_empty()).then(|| e.iter().sum::<f32>() / e.len() as f32);
        Some(Accuracy {
            held_out: hidden.len(),
            notas: errors.len(),
            mae: mean(&errors)?,
            movielens_mae: mean(&movielens),
            content_notas: content.len(),
            content_mae: mean(&content),
        })
    }

    /// Hides the `fraction` (0–1) of the owner's notas on séries given most recently, folds in the
    /// rest (those on filmes too), with notas on séries weighing `peso`, and predicts the hidden
    /// ones. `None` without a model or when none of them can be predicted.
    pub fn accuracy_series(&self, fraction: f32, peso: f32) -> Option<AccuracySeries> {
        let model = self.model.as_ref()?;
        let series: BTreeMap<ObraId, Vec<NotaEntry>> = self.owner.notas.iter().filter(|(&id, _)| self.tipo(id) == Tipo::Serie).map(|(&id, h)| (id, h.clone())).collect();
        let (mut known, hidden) = recommender::hold_out(&series, fraction);
        known.extend(self.notas_de_filmes());
        let profile = self.fold_in_series(model, &known, peso);
        // (below the floor, error, nota) of each hidden nota the model can read.
        let medidos: Vec<(bool, f32, f32)> = hidden
            .iter()
            .filter_map(|(id, e)| {
                let i = *self.index.get(id)?;
                let erro = (model.predict(&profile, self.items[i].as_ref()?) - e.nota).abs();
                Some((self.abaixo_do_piso(i), erro, e.nota))
            })
            .collect();
        let erro = |grupo: Option<bool>| {
            let de: Vec<_> = medidos.iter().filter(|m| grupo.is_none_or(|g| m.0 == g)).collect();
            let n = de.len() as f32;
            (!de.is_empty()).then(|| ErroSeries {
                notas: de.len(),
                mae: de.iter().map(|m| m.1).sum::<f32>() / n,
                media: de.iter().map(|m| m.2).sum::<f32>() / n,
            })
        };
        Some(AccuracySeries { geral: erro(None)?, acima: erro(Some(false)), abaixo: erro(Some(true)) })
    }

    /// Hides the `fraction` (0–1) of the owner's notas on animes given most recently, a temporada's
    /// counting as one, with every other nota on those animes so none echoes them, and predicts
    /// the hidden ones as `fold_in_animes` reads them. `None` without a model or when none can be
    /// predicted. Informational: animes have no gate.
    pub fn accuracy_animes(&self, fraction: f32) -> Option<ErroSeries> {
        let model = self.anime_model.as_ref()?;
        let inteiras = self.owner.notas.iter().filter_map(|(&id, h)| Some((id, None, h.last()?)));
        let de_temporada = self.owner.notas_temporada.iter().flat_map(|(&id, de)| de.iter().filter_map(move |(&t, h)| Some((id, Some(t), h.last()?))));
        let mut notas: Vec<(ObraId, Option<u32>, &NotaEntry)> = inteiras.chain(de_temporada).filter(|n| self.tipo(n.0) == Tipo::Anime).collect();
        notas.sort_by_key(|n| std::cmp::Reverse(n.2.at));
        notas.truncate(((notas.len() as f32 * fraction).round() as usize).min(notas.len()));
        let profile = self.fold_in_animes(model, &notas.iter().map(|n| n.0).collect());
        let medidos: Vec<(f32, f32)> = notas
            .iter()
            .filter_map(|&(id, t, e)| {
                let base = self.items.get(*self.index.get(&id)?)?.as_ref()?;
                let item = match (t, model.temporadas.get(&id)) {
                    (Some(t), Some(fatores)) => fatores.get(&t)?,
                    _ => base,
                };
                Some(((model.predict(&profile, item) - e.nota).abs(), e.nota))
            })
            .collect();
        let n = medidos.len() as f32;
        (!medidos.is_empty()).then(|| ErroSeries {
            notas: medidos.len(),
            mae: medidos.iter().map(|m| m.0).sum::<f32>() / n,
            media: medidos.iter().map(|m| m.1).sum::<f32>() / n,
        })
    }

    /// The catálogo obras passing `filtro`, recomendações first ("Para você": best percentile
    /// within their type first, popularity weighed by `filtro.garimpo`), then the rest in
    /// catálogo order.
    pub fn explorar(&self, filtro: &Filtro, page: usize) -> ExplorarPage {
        let matcher = filtro::Matcher::new(filtro);
        let mut relevancia = vec![(false, 0); self.movies.len()];
        let passes = |&i: &usize| matcher.passes(self, i).map(|r| relevancia[i] = r).is_some();
        let mut found: Vec<usize> = if filtro.nao_me_interessa {
            (0..self.movies.len()).filter(|&i| self.dispensado(i)).filter(passes).collect()
        } else if filtro.tipo.is_some() && filtro.garimpo == 0.0 && filtro.ja_vi != Mostrar::So && filtro.busca.trim().is_empty() {
            // Para você's order is the answer: stop at the page, and one more to tell has_more.
            self.para_voce.iter().copied().filter(passes).take((page + 1) * PAGE_SIZE + 1).collect()
        } else {
            self.para_voce.iter().copied().filter(passes).collect()
        };
        // Já vi movies are no recomendações, so Para você leaves them in catálogo order; shown
        // alone (Para rever) they rank too.
        let so_ja_vi = filtro.ja_vi == Mostrar::So;
        let ordenado = |i: usize| if so_ja_vi { self.previstas[i].is_some() } else { self.recomendavel(i) };
        if so_ja_vi {
            found.sort_by_key(|&i| !ordenado(i));
        }
        // On Tudo, places are among what passes `filtro`, whatever the garimpo.
        if so_ja_vi || filtro.garimpo != 0.0 || filtro.tipo.is_none() {
            // ponytail: popularity is log TMDB votes, one star at either end of the slider; tune
            // the weight once the owner has used it.
            let score = |i: usize| self.ranking(i) - filtro.garimpo * self.popularidade[i];
            let n = found.partition_point(|&i| ordenado(i));
            self.por_percentil(&mut found[..n], score);
        }
        // Stable: equally good matches keep the order above.
        found.sort_by_key(|&i| relevancia[i]);
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
            id: m.id,
            title: m.title.clone(),
            tipo: m.tipo(),
            original_title: m.original_title.clone(),
            year: m.year,
            poster_url: m.poster_path.as_ref().map(|p| format!("{POSTER_BASE}{p}")),
            nota_prevista: self.previstas[i].map(|(p, _)| p),
            confianca: self.previstas[i].map(|(_, c)| c),
            exibicao: self.previstas[i]
                .and_then(|(p, c)| display::exibicao(self.owner.settings.display_mode, p, c, self.ranked_de(i))),
        }
    }
}

/// `n` of `pool`, in pool order: the first `n` for `sorteio` 0, else those whose id
/// hashes lowest with it, so a movie leaving the pool frees only its own place.
fn sortear<T>(pool: Vec<T>, n: usize, sorteio: u64, id: impl Fn(&T) -> ObraId) -> Vec<T> {
    if sorteio == 0 || pool.len() <= n {
        return pool.into_iter().take(n).collect();
    }
    // SplitMix64's finalizer.
    let hash = |x: &T| {
        let mut z = (sorteio ^ u64::from(id(x).tmdb_id())).wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    let mut drawn: Vec<(u64, usize)> = pool.iter().map(hash).zip(0..).collect();
    drawn.sort_unstable();
    let mut keep = vec![false; pool.len()];
    drawn.iter().take(n).for_each(|&(_, i)| keep[i] = true);
    pool.into_iter().zip(keep).filter_map(|(x, keep)| keep.then_some(x)).collect()
}
