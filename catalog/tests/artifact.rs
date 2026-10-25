//! Seam 2: the catalog job, run on a tiny TMDB fixture, produces an artifact the core loads.

use cinelens_catalog::{build, Config};
use cinelens_core::Core;
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

fn fixture_artifact() -> Vec<u8> {
    let config = Config {
        today: "2026-10-25",
        recent_since: "2026-04-28",
        // 999999 is a training-data id TMDB no longer knows.
        extra_ids: &[13, 999999],
    };
    serde_json::to_vec(&build(&fixture_tmdb, &config).unwrap()).unwrap()
}

#[test]
fn core_queries_the_built_catalogo_most_voted_first() {
    let core = Core::open(&fixture_artifact()).unwrap();
    assert_eq!(core.data_version(), Some("2026-10-25"));

    let page = core.explorar(0);
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
    let config = Config { today: "2026-10-25", recent_since: "2026-04-28", extra_ids: &[] };
    assert!(build(&flaky, &config).is_err());
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
