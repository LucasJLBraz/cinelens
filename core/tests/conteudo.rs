//! Movies without collaborative data get a nota prevista from their content, and every nota
//! prevista carries a confiança.

use cinelens_core::{Core, Filtro};

/// 1–3 sci-fi by director 900, 4–6 drama by director 901, all in the model. 7 is a new sci-fi
/// release by 900 and 8 a new drama by 901, neither in the model; 9 shares nothing with it.
const CATALOG: &str = r#"{"format_version": 1, "data_version": "t", "movies": [
    {"tmdb_id": 1, "title": "A", "original_title": "A", "year": 1999, "poster_path": null, "genre_ids": [878], "directors": [{"id": 900, "name": "X"}], "cast": [], "original_language": "en"},
    {"tmdb_id": 2, "title": "B", "original_title": "B", "year": 2001, "poster_path": null, "genre_ids": [878], "directors": [{"id": 900, "name": "X"}], "cast": [], "original_language": "en"},
    {"tmdb_id": 3, "title": "C", "original_title": "C", "year": 2003, "poster_path": null, "genre_ids": [878], "directors": [{"id": 900, "name": "X"}], "cast": [], "original_language": "en"},
    {"tmdb_id": 4, "title": "D", "original_title": "D", "year": 1999, "poster_path": null, "genre_ids": [18], "directors": [{"id": 901, "name": "Y"}], "cast": [], "original_language": "en"},
    {"tmdb_id": 5, "title": "E", "original_title": "E", "year": 2001, "poster_path": null, "genre_ids": [18], "directors": [{"id": 901, "name": "Y"}], "cast": [], "original_language": "en"},
    {"tmdb_id": 6, "title": "F", "original_title": "F", "year": 2003, "poster_path": null, "genre_ids": [18], "directors": [{"id": 901, "name": "Y"}], "cast": [], "original_language": "en"},
    {"tmdb_id": 7, "title": "G", "original_title": "G", "year": 2026, "poster_path": null, "genre_ids": [878], "directors": [{"id": 900, "name": "X"}], "cast": [], "original_language": "en"},
    {"tmdb_id": 8, "title": "H", "original_title": "H", "year": 2026, "poster_path": null, "genre_ids": [18], "directors": [{"id": 901, "name": "Y"}], "cast": [], "original_language": "en"},
    {"tmdb_id": 9, "title": "I", "original_title": "I", "year": 2026, "poster_path": null, "genre_ids": [99], "directors": [{"id": 902, "name": "Z"}], "cast": [], "original_language": "xx"}
]}"#;

/// Movie 1 is a classic with many ratings; the content mapping covers the genres and
/// directors of 1–6.
const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05,
    "movies": [
        {"tmdb_id": 1, "bias": 0.3, "factors": [1.0, 0.0], "ratings": 50000},
        {"tmdb_id": 2, "bias": 0.2, "factors": [0.9, 0.1], "ratings": 800},
        {"tmdb_id": 3, "bias": 0.2, "factors": [1.0, -0.1], "ratings": 300},
        {"tmdb_id": 4, "bias": 0.3, "factors": [0.0, 1.0], "ratings": 40000},
        {"tmdb_id": 5, "bias": 0.2, "factors": [0.1, 0.9], "ratings": 500},
        {"tmdb_id": 6, "bias": 0.2, "factors": [-0.1, 1.0], "ratings": 200}
    ],
    "content": [
        {"feature": "g878", "bias": 0.2, "factors": [0.9, 0.0]},
        {"feature": "d900", "bias": 0.2, "factors": [1.0, 0.0]},
        {"feature": "g18", "bias": 0.2, "factors": [0.0, 0.9]},
        {"feature": "d901", "bias": 0.2, "factors": [0.0, 1.0]}
    ]
}"#;

fn sci_fi_lover() -> Core {
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    core.load_model(MODEL.as_bytes()).unwrap();
    core.rate(2, 5.0, 100).unwrap();
    core.rate(5, 1.0, 100).unwrap();
    core
}

#[test]
fn a_movie_absent_from_the_training_data_still_gets_a_nota_prevista() {
    let core = sci_fi_lover();
    let (new_sci_fi, new_drama) = (core.movie(7).unwrap().card, core.movie(8).unwrap().card);

    assert!(new_sci_fi.nota_prevista.unwrap() > new_drama.nota_prevista.unwrap() + 1.0);
}

#[test]
fn content_only_predictions_have_lower_confianca_than_well_rated_classics() {
    let core = sci_fi_lover();
    let classic = core.movie(1).unwrap().card.confianca.unwrap();
    let new_release = core.movie(7).unwrap().card.confianca.unwrap();

    assert!(new_release < classic, "{new_release} vs {classic}");
    assert!(classic > 0.9 && classic <= 1.0, "{classic}");
}

#[test]
fn every_nota_prevista_has_a_confianca() {
    let core = sci_fi_lover();
    for card in core.explorar(&Filtro::default(), 0).movies {
        assert_eq!(
            card.nota_prevista.is_some(),
            card.confianca.is_some(),
            "{}",
            card.tmdb_id
        );
    }
}

#[test]
fn a_movie_with_unknown_content_has_no_nota_prevista() {
    let core = sci_fi_lover();
    let card = core.movie(9).unwrap().card;
    assert_eq!((card.nota_prevista, card.confianca), (None, None));
}

#[test]
fn held_out_notas_outside_the_model_are_measured_apart_from_its_content() {
    let mut core = sci_fi_lover();
    core.rate(1, 5.0, 100).unwrap();
    core.rate(4, 1.0, 100).unwrap();
    // The most recent notas: a sci-fi in the model and the new sci-fi release outside it.
    core.rate(3, 5.0, 200).unwrap();
    core.rate(7, 5.0, 200).unwrap();

    let accuracy = core.accuracy(2.0 / 6.0).unwrap();
    assert_eq!((accuracy.held_out, accuracy.notas, accuracy.content_notas), (2, 1, 1));
    // Predicting the owner's mean (3.67) would miss the release by 1.33.
    assert!(accuracy.content_mae.unwrap() < 1.0, "{:?}", accuracy.content_mae);
}
