//! Rodada rápida: one movie at a time to rate or skip as "não vi": those the owner likely saw
//! and that teach the most first.

use cinelens_core::ObraId;
use cinelens_core::{Core, Marca, Tipo};

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
    core.rodada(Tipo::Filme, 10).iter().map(|m| m.id.tmdb_id()).collect()
}

#[test]
fn divisive_well_known_movies_come_first() {
    // 4 says little, but 5, as divisive as 1, is one few have seen: it comes last. 6 teaches the
    // fold-in nothing.
    assert_eq!(rodada(&core()), [1, 3, 2, 4, 5]);
}

#[test]
fn a_nota_moves_the_rodada_to_what_the_model_still_does_not_know() {
    let mut core = core();
    core.rate(ObraId::filme(1), 4.5, NOW).unwrap();
    // The first direction is known now, so 3 tells less than 2.
    assert_eq!(rodada(&core)[0], 2);
    // And the nota is like any other.
    assert_eq!(core.movie(ObraId::filme(1)).unwrap().nota, Some(4.5));
    assert!(!rodada(&core).contains(&1));
}

#[test]
fn nao_vi_skips_a_movie_for_good() {
    let mut core = core();
    core.nao_vi(ObraId::filme(1));
    assert_eq!(rodada(&core)[0], 3);

    let mut reopened = Core::open(CATALOG.as_bytes()).unwrap();
    reopened.load_model(MODEL.as_bytes()).unwrap();
    reopened.load_owner_data(&core.owner_data()).unwrap();
    assert!(!rodada(&reopened).contains(&1));
}

#[test]
fn movies_already_seen_or_dismissed_are_left_out() {
    let mut core = core();
    core.marcar(ObraId::filme(1), Marca::JaVi, true, NOW);
    core.marcar(ObraId::filme(3), Marca::NaoMeInteressa, true, NOW);
    assert_eq!(rodada(&core), [2, 4, 5]);
}

#[test]
fn the_next_movies_come_ready_to_show() {
    let core = core();
    let next = core.rodada(Tipo::Filme, 2);
    assert_eq!(next.len(), 2);
    assert_eq!(next[0].title, "Um");
    assert_eq!(next[0].poster_url.as_deref(), Some("https://image.tmdb.org/t/p/w342/1.jpg"));
}

mod series {
    use cinelens_core::{Core, Marca, ObraId, Reacao, Tipo};

    /// A filme to give the fold-in something, and séries through content: S1 and S2 along the
    /// first taste direction, S3 along the second, S4 as divisive as S3 but little seen, S5
    /// outside what the model knows.
    const FILMES: &str = r#"{"format_version": 1, "data_version": "f", "movies": [
        {"tmdb_id": 1, "title": "F", "original_title": "F", "year": 2000, "poster_path": null, "genre_ids": [18], "vote_count": 99000}
    ]}"#;

    const SERIES: &str = r#"{"format_version": 1, "data_version": "s", "movies": [
        {"tmdb_id": 2147483649, "title": "S1", "original_title": "S1", "year": 2020, "poster_path": null, "genre_ids": [18], "vote_count": 5000},
        {"tmdb_id": 2147483650, "title": "S2", "original_title": "S2", "year": 2020, "poster_path": null, "genre_ids": [18], "vote_count": 4000},
        {"tmdb_id": 2147483651, "title": "S3", "original_title": "S3", "year": 2020, "poster_path": null, "genre_ids": [35], "vote_count": 3000},
        {"tmdb_id": 2147483652, "title": "S4", "original_title": "S4", "year": 2020, "poster_path": null, "genre_ids": [35], "vote_count": 20},
        {"tmdb_id": 2147483653, "title": "S5", "original_title": "S5", "year": 2020, "poster_path": null, "genre_ids": [99], "vote_count": 9000}
    ]}"#;

    const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05,
        "movies": [{"tmdb_id": 1, "bias": 0.0, "factors": [1.0, 0.0], "ratings": 50000}],
        "content": [
            {"feature": "g18", "bias": 0.0, "factors": [1.0, 0.0]},
            {"feature": "g35", "bias": 0.0, "factors": [0.0, 1.0]}
        ]}"#;

    fn core() -> Core {
        let mut core = Core::default();
        core.load_saved(Some(FILMES.as_bytes()), Some(SERIES.as_bytes()), None, Some(MODEL.as_bytes()));
        core
    }

    fn rodada(core: &Core) -> Vec<u32> {
        core.rodada(Tipo::Serie, 10).iter().map(|m| m.id.tmdb_id()).collect()
    }

    #[test]
    fn well_known_series_the_model_can_read_come_first() {
        assert_eq!(rodada(&core()), [1, 2, 3, 4]);
    }

    #[test]
    fn it_is_scored_against_the_series_profile() {
        let mut core = core();
        // The first direction is known now, so S3 tells more than S2, though less voted.
        core.reagir(ObraId::serie(1), Reacao::Amei, 100).unwrap();
        assert_eq!(rodada(&core), [3, 2, 4]);
    }

    #[test]
    fn a_nota_on_a_filme_also_moves_it() {
        let mut core = core();
        core.rate(ObraId::filme(1), 5.0, 100).unwrap();
        assert_eq!(rodada(&core)[0], 3);
    }

    #[test]
    fn nao_vi_and_marks_leave_it() {
        let mut core = core();
        core.nao_vi(ObraId::serie(1));
        core.marcar(ObraId::serie(2), Marca::JaVi, true, 100);
        core.marcar(ObraId::serie(3), Marca::Assistindo, true, 100);
        assert_eq!(rodada(&core), [4]);
    }

    #[test]
    fn the_rodada_of_filmes_asks_no_serie() {
        let core = core();
        let filmes: Vec<u32> = core.rodada(Tipo::Filme, 10).iter().map(|m| m.id.tmdb_id()).collect();
        assert_eq!(filmes, [1]);
    }
}
