//! Seam 2: the catalog job, run on a tiny TMDB fixture, produces an artifact the core loads.

use std::collections::BTreeMap;
use std::sync::Mutex;

use cinelens_catalog::{build, Config, Critica};
use cinelens_core::{Core, Filtro};
use serde_json::{json, Value};

/// Answers requests from the fixture. Discover pages not listed are empty years; anything
/// else not listed is a TMDB 404.
fn fixture_tmdb(request: &str) -> Result<Option<Value>, String> {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/tmdb.json")).unwrap();
    match fixture.get(request) {
        Some(response) => Ok(Some(response.clone())),
        None if request.starts_with("/discover/movie") => {
            Ok(Some(json!({"page": 1, "total_pages": 1, "results": []})))
        }
        None => Ok(None),
    }
}

/// A day with no OMDb budget left.
fn no_omdb(_: &str) -> Result<Option<Value>, String> {
    panic!("OMDb asked with no budget")
}

fn config(omdb_budget: usize) -> Config<'static> {
    Config {
        today: "2026-10-25",
        recent_since: "2026-04-28",
        // 999999 is a training-data id TMDB no longer knows.
        extra_ids: &[13, 999999],
        omdb_budget,
    }
}

fn fixture_artifact() -> Vec<u8> {
    serde_json::to_vec(&build(&fixture_tmdb, &no_omdb, &mut BTreeMap::new(), &config(0)).unwrap()).unwrap()
}

#[test]
fn core_queries_the_built_catalogo_most_voted_first() {
    let core = Core::open(&fixture_artifact()).unwrap();
    assert_eq!(core.data_version(), Some("2026-10-25"));

    let page = core.explorar(&Filtro::default(), 0);
    let ids: Vec<u32> = page.movies.iter().map(|m| m.tmdb_id).collect();
    // Discover (both 1999 pages), the training-data extra and the recent release; 999999 skipped.
    assert_eq!(ids, [550, 13, 603, 1311]);
    assert!(!page.has_more);

    let gump = &page.movies[1];
    assert_eq!(gump.title, "Forrest Gump: O Contador de Histórias");
    assert_eq!(gump.original_title, "Forrest Gump");
    assert_eq!(gump.year, Some(1994));
    assert_eq!(gump.poster_url.as_deref(), Some("https://image.tmdb.org/t/p/w342/fg.jpg"));
}

#[test]
fn artifact_carries_genres_people_trailers_trending_and_work_type() {
    let artifact: Value = serde_json::from_slice(&fixture_artifact()).unwrap();

    assert_eq!(artifact["format_version"], 1);
    assert_eq!(artifact["genres"][3], json!({"id": 878, "name": "Ficção científica"}));
    assert_eq!(artifact["trending"], json!([603, 1311]));

    let fight_club = &artifact["movies"][0];
    assert_eq!(fight_club["work_type"], "movie");
    assert_eq!(fight_club["genre_ids"], json!([18]));
    assert_eq!(fight_club["directors"], json!([{"id": 900, "name": "David Fincher"}]));
    assert_eq!(fight_club["composers"], json!([{"id": 901, "name": "Compositor Fight Club"}]));
    assert_eq!(fight_club["cast"][1], json!({"id": 101, "name": "Edward Norton"}));
    // A pt-BR trailer wins over the English one; teasers are not trailers.
    assert_eq!(fight_club["trailer_youtube"], "pt550");
    assert_eq!(artifact["movies"][2]["trailer_youtube"], "en603");
    assert_eq!(artifact["movies"][1]["trailer_youtube"], Value::Null);
}

#[test]
fn build_fails_instead_of_publishing_a_partial_catalogo() {
    let flaky = |request: &str| {
        if request.starts_with("/movie/603") {
            Err("HTTP 500".to_string())
        } else {
            fixture_tmdb(request)
        }
    };
    let config = Config { extra_ids: &[], ..config(0) };
    assert!(build(&flaky, &no_omdb, &mut BTreeMap::new(), &config).is_err());
}

#[test]
fn artifact_carries_brazil_watch_providers() {
    let artifact: Value = serde_json::from_slice(&fixture_artifact()).unwrap();

    // Only providers some movie is on in Brazil, by name.
    assert_eq!(
        artifact["providers"],
        json!([{"id": 2, "name": "Apple TV"}, {"id": 1899, "name": "Max"}, {"id": 8, "name": "Netflix"}])
    );
    assert_eq!(
        artifact["movies"][0]["watch"],
        json!({
            "link": "https://www.themoviedb.org/movie/550-fight-club/watch?locale=BR",
            "flatrate": [8],
            "rent": [2],
            "buy": [2]
        })
    );
    // Forrest Gump is nowhere in Brazil.
    assert_eq!(artifact["movies"][1]["watch"], Value::Null);
}

#[test]
fn core_lists_where_to_watch_a_built_movie() {
    let core = Core::open(&fixture_artifact()).unwrap();

    let matrix = core.movie(603).unwrap().onde_assistir.unwrap();
    let names = |p: &[cinelens_core::Provedor]| p.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&matrix.assinatura), ["Max", "Netflix"]);
    assert!(matrix.aluguel.is_empty() && matrix.compra.is_empty());
    assert_eq!(names(core.provedores()), ["Apple TV", "Max", "Netflix"]);
    assert!(core.movie(13).unwrap().onde_assistir.is_none());
}

#[test]
fn artifact_carries_the_brazilian_age_rating() {
    let artifact: Value = serde_json::from_slice(&fixture_artifact()).unwrap();

    // The first non-empty Brazilian rating; other countries' are ignored.
    assert_eq!(artifact["movies"][0]["certification"], "18");
    assert_eq!(artifact["movies"][1]["certification"], Value::Null);
}

/// OMDb answers by IMDb id; `asked` records the ids in order.
fn fixture_omdb<'a>(asked: &'a Mutex<Vec<String>>) -> impl Fn(&str) -> Result<Option<Value>, String> + Sync + 'a {
    move |imdb_id: &str| {
        asked.lock().unwrap().push(imdb_id.to_string());
        let rt = |v: &str| json!([{"Source": "Internet Movie Database", "Value": "8.8/10"}, {"Source": "Rotten Tomatoes", "Value": v}]);
        Ok(Some(match imdb_id {
            "tt0000550" => json!({"Response": "True", "Metascore": "66", "Ratings": rt("79%")}),
            "tt0000013" => json!({"Response": "True", "Metascore": "N/A", "Ratings": rt("71%")}),
            "tt0000603" => json!({"Response": "True", "Metascore": "73", "Ratings": []}),
            _ => json!({"Response": "False", "Error": "Incorrect IMDb ID."}),
        }))
    }
}

fn critic_scores(artifact: &Value) -> Vec<(u64, Value)> {
    artifact["movies"].as_array().unwrap().iter().map(|m| (m["tmdb_id"].as_u64().unwrap(), m["critic_score"].clone())).collect()
}

#[test]
fn critic_scores_come_from_omdb_within_the_daily_budget_and_are_kept_between_runs() {
    let asked = Mutex::new(Vec::new());
    let omdb = fixture_omdb(&asked);
    // An earlier run already has Matrix.
    let mut cache = BTreeMap::from([("tt0000603".to_string(), Critica { score: Some(73), fetched: "2026-10-20".into() })]);

    let artifact = serde_json::to_value(build(&fixture_tmdb, &omdb, &mut cache, &config(2)).unwrap()).unwrap();
    // Missing ones, most voted first; the recent release waits for tomorrow's budget.
    assert_eq!(*asked.lock().unwrap(), ["tt0000550", "tt0000013"]);
    // Metacritic, else Rotten Tomatoes; Matrix from the earlier run.
    assert_eq!(critic_scores(&artifact), [(550, json!(66)), (13, json!(71)), (603, json!(73)), (1311, Value::Null)]);

    asked.lock().unwrap().clear();
    let tomorrow = Config { today: "2026-10-26", ..config(3) };
    let artifact = serde_json::to_value(build(&fixture_tmdb, &omdb, &mut cache, &tomorrow).unwrap()).unwrap();
    // What is still missing, then the stalest; nothing fetched today again.
    assert_eq!(*asked.lock().unwrap(), ["tt0001311", "tt0000603", "tt0000550"]);
    // OMDb does not know the recent release: kept as known to have no score.
    assert_eq!(critic_scores(&artifact)[3], (1311, Value::Null));
    assert_eq!(cache["tt0001311"].score, None);
    assert_eq!(cache["tt0001311"].fetched, "2026-10-26");

    // A second run the same day only has what is left of the day's budget.
    asked.lock().unwrap().clear();
    let again = Config { today: "2026-10-26", ..config(4) };
    build(&fixture_tmdb, &omdb, &mut cache, &again).unwrap();
    assert_eq!(*asked.lock().unwrap(), ["tt0000013"]);
}

#[test]
fn omdb_trouble_leaves_the_catalogo_without_new_critic_scores() {
    let asked = Mutex::new(Vec::new());
    let limited = |imdb_id: &str| {
        asked.lock().unwrap().push(imdb_id.to_string());
        Ok(Some(json!({"Response": "False", "Error": "Request limit reached!"})))
    };
    let mut cache = BTreeMap::from([("tt0000603".to_string(), Critica { score: Some(73), fetched: "2026-10-20".into() })]);
    let artifact = serde_json::to_value(build(&fixture_tmdb, &limited, &mut cache, &config(4)).unwrap()).unwrap();
    // Stops at the first refusal; the catálogo is still published with what was known.
    assert_eq!(asked.lock().unwrap().len(), 1);
    assert_eq!(critic_scores(&artifact)[2], (603, json!(73)));
    assert_eq!(cache.len(), 1);

    let down = |_: &str| Err("HTTP 503".to_string());
    assert!(build(&fixture_tmdb, &down, &mut cache, &config(4)).is_ok());
}

#[test]
fn core_reads_the_critic_scores() {
    let asked = Mutex::new(Vec::new());
    let artifact = build(&fixture_tmdb, &fixture_omdb(&asked), &mut BTreeMap::new(), &config(4)).unwrap();
    let core = Core::open(&serde_json::to_vec(&artifact).unwrap()).unwrap();
    let filtro = Filtro { critica_min: Some(70), ..Default::default() };
    let ids: Vec<u32> = core.explorar(&filtro, 0).movies.iter().map(|m| m.tmdb_id).collect();
    assert_eq!(ids, [13, 603]);
}
