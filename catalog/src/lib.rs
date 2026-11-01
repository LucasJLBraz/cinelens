//! The daily catalog job: builds the catálogo artifact from TMDB, with critics' scores from OMDb,
//! and the séries' artifact, published apart so installed apps without séries never see it.
//!
//! `build` only sees TMDB and OMDb through `fetch` and `omdb`, so the same code runs against the
//! real APIs (see `main.rs`) and against fixtures in tests.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use cinelens_core::{read_mapeamento, ObraId};

/// Feature films: TMDB's discover keeps movies at least this long (minutes).
const MIN_RUNTIME: u32 = 40;
/// Older movies need this many TMDB votes to enter the catálogo.
const MIN_VOTES: u32 = 50;
/// Recent releases enter with far fewer votes; below this they are mostly placeholders.
const MIN_VOTES_RECENT: u32 = 5;
const FIRST_YEAR: u32 = 1900;
/// TMDB refuses pages beyond this.
const MAX_PAGES: u32 = 500;
const CAST_SIZE: usize = 8;
const DETAIL_WORKERS: usize = 8;
/// TMDB's kinds of série that enter the catálogo, and its `with_type` codes for them.
const SERIE_TYPES: [&str; 3] = ["Scripted", "Miniseries", "Documentary"];
const SERIE_TYPE_CODES: &str = "0|2|4";
/// News, Reality, Soap (daily telenovelas) and Talk, whatever TMDB's type says.
const SERIE_GENEROS_FORA: [u32; 4] = [10763, 10764, 10766, 10767];
const ANIMACAO: u32 = 16;
/// Anime in style that AniList lacks and TMDB's keyword misses: Castlevania, Blue Eye Samurai and
/// The Boys Presents: Diabolical.
const ANIMES_MANUAIS: [u32; 3] = [71024, 225180, 152483];
/// Stands for AniList popularity on the manual list, which AniList lacks: past any vote cut the
/// download limit should ever need.
const ANILIST_MANUAL: u32 = 100_000_000;
/// AniList popularity that brings an anime in whatever its TMDB votes; the vote cut rises from
/// `MIN_VOTES`, and this one with it.
pub const ANILIST_POPULAR: u32 = 5_000;
/// How many AniList users weigh like one TMDB vote at the cut: `ANILIST_POPULAR / MIN_VOTES`.
const ANILIST_POR_VOTO: u32 = ANILIST_POPULAR / MIN_VOTES;

/// One TMDB GET, given as path plus query. `Ok(None)` is a 404.
pub trait Fetch: Fn(&str) -> Result<Option<Value>, String> + Sync {}
impl<F: Fn(&str) -> Result<Option<Value>, String> + Sync> Fetch for F {}

/// What OMDb said of a movie on one day; kept between runs by IMDb id, as OMDb allows only
/// 1000 requests a day.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Critica {
    /// Metacritic, else Rotten Tomatoes, 0–100; `None` when OMDb has neither.
    pub score: Option<u8>,
    /// `YYYY-MM-DD`.
    pub fetched: String,
}

/// What AniList says of the TMDB TV obras it knows, through `Fribb/anime-lists`. The source of
/// no entry in the catálogo, only of recognizing and keeping them.
#[derive(Default)]
pub struct AniList {
    obras: HashMap<u32, ObraAniList>,
}

#[derive(Default)]
struct ObraAniList {
    popularidade: u32,
    /// AniList has it, not only MyAnimeList.
    conhecida: bool,
    /// TMDB seasons with a serialized entry of their own.
    temporadas: BTreeSet<u32>,
}

impl AniList {
    /// `mapping` is `anime-list-full.json`; `popularidade` is by AniList id.
    pub fn new(mapping: &[u8], popularidade: &BTreeMap<u32, u32>) -> Result<AniList, String> {
        let mut obras: HashMap<u32, ObraAniList> = HashMap::new();
        for row in read_mapeamento(mapping)? {
            let Some(tv) = row.tmdb_tv() else { continue };
            let obra = obras.entry(tv).or_default();
            if let Some(anilist) = row.anilist_id {
                obra.conhecida = true;
                obra.popularidade = obra.popularidade.max(popularidade.get(&anilist).copied().unwrap_or(0));
            }
            // As the trainer joins them: a run the mapping does not split is its first temporada.
            if row.serializado() && row.mal_id.is_some() {
                obra.temporadas.insert(row.temporada().unwrap_or(1));
            }
        }
        Ok(AniList { obras })
    }

    fn popularidade(&self, tv: u32) -> u32 {
        self.obras.get(&tv).map_or(0, |o| o.popularidade)
    }
}

pub struct Config<'a> {
    /// `YYYY-MM-DD`; also the artifact's data version.
    pub today: &'a str,
    /// `YYYY-MM-DD`; releases from this day on enter with fewer votes.
    pub recent_since: &'a str,
    /// Movies that must be in the catálogo whatever their votes (the training data).
    pub extra_ids: &'a [u32],
    /// OMDb requests a day; what earlier runs today asked counts against it.
    pub omdb_budget: usize,
    pub anilist: &'a AniList,
}

#[derive(Serialize)]
pub struct Artifact {
    format_version: u32,
    data_version: String,
    genres: Vec<Genre>,
    /// TMDB trending this week, in TMDB's order.
    trending: Vec<ObraId>,
    /// Streaming, rental and purchase services some movie is on in Brazil, by name.
    providers: Vec<Provider>,
    /// Most voted first.
    movies: Vec<Movie>,
}

impl Artifact {
    pub fn obras(&self) -> usize {
        self.movies.len()
    }

    pub fn set_data_version(&mut self, version: &str) {
        self.data_version = version.to_string();
    }

    /// Raises the vote cut until `size` is at most `max`, dropping the obras below it (and below
    /// it in AniList users, for an anime), and returns the cut; `MIN_VOTES` when it already fits.
    pub fn raise_votes_to_fit(&mut self, max: usize, size: impl Fn(&Artifact) -> usize) -> u32 {
        let mut cut = MIN_VOTES;
        while size(self) > max && !self.movies.is_empty() {
            cut += cut / 10 + 1;
            self.movies.retain(|m| m.vote_count >= cut || m.anilist >= cut.saturating_mul(ANILIST_POR_VOTO));
        }
        cut
    }
}

#[derive(Serialize, Deserialize)]
struct Genre {
    id: u32,
    name: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Person {
    id: u32,
    name: String,
}

#[derive(Serialize, Clone)]
struct Provider {
    id: u32,
    name: String,
    logo_path: Option<String>,
}

/// Where a movie is in Brazil (TMDB's data from JustWatch), as provider ids.
#[derive(Serialize)]
struct Watch {
    /// TMDB's watch page, which links on to each service.
    link: Option<String>,
    flatrate: Vec<u32>,
    rent: Vec<u32>,
    buy: Vec<u32>,
}

#[derive(Serialize)]
struct Movie {
    tmdb_id: ObraId,
    work_type: &'static str,
    title: String,
    original_title: String,
    year: Option<u16>,
    release_date: Option<String>,
    runtime: Option<u32>,
    genre_ids: Vec<u32>,
    original_language: String,
    countries: Vec<String>,
    overview: String,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    imdb_id: Option<String>,
    vote_count: u32,
    vote_average: f32,
    popularity: f32,
    directors: Vec<Person>,
    composers: Vec<Person>,
    cast: Vec<Person>,
    trailer_youtube: Option<String>,
    watch: Option<Watch>,
    /// The Brazilian age rating (L, 10, 12, 14, 16, 18).
    certification: Option<String>,
    /// Critics' score (`Critica::score`).
    critic_score: Option<u8>,
    /// Séries only.
    #[serde(skip_serializing_if = "Option::is_none")]
    number_of_seasons: Option<u32>,
    /// Séries only: `em_andamento` or `terminada`.
    #[serde(skip_serializing_if = "Option::is_none")]
    situacao: Option<&'static str>,
    /// Animes only.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    temporadas: Vec<Temporada>,
    /// AniList popularity, which only the vote cut reads.
    #[serde(skip)]
    anilist: u32,
}

/// One of an anime's temporadas, as TMDB lists it.
#[derive(Serialize)]
struct Temporada {
    numero: u32,
    nome: String,
    episodios: Option<u32>,
    ano: Option<u16>,
    /// MyAnimeList has an entry for it: there are ratings behind it.
    entrada: bool,
}

#[derive(Deserialize)]
struct Page {
    total_pages: u32,
    results: Vec<Id>,
}

#[derive(Deserialize)]
struct Id {
    id: u32,
}

#[derive(Deserialize)]
struct Details {
    id: u32,
    title: String,
    original_title: String,
    #[serde(default)]
    release_date: String,
    runtime: Option<u32>,
    genres: Vec<Genre>,
    original_language: String,
    production_countries: Vec<Country>,
    #[serde(default)]
    overview: String,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    imdb_id: Option<String>,
    vote_count: u32,
    vote_average: f32,
    popularity: f32,
    credits: Credits,
    videos: Videos,
    #[serde(rename = "watch/providers", default)]
    watch_providers: WatchProviders,
    #[serde(default)]
    release_dates: ReleaseDates,
}

/// A série on TMDB.
#[derive(Deserialize)]
struct SerieDetails {
    id: u32,
    name: String,
    original_name: String,
    first_air_date: Option<String>,
    #[serde(default)]
    episode_run_time: Vec<u32>,
    last_episode_to_air: Option<Episode>,
    genres: Vec<Genre>,
    original_language: String,
    #[serde(default)]
    origin_country: Vec<String>,
    #[serde(default)]
    production_countries: Vec<Country>,
    #[serde(default)]
    overview: String,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    vote_count: u32,
    vote_average: f32,
    popularity: f32,
    #[serde(default)]
    created_by: Vec<Person>,
    number_of_seasons: Option<u32>,
    #[serde(default)]
    status: String,
    #[serde(rename = "type", default)]
    kind: String,
    credits: Credits,
    videos: Videos,
    #[serde(rename = "watch/providers", default)]
    watch_providers: WatchProviders,
    #[serde(default)]
    content_ratings: ContentRatings,
    #[serde(default)]
    adult: bool,
    #[serde(default)]
    keywords: Keywords,
    #[serde(default)]
    seasons: Vec<Season>,
}

#[derive(Deserialize, Default)]
struct Keywords {
    results: Vec<Keyword>,
}

#[derive(Deserialize)]
struct Keyword {
    name: String,
}

#[derive(Deserialize)]
struct Season {
    season_number: u32,
    #[serde(default)]
    name: String,
    episode_count: Option<u32>,
    air_date: Option<String>,
}

#[derive(Deserialize)]
struct Episode {
    runtime: Option<u32>,
}

#[derive(Deserialize, Default)]
struct ContentRatings {
    results: Vec<ContentRating>,
}

#[derive(Deserialize)]
struct ContentRating {
    iso_3166_1: String,
    #[serde(default)]
    rating: String,
}

#[derive(Deserialize, Default)]
struct ReleaseDates {
    results: Vec<CountryReleases>,
}

#[derive(Deserialize)]
struct CountryReleases {
    iso_3166_1: String,
    release_dates: Vec<Release>,
}

#[derive(Deserialize)]
struct Release {
    #[serde(default)]
    certification: String,
}

#[derive(Deserialize, Default)]
struct WatchProviders {
    results: HashMap<String, Region>,
}

#[derive(Deserialize)]
struct Region {
    link: Option<String>,
    #[serde(default)]
    flatrate: Vec<RegionProvider>,
    #[serde(default)]
    rent: Vec<RegionProvider>,
    #[serde(default)]
    buy: Vec<RegionProvider>,
}

#[derive(Deserialize)]
struct RegionProvider {
    provider_id: u32,
    provider_name: String,
    #[serde(default)]
    logo_path: Option<String>,
}

#[derive(Deserialize)]
struct Country {
    iso_3166_1: String,
}

#[derive(Deserialize)]
struct Credits {
    cast: Vec<Person>,
    crew: Vec<Crew>,
}

#[derive(Deserialize)]
struct Crew {
    id: u32,
    name: String,
    job: String,
}

#[derive(Deserialize)]
struct Videos {
    results: Vec<Video>,
}

#[derive(Deserialize)]
struct Video {
    key: String,
    site: String,
    #[serde(rename = "type")]
    kind: String,
    iso_639_1: String,
}

/// `omdb` is asked for an IMDb id. `critica` is what earlier runs learned from OMDb; this run
/// adds to and refreshes it.
pub fn build(
    fetch: &impl Fetch,
    omdb: &impl Fetch,
    critica: &mut BTreeMap<String, Critica>,
    config: &Config,
) -> Result<Artifact, String> {
    let genres: GenreList = get(fetch, "/genre/movie/list?language=pt-BR")?;
    let trending: Vec<u32> = get::<Page>(fetch, "/trending/movie/week?language=pt-BR")?
        .results
        .into_iter()
        .map(|m| m.id)
        .collect();

    let mut ids: BTreeSet<u32> = config.extra_ids.iter().copied().collect();
    ids.extend(&trending);
    let this_year: u32 = config.today.get(..4).and_then(|y| y.parse().ok()).ok_or("bad today date")?;
    // Discover caps results at MAX_PAGES, so older movies are listed one year at a time.
    for year in FIRST_YEAR..=this_year {
        ids.extend(discover(fetch, MIN_VOTES, &format!("{year}-01-01"), &format!("{year}-12-31"))?);
    }
    ids.extend(discover(fetch, MIN_VOTES_RECENT, config.recent_since, config.today)?);

    let (mut movies, providers): (Vec<Movie>, Vec<Vec<Provider>>) =
        details(fetch, &ids.into_iter().collect::<Vec<_>>(), movie_request, |body| {
            serde_json::from_value::<Details>(body).map(|d| Some(movie(d)))
        })?
        .into_iter()
        .unzip();
    movies.sort_by_key(|m| (std::cmp::Reverse(m.vote_count), m.tmdb_id));
    critic_scores(omdb, critica, &movies, config);
    for m in &mut movies {
        m.critic_score = m.imdb_id.as_ref().and_then(|i| critica.get(i)?.score);
    }
    Ok(Artifact {
        format_version: cinelens_core::CATALOG_FORMAT_VERSION,
        data_version: config.today.to_string(),
        genres: genres.genres,
        trending: trending.into_iter().map(ObraId::filme).collect(),
        providers: by_name(providers),
        movies,
    })
}

/// The séries' artifact: same shape as the filmes', `tmdb_id` carrying each série's `ObraId`,
/// genres already as filme genres, and no critics' scores (OMDb is not asked).
pub fn build_series(fetch: &impl Fetch, config: &Config) -> Result<Artifact, String> {
    let trending: Vec<u32> =
        get::<Page>(fetch, "/trending/tv/week?language=pt-BR")?.results.into_iter().map(|m| m.id).collect();
    let mut ids: BTreeSet<u32> = trending.iter().copied().collect();
    let this_year: u32 = config.today.get(..4).and_then(|y| y.parse().ok()).ok_or("bad today date")?;
    for year in FIRST_YEAR..=this_year {
        ids.extend(discover_series(fetch, MIN_VOTES, &format!("{year}-01-01"), &format!("{year}-12-31"))?);
    }
    ids.extend(discover_series(fetch, MIN_VOTES_RECENT, config.recent_since, config.today)?);

    let (mut movies, providers): (Vec<Movie>, Vec<Vec<Provider>>) =
        details(fetch, &ids.into_iter().collect::<Vec<_>>(), serie_request, |body| {
            serde_json::from_value::<SerieDetails>(body).map(|d| serie(d, config.anilist))
        })?
        .into_iter()
        .unzip();
    movies.sort_by_key(|m| (std::cmp::Reverse(m.vote_count), m.tmdb_id));
    Ok(Artifact {
        format_version: cinelens_core::CATALOG_FORMAT_VERSION,
        data_version: config.today.to_string(),
        genres: Vec::new(),
        trending: trending.into_iter().map(ObraId::serie).collect(),
        providers: by_name(providers),
        movies,
    })
}

/// The animes' artifact: same shape as the séries', `work_type` saying what they are and each
/// with its temporadas.
pub fn build_animes(fetch: &impl Fetch, config: &Config) -> Result<Artifact, String> {
    let trending: Vec<u32> =
        get::<Page>(fetch, "/trending/tv/week?language=pt-BR")?.results.into_iter().map(|m| m.id).collect();
    let mut ids: BTreeSet<u32> = trending.iter().copied().collect();
    ids.extend(ANIMES_MANUAIS);
    ids.extend(config.anilist.obras.iter().filter(|(_, o)| o.popularidade >= ANILIST_POPULAR).map(|(&id, _)| id));
    let this_year: u32 = config.today.get(..4).and_then(|y| y.parse().ok()).ok_or("bad today date")?;
    for year in FIRST_YEAR..=this_year {
        ids.extend(discover_animacao(fetch, MIN_VOTES, &format!("{year}-01-01"), &format!("{year}-12-31"))?);
    }
    ids.extend(discover_animacao(fetch, MIN_VOTES_RECENT, config.recent_since, config.today)?);

    let (mut movies, providers): (Vec<Movie>, Vec<Vec<Provider>>) =
        details(fetch, &ids.into_iter().collect::<Vec<_>>(), serie_request, |body| {
            serde_json::from_value::<SerieDetails>(body).map(|d| anime(d, config.anilist))
        })?
        .into_iter()
        .unzip();
    movies.sort_by_key(|m| (std::cmp::Reverse(m.vote_count), m.tmdb_id));
    Ok(Artifact {
        format_version: cinelens_core::CATALOG_FORMAT_VERSION,
        data_version: config.today.to_string(),
        genres: Vec::new(),
        trending: trending.into_iter().map(ObraId::anime).collect(),
        providers: by_name(providers),
        movies,
    })
}

fn by_name(providers: Vec<Vec<Provider>>) -> Vec<Provider> {
    let providers: BTreeMap<u32, Provider> = providers.into_iter().flatten().map(|p| (p.id, p)).collect();
    let mut providers: Vec<Provider> = providers.into_values().collect();
    providers.sort_by(|a, b| a.name.cmp(&b.name));
    providers
}

/// Spends the day's OMDb budget: movies never asked first, most voted first, then the stalest.
/// Trouble with OMDb (its daily limit, a bad key, the network) stops asking and is only warned
/// about: the catálogo goes out with the scores already known.
fn critic_scores(omdb: &impl Fetch, critica: &mut BTreeMap<String, Critica>, movies: &[Movie], config: &Config) {
    let mut ids: Vec<&String> = movies.iter().filter_map(|m| m.imdb_id.as_ref()).collect();
    // Stable: most voted first among the never asked and among the same day.
    ids.sort_by_key(|&i| critica.get(i).map(|c| c.fetched.clone()));
    let stale = ids.iter().filter(|&&i| critica.get(i).is_none_or(|c| c.fetched.as_str() < config.today));
    let asked_today = critica.values().filter(|c| c.fetched == config.today).count();
    let ids: Vec<String> = stale.take(config.omdb_budget.saturating_sub(asked_today)).map(|&i| i.clone()).collect();
    for imdb_id in ids {
        match omdb(&imdb_id).and_then(|body| omdb_score(&body.unwrap_or_default())) {
            Ok(score) => _ = critica.insert(imdb_id, Critica { score, fetched: config.today.to_string() }),
            Err(e) => {
                eprintln!("::warning::OMDb {imdb_id}: {e}; no more critics' scores this run");
                return;
            }
        }
    }
}

/// The score in an OMDb answer. A movie OMDb does not know has none; any other refusal is an error.
fn omdb_score(body: &Value) -> Result<Option<u8>, String> {
    if body["Response"] != "True" {
        let error = body["Error"].as_str().unwrap_or("no answer");
        return if error.contains("not found") || error.starts_with("Incorrect IMDb ID") { Ok(None) } else { Err(error.to_string()) };
    }
    // ponytail: Rotten Tomatoes is the share of positive reviews, not an average like Metacritic,
    // and clears the same cut more easily; give it its own cut if it lets weak movies in.
    let metascore = body["Metascore"].as_str().and_then(|s| s.parse().ok());
    let ratings = body["Ratings"].as_array().into_iter().flatten();
    let rotten = ratings
        .filter(|r| r["Source"] == "Rotten Tomatoes")
        .find_map(|r| r["Value"].as_str()?.strip_suffix('%')?.parse().ok());
    Ok(metascore.or(rotten).filter(|&s: &u8| s <= 100))
}

#[derive(Deserialize)]
struct GenreList {
    genres: Vec<Genre>,
}

fn get<T: serde::de::DeserializeOwned>(fetch: &impl Fetch, request: &str) -> Result<T, String> {
    let body = fetch(request)?.ok_or_else(|| format!("{request}: not found"))?;
    serde_json::from_value(body).map_err(|e| format!("{request}: {e}"))
}

fn discover(fetch: &impl Fetch, min_votes: u32, from: &str, to: &str) -> Result<Vec<u32>, String> {
    discover_pages(fetch, from, to, |page| {
        format!(
            "/discover/movie?language=pt-BR&include_adult=false&with_runtime.gte={MIN_RUNTIME}\
             &vote_count.gte={min_votes}&primary_release_date.gte={from}&primary_release_date.lte={to}&page={page}"
        )
    })
}

fn discover_series(fetch: &impl Fetch, min_votes: u32, from: &str, to: &str) -> Result<Vec<u32>, String> {
    discover_pages(fetch, from, to, |page| {
        format!(
            "/discover/tv?language=pt-BR&include_adult=false&with_type={SERIE_TYPE_CODES}\
             &vote_count.gte={min_votes}&first_air_date.gte={from}&first_air_date.lte={to}&page={page}"
        )
    })
}

fn discover_animacao(fetch: &impl Fetch, min_votes: u32, from: &str, to: &str) -> Result<Vec<u32>, String> {
    discover_pages(fetch, from, to, |page| {
        format!(
            "/discover/tv?language=pt-BR&include_adult=false&with_genres={ANIMACAO}\
             &vote_count.gte={min_votes}&first_air_date.gte={from}&first_air_date.lte={to}&page={page}"
        )
    })
}

fn discover_pages(fetch: &impl Fetch, from: &str, to: &str, request: impl Fn(u32) -> String) -> Result<Vec<u32>, String> {
    let mut ids = Vec::new();
    let mut page = 1;
    loop {
        let request = request(page);
        let result: Page = get(fetch, &request)?;
        if result.total_pages > MAX_PAGES {
            return Err(format!("{from}..{to} has {} pages, more than TMDB serves", result.total_pages));
        }
        ids.extend(result.results.into_iter().map(|m| m.id));
        if page >= result.total_pages {
            return Ok(ids);
        }
        page += 1;
    }
}

fn movie_request(id: u32) -> String {
    format!("/movie/{id}?language=pt-BR&append_to_response=credits,videos,watch/providers,release_dates&include_video_language=pt,en")
}

fn serie_request(id: u32) -> String {
    format!("/tv/{id}?language=pt-BR&append_to_response=credits,videos,watch/providers,content_ratings,keywords&include_video_language=pt,en")
}

/// Fetches every obra's details in parallel. One TMDB no longer knows, or `parse` leaves out, is
/// skipped; any other failure fails the build, so a partial catálogo is never published. Each
/// obra comes with the providers it is on in Brazil.
fn details<E: std::fmt::Display>(
    fetch: &impl Fetch,
    ids: &[u32],
    request: impl Fn(u32) -> String + Sync,
    parse: impl Fn(Value) -> Result<Option<(Movie, Vec<Provider>)>, E> + Sync,
) -> Result<Vec<(Movie, Vec<Provider>)>, String> {
    let next = AtomicUsize::new(0);
    let movies = Mutex::new(Vec::with_capacity(ids.len()));
    let failure = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..DETAIL_WORKERS {
            s.spawn(|| {
                while let Some(&id) = ids.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let request = request(id);
                    match fetch(&request).and_then(|body| match body {
                        None => Ok(None),
                        Some(body) => parse(body).map_err(|e| format!("{request}: {e}")),
                    }) {
                        Ok(Some(m)) => movies.lock().unwrap().push(m),
                        Ok(None) => {}
                        Err(e) => {
                            *failure.lock().unwrap() = Some(e);
                            next.store(ids.len(), Ordering::Relaxed);
                        }
                    }
                }
            });
        }
    });
    match failure.into_inner().unwrap() {
        Some(e) => Err(e),
        None => Ok(movies.into_inner().unwrap()),
    }
}

/// Where an obra is in Brazil.
fn watch(mut providers: WatchProviders) -> (Option<Watch>, Vec<Provider>) {
    let region = providers.results.remove("BR");
    let providers: Vec<Provider> = region
        .iter()
        .flat_map(|r| r.flatrate.iter().chain(&r.rent).chain(&r.buy))
        .map(|p| Provider { id: p.provider_id, name: p.provider_name.clone(), logo_path: p.logo_path.clone() })
        .collect();
    let ids = |list: &[RegionProvider]| list.iter().map(|p| p.provider_id).collect();
    let watch = region.map(|r| Watch { flatrate: ids(&r.flatrate), rent: ids(&r.rent), buy: ids(&r.buy), link: r.link });
    (watch, providers)
}

fn movie(mut d: Details) -> (Movie, Vec<Provider>) {
    let (watch, providers) = watch(std::mem::take(&mut d.watch_providers));
    (movie_details(d, watch), providers)
}

/// A pt-BR trailer, else an English one.
fn trailer(videos: &Videos) -> Option<String> {
    let of = |lang: &str| {
        videos.results.iter().find(|v| v.site == "YouTube" && v.kind == "Trailer" && v.iso_639_1 == lang).map(|v| v.key.clone())
    };
    of("pt").or_else(|| of("en"))
}

fn crew(credits: &Credits, job: &str) -> Vec<Person> {
    credits.crew.iter().filter(|c| c.job == job).map(|c| Person { id: c.id, name: c.name.clone() }).collect()
}

/// TMDB's TV genres that filmes lack, as the filme genres they correspond to, so Explorar's
/// genre filter and the content features read one vocabulary.
fn filme_genres(tv_genre: u32) -> Vec<u32> {
    match tv_genre {
        10759 => vec![28, 12],
        10765 => vec![878, 14],
        10768 => vec![10752],
        10762 => vec![10751],
        same => vec![same],
    }
}

impl SerieDetails {
    fn palavra_chave(&self, palavra: &str) -> bool {
        self.keywords.results.iter().any(|k| k.name.eq_ignore_ascii_case(palavra))
    }

    fn hentai(&self) -> bool {
        self.adult || self.palavra_chave("hentai")
    }

    /// TMDB lists it as animation, and it is Japanese, in AniList, under TMDB's keyword or on the
    /// manual list.
    fn estilo_anime(&self, anilist: &AniList) -> bool {
        self.genres.iter().any(|g| g.id == ANIMACAO)
            && (self.original_language == "ja"
                || self.origin_country.iter().any(|c| c == "JP")
                || anilist.obras.get(&self.id).is_some_and(|o| o.conhecida)
                || self.palavra_chave("anime")
                || ANIMES_MANUAIS.contains(&self.id))
    }
}

/// The série as an obra; `None` for what the catálogo leaves out. Discover already asks for the
/// right types, but trending does not, and TMDB labels some telenovelas as scripted. A série
/// without genres leaves too: nothing shows it is none of those. Animes go out on their own.
fn serie(d: SerieDetails, anilist: &AniList) -> Option<(Movie, Vec<Provider>)> {
    let has = |id: u32| d.genres.iter().any(|g| g.id == id);
    if d.genres.is_empty() || !SERIE_TYPES.contains(&d.kind.as_str()) || SERIE_GENEROS_FORA.iter().any(|&g| has(g)) || d.hentai() || d.estilo_anime(anilist) {
        return None;
    }
    Some(obra_tv(d, ObraId::serie, "tv"))
}

/// The anime as an obra; `None` for anything else.
fn anime(d: SerieDetails, anilist: &AniList) -> Option<(Movie, Vec<Provider>)> {
    if d.hentai() || !d.estilo_anime(anilist) {
        return None;
    }
    let entradas = anilist.obras.get(&d.id).map(|o| &o.temporadas);
    let temporadas = d.seasons.iter().filter(|s| s.season_number > 0).map(|s| Temporada {
        numero: s.season_number,
        nome: s.name.clone(),
        episodios: s.episode_count,
        ano: s.air_date.as_deref().and_then(|a| a.get(..4)).and_then(|y| y.parse().ok()),
        entrada: entradas.is_some_and(|e| e.contains(&s.season_number)),
    });
    let temporadas: Vec<Temporada> = temporadas.collect();
    let popularidade = if ANIMES_MANUAIS.contains(&d.id) { ANILIST_MANUAL } else { anilist.popularidade(d.id) };
    let (mut anime, providers) = obra_tv(d, ObraId::anime, "anime");
    anime.temporadas = temporadas;
    anime.anilist = popularidade;
    Some((anime, providers))
}

fn obra_tv(mut d: SerieDetails, id: fn(u32) -> ObraId, work_type: &'static str) -> (Movie, Vec<Provider>) {
    let (watch, providers) = watch(std::mem::take(&mut d.watch_providers));
    let mut genre_ids: Vec<u32> = Vec::new();
    for g in d.genres.iter().flat_map(|g| filme_genres(g.id)) {
        if !genre_ids.contains(&g) {
            genre_ids.push(g);
        }
    }
    let serie = Movie {
        tmdb_id: id(d.id),
        work_type,
        directors: d.created_by,
        composers: crew(&d.credits, "Original Music Composer"),
        cast: d.credits.cast.iter().take(CAST_SIZE).cloned().collect(),
        trailer_youtube: trailer(&d.videos),
        certification: d.content_ratings.results.into_iter().find(|c| c.iso_3166_1 == "BR" && !c.rating.is_empty()).map(|c| c.rating),
        year: d.first_air_date.as_deref().and_then(|r| r.get(..4)).and_then(|y| y.parse().ok()),
        release_date: d.first_air_date.filter(|r| !r.is_empty()),
        title: d.name,
        original_title: d.original_name,
        runtime: d.episode_run_time.first().copied().or(d.last_episode_to_air.and_then(|e| e.runtime)).filter(|&r| r > 0),
        genre_ids,
        original_language: d.original_language,
        countries: d.production_countries.into_iter().map(|c| c.iso_3166_1).collect(),
        overview: d.overview,
        poster_path: d.poster_path,
        backdrop_path: d.backdrop_path,
        imdb_id: None,
        vote_count: d.vote_count,
        vote_average: d.vote_average,
        popularity: d.popularity,
        watch,
        critic_score: None,
        number_of_seasons: d.number_of_seasons,
        situacao: Some(if matches!(d.status.as_str(), "Ended" | "Canceled") { "terminada" } else { "em_andamento" }),
        temporadas: Vec::new(),
        anilist: 0,
    };
    (serie, providers)
}

fn movie_details(d: Details, watch: Option<Watch>) -> Movie {
    Movie {
        tmdb_id: ObraId::filme(d.id),
        work_type: "movie",
        directors: crew(&d.credits, "Director"),
        composers: crew(&d.credits, "Original Music Composer"),
        cast: d.credits.cast.iter().take(CAST_SIZE).cloned().collect(),
        trailer_youtube: trailer(&d.videos),
        certification: d
            .release_dates
            .results
            .iter()
            .filter(|c| c.iso_3166_1 == "BR")
            .flat_map(|c| &c.release_dates)
            .map(|r| r.certification.clone())
            .find(|c| !c.is_empty()),
        year: d.release_date.get(..4).and_then(|y| y.parse().ok()),
        release_date: Some(d.release_date).filter(|r| !r.is_empty()),
        title: d.title,
        original_title: d.original_title,
        runtime: d.runtime.filter(|&r| r > 0),
        genre_ids: d.genres.iter().map(|g| g.id).collect(),
        original_language: d.original_language,
        countries: d.production_countries.into_iter().map(|c| c.iso_3166_1).collect(),
        overview: d.overview,
        poster_path: d.poster_path,
        backdrop_path: d.backdrop_path,
        imdb_id: d.imdb_id.filter(|i| !i.is_empty()),
        vote_count: d.vote_count,
        vote_average: d.vote_average,
        popularity: d.popularity,
        watch,
        critic_score: None,
        number_of_seasons: None,
        situacao: None,
        temporadas: Vec::new(),
        anilist: 0,
    }
}
