//! Rodada rápida: one movie at a time to rate or skip as "não vi", the most informative first.

use cinelens_core::{Core, Marca};

/// All equally well known but 5 (little seen) and 6 (not in the model: content only).
const CATALOG: &str = r#"{"format_version": 1, "data_version": "t", "movies": [
    {"tmdb_id": 1, "title": "Um", "original_title": "1", "year": 2000, "genre_ids": [18], "vote_count": 10000, "poster_path": "/1.jpg"},
    {"tmdb_id": 2, "title": "Dois", "original_title": "2", "year": 2000, "genre_ids": [18], "vote_count": 10000, "poster_path": null},
    {"tmdb_id": 3, "title": "Três", "original_title": "3", "year": 2000, "genre_ids": [18], "vote_count": 10000, "poster_path": null},
    {"tmdb_id": 4, "title": "Quatro", "original_title": "4", "year": 2000, "genre_ids": [18], "vote_count": 10000, "poster_path": null},
    {"tmdb_id": 5, "title": "Cinco", "original_title": "5", "year": 2000, "genre_ids": [18], "vote_count": 20, "poster_path": null},
    {"tmdb_id": 6, "title": "Seis", "original_title": "6", "year": 2026, "genre_ids": [18], "vote_count": 10000, "poster_path": null}
]}"#;

/// Two taste directions. 1 and 3 split people along the first, 2 along the second; 4 splits
/// hardly anyone (the model is sure what the owner will think of it); 5 is as divisive as 1
/// but little seen.
const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05,
    "content": [{"feature": "g18", "bias": 0.0, "factors": [3.0, 3.0]}],
    "movies": [
        {"tmdb_id": 1, "bias": 0.0, "factors": [1.0, 0.0], "ratings": 50000},
        {"tmdb_id": 2, "bias": 0.0, "factors": [0.0, 0.9], "ratings": 50000},
        {"tmdb_id": 3, "bias": 0.0, "factors": [0.95, 0.0], "ratings": 50000},
        {"tmdb_id": 4, "bias": 0.0, "factors": [0.1, 0.1], "ratings": 50000},
        {"tmdb_id": 5, "bias": 0.0, "factors": [1.0, 0.0], "ratings": 50000}
    ]}"#;

const NOW: i64 = 1_792_929_600;

fn core() -> Core {
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    core.load_model(MODEL.as_bytes()).unwrap();
    core
}

fn rodada(core: &Core) -> Vec<u32> {
    core.rodada(10).iter().map(|m| m.tmdb_id).collect()
}

#[test]
fn divisive_well_known_movies_come_first() {
    // 5 is as divisive as 1 but few have seen it; 4 says little; 6 teaches the fold-in nothing.
    assert_eq!(rodada(&core()), [1, 3, 2, 5, 4]);
}

#[test]
fn a_nota_moves_the_rodada_to_what_the_model_still_does_not_know() {
    let mut core = core();
    core.rate(1, 4.5, NOW).unwrap();
    // The first direction is known now, so 3 tells less than 2.
    assert_eq!(rodada(&core)[0], 2);
    // And the nota is like any other.
    assert_eq!(core.movie(1).unwrap().nota, Some(4.5));
    assert!(!rodada(&core).contains(&1));
}

#[test]
fn nao_vi_skips_a_movie_for_good() {
    let mut core = core();
    core.nao_vi(1);
    assert_eq!(rodada(&core)[0], 3);

    let mut reopened = Core::open(CATALOG.as_bytes()).unwrap();
    reopened.load_model(MODEL.as_bytes()).unwrap();
    reopened.load_owner_data(&core.owner_data()).unwrap();
    assert!(!rodada(&reopened).contains(&1));
}

#[test]
fn movies_already_seen_or_dismissed_are_left_out() {
    let mut core = core();
    core.marcar(1, Marca::JaVi, true, NOW);
    core.marcar(3, Marca::NaoMeInteressa, true, NOW);
    assert_eq!(rodada(&core), [2, 5, 4]);
}

#[test]
fn the_next_movies_come_ready_to_show() {
    let core = core();
    let next = core.rodada(2);
    assert_eq!(next.len(), 2);
    assert_eq!(next[0].title, "Um");
    assert_eq!(next[0].poster_url.as_deref(), Some("https://image.tmdb.org/t/p/w342/1.jpg"));
}
