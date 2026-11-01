//! The anime training job, run on small fixtures, publishes factors by temporada and nothing else.

use std::io::Cursor;

use cinelens_core::read_mapeamento;
use cinelens_train::train_animes;
use serde_json::{json, Value};

/// Entries 1–4 and 8 suit one half of the users, 5–7 the other. 9 is hentai.
fn ratings() -> String {
    let mut csv = String::from("user_id,anime_id,title,user_status,user_score,user_eps_watched,user_rewatch,updated_at\n");
    for user in 0..40u32 {
        for entry in 1..=9u32 {
            if entry % 8 == user % 8 {
                continue;
            }
            let first_half = entry <= 4 || entry == 8;
            let score = if first_half == (user % 2 == 0) { 9 } else { 3 };
            csv += &format!("secret_user{user},{entry},\"Title, {entry}\",completed,{score},12,False,2023-11-06 08:09:12+00:00\n");
        }
        // No score given: not a rating.
        csv += &format!("secret_user{user},1,\"Title, 1\",watching,0,1,False,2023-11-06 08:09:12+00:00\n");
    }
    csv += "few_user,10,Rare,completed,8,1,False,2023-11-06 08:09:12+00:00\n";
    csv
}

const TITLES: &str = "anime_id,title,mean,genres,studios,synopsis,media_type,num_episodes\r\n\
1,A,8.75,\"Action, Sci-Fi\",S,\"Two\n\nlines, here\",tv,12\r\n\
9,H,6.0,\"Fantasy, Hentai\",S,x,ova,2\r\n";

fn mapping() -> Vec<u8> {
    serde_json::to_vec(&json!([
        // Two parts of one temporada, then the next.
        {"mal_id": 1, "type": "TV", "themoviedb_id": {"tv": 100}, "season": {"tmdb": 1}},
        {"mal_id": 2, "type": "TV", "themoviedb_id": {"tv": 100}, "season": {"tmdb": 1}},
        {"mal_id": 3, "type": "TV", "themoviedb_id": {"tv": 100}, "season": {"tmdb": 2}},
        // One long run, not split into seasons.
        {"mal_id": 4, "type": "TV", "themoviedb_id": {"tv": 200}, "season": null},
        {"mal_id": 8, "type": "ONA", "themoviedb_id": {"tv": 500}, "season": {"tmdb": 1}},
        // An animated feature, the bridge from notas on filmes.
        {"mal_id": 5, "type": "MOVIE", "themoviedb_id": {"movie": [300]}},
        // Left out: a special, hentai, no TMDB id.
        {"mal_id": 6, "type": "SPECIAL", "themoviedb_id": {"tv": 100}, "season": {"tmdb": 0}},
        {"mal_id": 9, "type": "TV", "themoviedb_id": {"tv": 400}, "season": {"tmdb": 1}},
        {"mal_id": 7, "type": "TV"}
    ]))
    .unwrap()
}

fn model() -> (Value, Vec<u8>) {
    let mapping = read_mapeamento(&mapping()).unwrap();
    let content = vec![(100, vec!["g16".to_string()]), (200, vec!["g16".to_string()]), (500, vec!["g16".to_string()]), (300, vec!["g16".to_string()])];
    let model = train_animes(Cursor::new(ratings()), Cursor::new(TITLES), &mapping, &content, "2026-11-01").unwrap();
    let bytes = serde_json::to_vec(&model).unwrap();
    (serde_json::from_slice(&bytes).unwrap(), bytes)
}

#[test]
fn entries_join_by_temporada_and_a_long_run_counts_as_its_first() {
    let (m, _) = model();
    let keys: Vec<(u64, u64, u64)> = m["animes"].as_array().unwrap().iter().map(|a| (a["tmdb_id"].as_u64().unwrap(), a["temporada"].as_u64().unwrap(), a["ratings"].as_u64().unwrap())).collect();
    // Each entry has 35 ratings with a score: 40 users less the 5 who skip it.
    assert_eq!(keys, [(100, 1, 70), (100, 2, 35), (200, 1, 35), (500, 1, 35)]);
}

#[test]
fn the_parts_of_a_temporada_weigh_by_ratings_into_one_set_of_factors() {
    let (m, _) = model();
    assert_eq!(m["animes"][0]["factors"].as_array().unwrap().len(), 32);
}

#[test]
fn animated_features_are_published_by_their_filme_id() {
    let (m, _) = model();
    let ids: Vec<u64> = m["movies"].as_array().unwrap().iter().map(|a| a["tmdb_id"].as_u64().unwrap()).collect();
    assert_eq!(ids, [300]);
}

#[test]
fn hentai_never_reaches_the_artifact() {
    let (m, _) = model();
    assert!(!m["animes"].as_array().unwrap().iter().any(|a| a["tmdb_id"] == 400));
}

#[test]
fn no_rating_or_username_is_published() {
    let (m, bytes) = model();
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("secret_user") && !text.contains("few_user") && !text.contains("Title"));
    assert_eq!(m.as_object().unwrap().keys().map(String::as_str).collect::<Vec<_>>(), ["animes", "content", "data_version", "format_version", "global_mean", "lambda", "movies"]);
}

#[test]
fn scores_read_on_the_app_scale() {
    let (m, _) = model();
    let mean = m["global_mean"].as_f64().unwrap();
    assert!((1.5..=4.5).contains(&mean), "{mean}");
}

#[test]
fn content_is_learned_for_animes_without_factors() {
    let (m, _) = model();
    assert_eq!(m["content"][0]["feature"], "g16");
}
