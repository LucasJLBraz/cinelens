//! Daily catálogo deltas: the catalog job diffs yesterday's catálogo against today's, the app
//! applies the delta to the one it has.

use cinelens_core::{apply_catalog_delta, catalog_delta, Core};
use serde_json::{json, Value};

fn ontem() -> Value {
    json!({"format_version": 1, "data_version": "2026-10-24", "trending": [1],
        "movies": [
            {"tmdb_id": 1, "title": "Um", "original_title": "One", "year": 2001, "vote_count": 10},
            {"tmdb_id": 2, "title": "Dois", "original_title": "Two", "year": 2002},
            {"tmdb_id": 3, "title": "Três", "original_title": "Three", "year": 2003}
        ]})
}

/// One movie changed, one gone, one new; trending moved on.
fn hoje() -> Value {
    json!({"format_version": 1, "data_version": "2026-10-25", "trending": [4],
        "movies": [
            {"tmdb_id": 1, "title": "Um", "original_title": "One", "year": 2001, "vote_count": 11},
            {"tmdb_id": 3, "title": "Três", "original_title": "Three", "year": 2003},
            {"tmdb_id": 4, "title": "Quatro", "original_title": "Four", "year": 2026}
        ]})
}

fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}

#[test]
fn applying_the_delta_to_yesterdays_catalogo_gives_todays() {
    let delta = catalog_delta(&bytes(&ontem()), &bytes(&hoje())).unwrap();
    let updated = apply_catalog_delta(&bytes(&ontem()), &delta).unwrap();
    let updated: Value = serde_json::from_slice(&updated).unwrap();
    assert_eq!(updated, hoje());
    let core = Core::open(&bytes(&updated)).unwrap();
    assert_eq!(core.data_version(), Some("2026-10-25"));
}

#[test]
fn the_delta_carries_only_what_changed() {
    let delta: Value = serde_json::from_slice(&catalog_delta(&bytes(&ontem()), &bytes(&hoje())).unwrap()).unwrap();
    let ids: Vec<u64> = delta["movies"].as_array().unwrap().iter().map(|m| m["tmdb_id"].as_u64().unwrap()).collect();
    assert_eq!(ids, [1, 4]);
    assert_eq!(delta["removed"], json!([2]));
}

#[test]
fn a_delta_from_another_version_is_refused() {
    let delta = catalog_delta(&bytes(&ontem()), &bytes(&hoje())).unwrap();
    let mut outro = ontem();
    outro["data_version"] = json!("2026-10-20");
    assert!(apply_catalog_delta(&bytes(&outro), &delta).is_err());
}
