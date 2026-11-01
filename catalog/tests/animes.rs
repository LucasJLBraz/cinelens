//! The animes job, run on a fake TMDB, recognizes animes and publishes them in their own asset.

use std::collections::BTreeMap;

use cinelens_catalog::{build_animes, build_series, AniList, Config};
use cinelens_core::{Core, ObraId, Tipo};
use serde_json::{json, Value};

const EDGERUNNERS: u32 = 105248;
const LINK_CLICK: u32 = 123542;
const CASTLEVANIA: u32 = 71024;
const BLUE_EYE_SAMURAI: u32 = 225180;
const DIABOLICAL: u32 = 152483;
const SCOOBY: u32 = 926;
const BEN_10: u32 = 4686;
const ARCANE: u32 = 94605;
const INVINCIBLE: u32 = 95557;

fn animation() -> Value {
    json!([{"id": 16, "name": "Animação"}])
}

/// Animation from the USA unless `over` says otherwise; nothing in it says anime.
fn obra(id: u32, over: Value) -> Value {
    let mut base = json!({
        "id": id, "name": format!("Obra {id}"), "original_name": format!("Original {id}"),
        "first_air_date": "2019-04-01", "episode_run_time": [24], "genres": animation(),
        "original_language": "en", "origin_country": ["US"], "production_countries": [],
        "overview": "", "poster_path": null, "backdrop_path": null,
        "vote_count": 1000, "vote_average": 8.0, "popularity": 1.0,
        "created_by": [], "number_of_seasons": 1, "status": "Ended", "type": "Scripted",
        "credits": {"cast": [], "crew": []}, "videos": {"results": []}
    });
    base.as_object_mut().unwrap().extend(over.as_object().unwrap().clone());
    base
}

/// Discover and trending list `listed`; `/tv/<id>` answers from `all`.
fn tmdb(all: Vec<Value>, listed: usize) -> impl Fn(&str) -> Result<Option<Value>, String> + Sync {
    move |request: &str| {
        let ids: Vec<Value> = all.iter().take(listed).map(|s| json!({"id": s["id"]})).collect();
        if request.starts_with("/trending/tv") {
            return Ok(Some(json!({"total_pages": 1, "results": []})));
        }
        if request.starts_with("/discover/tv") {
            assert!(request.contains("with_genres=16") || request.contains("with_type"), "{request}");
            let first = request.contains("first_air_date.gte=2019-01-01");
            return Ok(Some(json!({"total_pages": 1, "results": if first { ids } else { Vec::new() }})));
        }
        let id: u64 = request.strip_prefix("/tv/").and_then(|r| r.split('?').next()?.parse().ok()).expect(request);
        Ok(all.iter().find(|s| s["id"] == id).cloned())
    }
}

fn config(anilist: &AniList) -> Config<'_> {
    Config { today: "2026-10-25", recent_since: "2026-04-28", extra_ids: &[], omdb_budget: 0, anilist }
}

fn built(all: Vec<Value>, anilist: &AniList) -> Value {
    let listed = all.len();
    serde_json::to_value(build_animes(&tmdb(all, listed), &config(anilist)).unwrap()).unwrap()
}

fn ids(artifact: &Value) -> Vec<u32> {
    let mut ids: Vec<u32> = artifact["movies"].as_array().unwrap().iter().map(|m| serde_json::from_value::<ObraId>(m["tmdb_id"].clone()).unwrap().tmdb_id()).collect();
    ids.sort();
    ids
}

/// AniList knows these, by TMDB id, with their AniList popularity.
fn anilist(known: &[(u32, u32)]) -> AniList {
    let mapping: Vec<Value> = known.iter().enumerate().map(|(i, (id, _))| json!({"anilist_id": i, "mal_id": i, "type": "TV", "themoviedb_id": {"tv": id}, "season": {"tmdb": 1}})).collect();
    let popularidade: BTreeMap<u32, u32> = known.iter().enumerate().map(|(i, (_, p))| (i as u32, *p)).collect();
    AniList::new(&serde_json::to_vec(&mapping).unwrap(), &popularidade).unwrap()
}

#[test]
fn each_rule_recognizes_an_anime_and_western_animation_stays_out() {
    let all = vec![
        // Japanese origin, by country or by language.
        obra(EDGERUNNERS, json!({"origin_country": ["JP"]})),
        obra(2, json!({"original_language": "ja", "origin_country": []})),
        // In AniList, from China.
        obra(LINK_CLICK, json!({"original_language": "zh", "origin_country": ["CN"]})),
        // TMDB's keyword, whatever its case.
        obra(3, json!({"keywords": {"results": [{"id": 210024, "name": "Anime"}]}})),
        // The manual list.
        obra(CASTLEVANIA, json!({})),
        obra(BLUE_EYE_SAMURAI, json!({})),
        obra(DIABOLICAL, json!({})),
        // Western animation.
        obra(SCOOBY, json!({})),
        obra(BEN_10, json!({})),
        obra(ARCANE, json!({"keywords": {"results": [{"id": 1, "name": "video game"}]}})),
        obra(INVINCIBLE, json!({})),
    ];
    let artifact = built(all, &anilist(&[(LINK_CLICK, 100)]));
    let mut expected = vec![EDGERUNNERS, 2, LINK_CLICK, 3, CASTLEVANIA, BLUE_EYE_SAMURAI, DIABOLICAL];
    expected.sort();
    assert_eq!(ids(&artifact), expected);
    assert_eq!(artifact["movies"][0]["work_type"], "anime");
}

#[test]
fn what_tmdb_does_not_list_as_animation_is_no_anime() {
    let drama = json!([{"id": 18, "name": "Drama"}]);
    let all = vec![obra(CASTLEVANIA, json!({"genres": drama})), obra(1, json!({"genres": drama, "origin_country": ["JP"]}))];
    assert!(ids(&built(all, &anilist(&[(1, 100)]))).is_empty());
}

#[test]
fn hentai_is_left_out_of_animes_and_of_series() {
    let hentai = vec![
        obra(1, json!({"origin_country": ["JP"], "adult": true})),
        obra(2, json!({"origin_country": ["JP"], "keywords": {"results": [{"id": 9, "name": "hentai"}]}})),
    ];
    assert!(ids(&built(hentai.clone(), &anilist(&[]))).is_empty());
    let empty = anilist(&[]);
    let series = serde_json::to_value(build_series(&tmdb(hentai, 2), &config(&empty)).unwrap()).unwrap();
    assert!(ids(&series).is_empty());
}

#[test]
fn an_anime_never_also_goes_out_as_a_serie() {
    let all = vec![obra(CASTLEVANIA, json!({})), obra(LINK_CLICK, json!({})), obra(3, json!({"keywords": {"results": [{"id": 1, "name": "anime"}]}})), obra(ARCANE, json!({}))];
    let known = anilist(&[(LINK_CLICK, 100)]);
    let series = serde_json::to_value(build_series(&tmdb(all, 4), &config(&known)).unwrap()).unwrap();
    assert_eq!(ids(&series), [ARCANE]);
}

#[test]
fn high_anilist_popularity_brings_in_an_anime_discover_does_not_list() {
    // The first is listed; the others have too few votes for discover.
    let all = vec![
        obra(1, json!({"origin_country": ["JP"]})),
        obra(2, json!({"origin_country": ["JP"], "vote_count": 3})),
        obra(3, json!({"origin_country": ["JP"], "vote_count": 3})),
    ];
    let known = anilist(&[(2, 50_000), (3, 40)]);
    let artifact = serde_json::to_value(build_animes(&tmdb(all, 1), &config(&known)).unwrap()).unwrap();
    assert_eq!(ids(&artifact), [1, 2]);
}

#[test]
fn the_vote_cut_spares_high_anilist_popularity_until_it_passes_that_too() {
    let all = vec![
        obra(1, json!({"origin_country": ["JP"], "vote_count": 60})),
        obra(2, json!({"origin_country": ["JP"], "vote_count": 3})),
    ];
    let known = anilist(&[(2, 50_000)]);
    let mut artifact = build_animes(&tmdb(all, 2), &config(&known)).unwrap();
    artifact.raise_votes_to_fit(1, |a| a.obras());
    assert_eq!(ids(&serde_json::to_value(&artifact).unwrap()), [2]);
    artifact.raise_votes_to_fit(0, |a| a.obras());
    assert_eq!(artifact.obras(), 0);
}

fn asset(all: Vec<Value>, known: &AniList) -> Vec<u8> {
    let listed = all.len();
    serde_json::to_vec(&build_animes(&tmdb(all, listed), &config(known)).unwrap()).unwrap()
}

#[test]
fn the_core_loads_the_asset_as_animes() {
    let mut core = Core::default();
    core.load_animes(&asset(vec![obra(CASTLEVANIA, json!({}))], &anilist(&[]))).unwrap();
    assert_eq!(core.tipo(ObraId::anime(CASTLEVANIA)), Tipo::Anime);
    assert_eq!(core.animes_version(), Some("2026-10-25"));
}

#[test]
fn installed_apps_never_read_the_asset_as_filmes_or_series() {
    let bytes = asset(vec![obra(CASTLEVANIA, json!({}))], &anilist(&[]));
    assert!(Core::open(&bytes).is_err());
    assert!(Core::default().load_series(&bytes).is_err());
}

#[test]
fn a_serie_that_becomes_an_anime_keeps_its_id_and_nota() {
    let empty = anilist(&[]);
    let as_serie = serde_json::to_vec(&build_series(&tmdb(vec![obra(ARCANE, json!({}))], 1), &config(&empty)).unwrap()).unwrap();
    let mut core = Core::default();
    core.load_series(&as_serie).unwrap();
    let id = ObraId::serie(ARCANE);
    core.rate(id, 4.5, 100).unwrap();
    assert_eq!(core.tipo(id), Tipo::Serie);

    // A next build recognizes it. Until the next séries asset arrives, both list it.
    core.load_animes(&asset(vec![obra(ARCANE, json!({"keywords": {"results": [{"id": 1, "name": "anime"}]}}))], &empty)).unwrap();
    assert_eq!(ObraId::anime(ARCANE), id);
    assert_eq!(core.tipo(id), Tipo::Anime);
    assert_eq!(core.movie(id).unwrap().nota, Some(4.5));
    assert_eq!(core.explorar(&Default::default(), 0).movies.len(), 1);
}

#[test]
fn temporadas_come_as_tmdb_lists_them_without_specials_and_say_which_have_an_entry() {
    let seasons = json!([
        {"season_number": 0, "name": "Especiais", "episode_count": 3, "air_date": "2019-01-01"},
        {"season_number": 1, "name": "Temporada 1", "episode_count": 12, "air_date": "2019-04-01"},
        {"season_number": 2, "name": "Temporada 2", "episode_count": 13, "air_date": null},
        {"season_number": 3, "name": "Temporada 3", "episode_count": 13, "air_date": "2024-04-01"}
    ]);
    let mapping = json!([
        {"anilist_id": 1, "mal_id": 1, "type": "TV", "themoviedb_id": {"tv": 7}, "season": {"tmdb": 1}},
        {"anilist_id": 2, "mal_id": 2, "type": "TV", "themoviedb_id": {"tv": 7}, "season": {"tmdb": 2}},
        {"anilist_id": 3, "mal_id": 3, "type": "SPECIAL", "themoviedb_id": {"tv": 7}, "season": {"tmdb": 3}}
    ]);
    let known = AniList::new(&serde_json::to_vec(&mapping).unwrap(), &BTreeMap::new()).unwrap();
    let artifact = built(vec![obra(7, json!({"origin_country": ["JP"], "seasons": seasons}))], &known);
    assert_eq!(
        artifact["movies"][0]["temporadas"],
        json!([
            {"numero": 1, "nome": "Temporada 1", "episodios": 12, "ano": 2019, "estreia": "2019-04-01", "entrada": true},
            {"numero": 2, "nome": "Temporada 2", "episodios": 13, "ano": null, "estreia": null, "entrada": true},
            {"numero": 3, "nome": "Temporada 3", "episodios": 13, "ano": 2024, "estreia": "2024-04-01", "entrada": false}
        ])
    );
}

#[test]
fn a_movie_in_the_mapping_is_not_taken_for_the_tv_obra_with_its_number() {
    let mapping = json!([{"anilist_id": 1, "mal_id": 1, "type": "MOVIE", "themoviedb_id": {"movie": [ARCANE]}}]);
    let known = AniList::new(&serde_json::to_vec(&mapping).unwrap(), &BTreeMap::new()).unwrap();
    assert!(ids(&built(vec![obra(ARCANE, json!({}))], &known)).is_empty());
}

#[test]
fn the_manual_list_survives_any_realistic_vote_cut() {
    let all = vec![obra(CASTLEVANIA, json!({"vote_count": 60})), obra(EDGERUNNERS, json!({"origin_country": ["JP"], "vote_count": 60}))];
    let none = anilist(&[]);
    let mut artifact = build_animes(&tmdb(all, 2), &config(&none)).unwrap();
    artifact.raise_votes_to_fit(1, |a| a.obras());
    assert_eq!(ids(&serde_json::to_value(&artifact).unwrap()), [CASTLEVANIA]);
}

#[test]
fn entries_count_as_the_trainer_counts_them() {
    // No AniList id, and no season: temporada 1, as in the trainer. It recognizes nothing.
    let mapping = json!([
        {"mal_id": 1, "type": "TV", "themoviedb_id": {"tv": 7}, "season": null},
        {"mal_id": 2, "type": "TV", "themoviedb_id": {"tv": 7}, "season": {"tmdb": 2}}
    ]);
    let known = AniList::new(&serde_json::to_vec(&mapping).unwrap(), &BTreeMap::new()).unwrap();
    assert!(ids(&built(vec![obra(7, json!({}))], &known)).is_empty());
    let seasons = json!([{"season_number": 1, "name": "T1"}, {"season_number": 2, "name": "T2"}]);
    let artifact = built(vec![obra(7, json!({"origin_country": ["JP"], "seasons": seasons}))], &known);
    let entradas: Vec<&Value> = artifact["movies"][0]["temporadas"].as_array().unwrap().iter().map(|t| &t["entrada"]).collect();
    assert_eq!(entradas, [true, true]);
}
