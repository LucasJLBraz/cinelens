//! The séries job, run on a fake TMDB, produces an asset of its own that the core loads.

use cinelens_catalog::{build_series, Config};
use cinelens_core::{Core, ObraId};
use serde_json::{json, Value};

/// A série's details; `over` replaces fields.
fn serie(id: u32, over: Value) -> Value {
    let mut base = json!({
        "id": id, "name": format!("Série {id}"), "original_name": format!("Show {id}"),
        "first_air_date": "2019-04-01", "episode_run_time": [52], "genres": [{"id": 18, "name": "Drama"}],
        "original_language": "en", "origin_country": ["US"], "production_countries": [{"iso_3166_1": "US"}],
        "overview": "Sinopse", "poster_path": "/p.jpg", "backdrop_path": null,
        "vote_count": 1000 - id, "vote_average": 8.1, "popularity": 10.0,
        "created_by": [{"id": 70, "name": "Criadora"}], "number_of_seasons": 3,
        "status": "Returning Series", "type": "Scripted",
        "credits": {"cast": [{"id": 71, "name": "Atriz"}], "crew": [{"id": 72, "name": "Compositor", "job": "Original Music Composer"}]},
        "videos": {"results": [{"key": "yt", "site": "YouTube", "type": "Trailer", "iso_639_1": "en"}]},
        "watch/providers": {"results": {"BR": {"link": "l", "flatrate": [{"provider_id": 8, "provider_name": "Netflix", "logo_path": "/n.jpg"}]}}},
        "content_ratings": {"results": [{"iso_3166_1": "US", "rating": "TV-MA"}, {"iso_3166_1": "BR", "rating": "16"}]}
    });
    base.as_object_mut().unwrap().extend(over.as_object().unwrap().clone());
    base
}

/// Discover and trending list what they are given; `/tv/<id>` answers from `series`.
fn tmdb(series: Vec<Value>) -> impl Fn(&str) -> Result<Option<Value>, String> + Sync {
    move |request: &str| {
        assert!(!request.starts_with("/movie") && !request.contains("omdb"), "{request}");
        let ids: Vec<Value> = series.iter().map(|s| json!({"id": s["id"]})).collect();
        if request.starts_with("/trending/tv") {
            return Ok(Some(json!({"total_pages": 1, "results": ids})));
        }
        if request.starts_with("/discover/tv") {
            let first = request.contains("first_air_date.gte=2019-01-01");
            return Ok(Some(json!({"total_pages": 1, "results": if first { ids } else { Vec::new() }})));
        }
        let id: u64 = request.strip_prefix("/tv/").and_then(|r| r.split('?').next()?.parse().ok()).expect(request);
        Ok(series.iter().find(|s| s["id"] == id).cloned())
    }
}

fn config() -> Config<'static> {
    Config { today: "2026-10-25", recent_since: "2026-04-28", extra_ids: &[], omdb_budget: 0, anilist: Box::leak(Box::default()) }
}

fn built(series: Vec<Value>) -> Value {
    serde_json::to_value(build_series(&tmdb(series), &config()).unwrap()).unwrap()
}

fn ids(artifact: &Value) -> Vec<u32> {
    artifact["movies"].as_array().unwrap().iter().map(|m| serde_json::from_value::<ObraId>(m["tmdb_id"].clone()).unwrap().tmdb_id()).collect()
}

#[test]
fn scripted_miniseries_and_documentary_stay_and_the_rest_leave() {
    let keep = [(1, "Scripted"), (2, "Miniseries"), (3, "Documentary")];
    let mut series: Vec<Value> = keep.iter().map(|(id, kind)| serie(*id, json!({"type": kind}))).collect();
    series.extend([(4, "Reality"), (5, "Talk Show"), (6, "News"), (7, "Video")].map(|(id, kind)| serie(id, json!({"type": kind}))));
    // Daily telenovelas, reality or talk or news labelled as scripted.
    series.extend([(8, 10766), (9, 10764), (10, 10767), (11, 10763)].map(|(id, g)| serie(id, json!({"genres": [{"id": g, "name": "x"}]}))));
    let animation = json!([{"id": 16, "name": "Animação"}]);
    // Animation of Japanese origin, by country or language.
    series.push(serie(12, json!({"genres": animation, "origin_country": ["JP"]})));
    series.push(serie(13, json!({"genres": animation, "original_language": "ja", "origin_country": []})));
    // Animation from elsewhere, and a Japanese drama, stay.
    series.push(serie(14, json!({"genres": animation})));
    series.push(serie(15, json!({"origin_country": ["JP"], "original_language": "ja"})));
    // Without genres, nothing shows it is none of the above.
    series.push(serie(16, json!({"genres": []})));

    assert_eq!(ids(&built(series)), [1, 2, 3, 14, 15]);
}

#[test]
fn the_asset_carries_what_a_serie_needs_with_creators_as_direction() {
    let artifact = built(vec![serie(1, json!({}))]);
    let s = &artifact["movies"][0];
    assert_eq!(s["work_type"], "tv");
    assert_eq!(s["number_of_seasons"], 3);
    assert_eq!(s["situacao"], "em_andamento");
    assert_eq!(s["directors"], json!([{"id": 70, "name": "Criadora"}]));
    assert_eq!(s["runtime"], 52);
    assert_eq!(s["year"], 2019);
    assert_eq!(s["release_date"], "2019-04-01");
    assert_eq!(s["certification"], "16");
    assert_eq!(s["trailer_youtube"], "yt");
    assert_eq!(artifact["providers"][0]["name"], "Netflix");
}

#[test]
fn ended_and_canceled_series_are_terminada() {
    for status in ["Ended", "Canceled"] {
        let artifact = built(vec![serie(1, json!({"status": status}))]);
        assert_eq!(artifact["movies"][0]["situacao"], "terminada", "{status}");
    }
}

#[test]
fn episode_length_falls_back_to_the_last_episode() {
    let artifact = built(vec![serie(1, json!({"episode_run_time": [], "last_episode_to_air": {"runtime": 38}}))]);
    assert_eq!(artifact["movies"][0]["runtime"], 38);
}

#[test]
fn tv_genres_become_the_filme_genres_they_correspond_to() {
    let genres = json!([{"id": 10759, "name": ""}, {"id": 10765, "name": ""}, {"id": 10768, "name": ""}, {"id": 10762, "name": ""}, {"id": 18, "name": ""}]);
    let artifact = built(vec![serie(1, json!({"genres": genres}))]);
    assert_eq!(artifact["movies"][0]["genre_ids"], json!([28, 12, 878, 14, 10752, 10751, 18]));
}

#[test]
fn the_core_loads_the_asset_under_serie_ids() {
    let artifact = serde_json::to_vec(&build_series(&tmdb(vec![serie(1, json!({}))]), &config()).unwrap()).unwrap();
    let mut core = Core::default();
    core.load_series(&artifact).unwrap();
    assert_eq!(core.movie(ObraId::serie(1)).unwrap().card.original_title, "Show 1");
    assert!(core.movie(ObraId::filme(1)).is_none());
}

#[test]
fn a_second_build_on_the_same_day_can_carry_its_own_version() {
    let mut series = build_series(&tmdb(vec![serie(1, json!({}))]), &config()).unwrap();
    series.set_data_version("2026-10-25.42");
    let mut core = Core::default();
    core.load_series(&serde_json::to_vec(&series).unwrap()).unwrap();
    assert_eq!(core.series_version(), Some("2026-10-25.42"));
}

#[test]
fn a_filme_catalogo_reader_never_sees_the_asset_as_filmes() {
    let artifact = serde_json::to_vec(&build_series(&tmdb(vec![serie(1, json!({}))]), &config()).unwrap()).unwrap();
    assert!(Core::open(&artifact).is_err());
}

#[test]
fn a_serie_without_a_first_episode_date_still_builds() {
    let artifact = built(vec![serie(1, json!({"first_air_date": null}))]);
    assert_eq!(artifact["movies"][0]["release_date"], Value::Null);
}

#[test]
fn a_failure_fails_the_build() {
    let flaky = |request: &str| if request.starts_with("/tv/1") { Err("HTTP 500".to_string()) } else { tmdb(vec![serie(1, json!({}))])(request) };
    assert!(build_series(&flaky, &config()).is_err());
}

#[test]
fn the_vote_cut_rises_until_the_asset_fits() {
    let votes = [(1, 100), (2, 1_000), (3, 10_000)].map(|(id, votes)| serie(id, json!({"vote_count": votes})));
    let mut artifact = build_series(&tmdb(votes.to_vec()), &config()).unwrap();
    let count = |a: &cinelens_catalog::Artifact| a.obras();

    assert_eq!(artifact.raise_votes_to_fit(3, count), 50);
    assert_eq!(artifact.obras(), 3);

    let cut = artifact.raise_votes_to_fit(1, count);
    assert_eq!(artifact.obras(), 1);
    assert!(cut > 1_000 && cut <= 10_000, "{cut}");
}
