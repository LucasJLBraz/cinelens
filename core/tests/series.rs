//! Séries come from their own asset and live beside filmes in the same catálogo.

use cinelens_core::{Core, Filtro, ObraId, Tipo};

/// A filme and a série share TMDB number 7.
const FILMES: &str = r#"{"format_version": 1, "data_version": "f1", "genres": [{"id": 878, "name": "Ficção científica"}], "trending": [], "providers": [{"id": 8, "name": "Netflix", "logo_path": null}], "movies": [
    {"tmdb_id": 7, "title": "O Filme", "original_title": "The Movie", "year": 2000, "poster_path": null, "vote_count": 900, "genre_ids": [878], "watch": {"link": null, "flatrate": [8]}},
    {"tmdb_id": 8, "title": "Outro Filme", "original_title": "Other Movie", "year": 2001, "poster_path": null, "vote_count": 100, "genre_ids": [878]}
]}"#;

const SERIES: &str = r#"{"format_version": 1, "data_version": "s1", "genres": [], "trending": [2147483655], "providers": [{"id": 9, "name": "Max", "logo_path": null}], "movies": [
    {"tmdb_id": 2147483655, "work_type": "tv", "title": "A Série", "original_title": "The Show", "year": 2010, "poster_path": null, "vote_count": 500, "runtime": 45, "genre_ids": [878], "watch": {"link": null, "flatrate": [9]}}
]}"#;

fn core() -> Core {
    let mut core = Core::open(FILMES.as_bytes()).unwrap();
    core.load_series(SERIES.as_bytes()).unwrap();
    core
}

fn ids(core: &Core, filtro: &Filtro) -> Vec<ObraId> {
    core.explorar(filtro, 0).movies.iter().map(|m| m.id).collect()
}

const FILME: ObraId = ObraId::filme(7);
const SERIE: ObraId = ObraId::serie(7);

#[test]
fn a_filme_and_a_serie_with_the_same_number_are_two_obras() {
    let core = core();
    assert_eq!(core.movie(FILME).unwrap().card.title, "O Filme");
    assert_eq!(core.movie(SERIE).unwrap().card.title, "A Série");
}

#[test]
fn tudo_shows_both_types_most_voted_first() {
    assert_eq!(ids(&core(), &Filtro::default()), [FILME, SERIE, ObraId::filme(8)]);
}

#[test]
fn the_tipo_filter_narrows_to_one_type() {
    let core = core();
    let tipo = |t| Filtro { tipo: Some(t), ..Default::default() };
    assert_eq!(ids(&core, &tipo(Tipo::Serie)), [SERIE]);
    assert_eq!(ids(&core, &tipo(Tipo::Filme)), [FILME, ObraId::filme(8)]);
}

#[test]
fn search_finds_a_serie_by_portuguese_and_original_title() {
    let core = core();
    for busca in ["a serie", "the show"] {
        let filtro = Filtro { busca: busca.into(), ..Default::default() };
        assert_eq!(ids(&core, &filtro), [SERIE], "{busca}");
    }
}

#[test]
fn the_duration_filter_reads_episode_length_on_a_serie() {
    let filtro = Filtro { tipo: Some(Tipo::Serie), duracao_max: Some(60), ..Default::default() };
    assert_eq!(ids(&core(), &filtro), [SERIE]);
}

#[test]
fn a_new_catalogo_of_either_type_leaves_the_other_in_place() {
    let mut core = core();
    core.load_catalog(FILMES.as_bytes()).unwrap();
    assert!(core.movie(SERIE).is_some());
    core.load_series(SERIES.as_bytes()).unwrap();
    assert!(core.movie(FILME).is_some());
    assert_eq!(ids(&core, &Filtro::default()).len(), 3);
    assert_eq!(core.data_version(), Some("f1"));
    assert_eq!(core.series_version(), Some("s1"));
}

#[test]
fn services_and_trending_come_from_both_assets() {
    let core = core();
    let nomes: Vec<&str> = core.provedores().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(nomes, ["Max", "Netflix"]);
    let filtro = Filtro { em_alta: true, ..Default::default() };
    assert_eq!(ids(&core, &filtro), [SERIE]);
}

#[test]
fn an_asset_with_the_wrong_type_of_obra_is_refused() {
    let mut core = core();
    assert!(core.load_series(FILMES.as_bytes()).is_err());
    assert!(core.load_catalog(SERIES.as_bytes()).is_err());
    assert_eq!(ids(&core, &Filtro::default()).len(), 3);
}

#[test]
fn a_launch_loads_the_saved_series_too() {
    let mut core = Core::default();
    core.load_saved(Some(FILMES.as_bytes()), Some(SERIES.as_bytes()), None);
    assert_eq!(ids(&core, &Filtro::default()).len(), 3);
}

#[test]
fn invalid_saved_series_leave_the_filmes() {
    let mut core = Core::default();
    core.load_saved(Some(FILMES.as_bytes()), Some(b"nope"), None);
    assert_eq!(ids(&core, &Filtro::default()).len(), 2);
    assert_eq!(core.series_version(), None);
}
