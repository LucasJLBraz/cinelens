//! Explorar: filters, search and Populares ↔ Garimpo, alone and combined.

use cinelens_core::{Core, Filtro, Marca, Mostrar};

/// Nolan (11) directed 2 and 6; 8 has no year, runtime nor nota prevista.
const CATALOG: &str = r#"{"format_version": 1, "data_version": "t",
    "genres": [{"id": 28, "name": "Ação"}, {"id": 878, "name": "Ficção científica"}, {"id": 18, "name": "Drama"}],
    "providers": [{"id": 8, "name": "Netflix"}, {"id": 337, "name": "Disney Plus"}],
    "movies": [
    {"tmdb_id": 1, "title": "Matrix", "original_title": "The Matrix", "year": 1999, "runtime": 136, "poster_path": null, "genre_ids": [878, 28], "original_language": "en", "countries": ["US"], "vote_count": 30000,
     "directors": [{"id": 10, "name": "Lana Wachowski"}], "cast": [{"id": 20, "name": "Keanu Reeves"}], "watch": {"flatrate": [8]}},
    {"tmdb_id": 2, "title": "A Origem", "original_title": "Inception", "year": 2010, "runtime": 148, "poster_path": null, "genre_ids": [878, 28, 12], "original_language": "en", "countries": ["US", "GB"], "vote_count": 35000,
     "directors": [{"id": 11, "name": "Christopher Nolan"}], "cast": [{"id": 21, "name": "Leonardo DiCaprio"}], "watch": {"flatrate": [337]}},
    {"tmdb_id": 3, "title": "Cidade de Deus", "original_title": "Cidade de Deus", "year": 2002, "runtime": 130, "poster_path": null, "genre_ids": [80, 18], "original_language": "pt", "countries": ["BR"], "vote_count": 7000,
     "directors": [{"id": 12, "name": "Fernando Meirelles"}], "cast": [{"id": 22, "name": "Alexandre Rodrigues"}]},
    {"tmdb_id": 4, "title": "Central do Brasil", "original_title": "Central do Brasil", "year": 1998, "runtime": 113, "poster_path": null, "genre_ids": [18], "original_language": "pt", "countries": ["BR"], "vote_count": 900,
     "directors": [{"id": 13, "name": "Walter Salles"}], "cast": [{"id": 23, "name": "Fernanda Montenegro"}], "watch": {"flatrate": [8], "rent": [337]}},
    {"tmdb_id": 5, "title": "O Fabuloso Destino de Amélie Poulain", "original_title": "Le Fabuleux Destin d'Amélie Poulain", "year": 2001, "runtime": 122, "poster_path": null, "genre_ids": [35, 10749], "original_language": "fr", "countries": ["FR"], "vote_count": 11000,
     "directors": [{"id": 14, "name": "Jean-Pierre Jeunet"}], "cast": [{"id": 24, "name": "Audrey Tautou"}]},
    {"tmdb_id": 6, "title": "Amnésia", "original_title": "Memento", "year": 2000, "runtime": 113, "poster_path": null, "genre_ids": [9648, 53], "original_language": "en", "countries": ["US"], "vote_count": 14000,
     "directors": [{"id": 11, "name": "Christopher Nolan"}], "cast": [{"id": 25, "name": "Guy Pearce"}]},
    {"tmdb_id": 7, "title": "Ação Total", "original_title": "Total Action", "year": 1985, "runtime": 95, "poster_path": null, "genre_ids": [28], "original_language": "en", "countries": ["US"], "vote_count": 50},
    {"tmdb_id": 8, "title": "Sem Dados", "original_title": "No Data", "poster_path": null}
]}"#;

/// No notas yet: each nota prevista is 3.5 + bias, so Para você is 4, 2, 3, 1, 6, 5, 7 (then 8).
/// Movie 4 has few ratings, so a low confiança.
const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05, "content": [],
    "movies": [
        {"tmdb_id": 1, "bias": 0.3, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 2, "bias": 0.5, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 3, "bias": 0.4, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 4, "bias": 0.6, "factors": [0.0], "ratings": 10},
        {"tmdb_id": 5, "bias": 0.1, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 6, "bias": 0.2, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 7, "bias": -0.5, "factors": [0.0], "ratings": 50000}
    ]}"#;

fn core() -> Core {
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    core.load_model(MODEL.as_bytes()).unwrap();
    core
}

fn ids(core: &Core, filtro: Filtro) -> Vec<u32> {
    core.explorar(&filtro, 0).movies.iter().map(|m| m.tmdb_id).collect()
}

#[test]
fn unfiltered_explorar_is_para_voce() {
    assert_eq!(ids(&core(), Filtro::default()), [4, 2, 3, 1, 6, 5, 7, 8]);
}

#[test]
fn included_genres_combine_with_and_by_default_or_with_the_switch() {
    let core = core();
    assert_eq!(ids(&core, Filtro { incluir: vec![878, 28], ..Default::default() }), [2, 1]);
    assert_eq!(ids(&core, Filtro { incluir: vec![878, 28], qualquer: true, ..Default::default() }), [2, 1, 7]);
}

#[test]
fn excluded_genres_hide_every_movie_with_them() {
    let core = core();
    assert_eq!(ids(&core, Filtro { excluir: vec![18, 35], ..Default::default() }), [2, 1, 6, 7, 8]);
    assert_eq!(ids(&core, Filtro { incluir: vec![28], excluir: vec![12], ..Default::default() }), [1, 7]);
}

#[test]
fn genres_are_the_catalogos_list_in_portuguese() {
    let generos = core().opcoes().generos;
    let names: Vec<&str> = generos.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Ação", "Ficção científica", "Drama"]);
}

#[test]
fn year_and_runtime_ranges_leave_out_movies_without_them() {
    let core = core();
    assert_eq!(ids(&core, Filtro { ano_min: Some(2000), ano_max: Some(2009), ..Default::default() }), [3, 6, 5]);
    assert_eq!(ids(&core, Filtro { duracao_max: Some(115), ..Default::default() }), [4, 6, 7]);
    assert_eq!(ids(&core, Filtro { duracao_min: Some(130), ..Default::default() }), [2, 3, 1]);
}

#[test]
fn original_language_and_country() {
    let core = core();
    assert_eq!(ids(&core, Filtro { idioma: Some("pt".into()), ..Default::default() }), [4, 3]);
    assert_eq!(ids(&core, Filtro { pais: Some("GB".into()), ..Default::default() }), [2]);
    let opcoes = core.opcoes();
    assert_eq!(opcoes.idiomas, ["en", "pt", "fr"], "most movies first");
    assert_eq!(opcoes.paises, ["US", "BR", "FR", "GB"]);
}

#[test]
fn minimum_nota_prevista_and_confianca() {
    let core = core();
    assert_eq!(ids(&core, Filtro { nota_prevista_min: Some(3.85), ..Default::default() }), [4, 2, 3]);
    assert_eq!(ids(&core, Filtro { confianca_min: Some(0.5), ..Default::default() }), [2, 3, 1, 6, 5, 7]);
}

#[test]
fn rated_ja_vi_and_quero_assistir_can_be_hidden_or_shown_alone() {
    let mut core = core();
    core.rate(2, 4.0, 100).unwrap();
    core.marcar(3, Marca::JaVi, true, 100);
    core.marcar(5, Marca::QueroAssistir, true, 100);

    assert_eq!(ids(&core, Filtro { com_nota: Mostrar::So, ..Default::default() }), [2]);
    assert!(!ids(&core, Filtro { com_nota: Mostrar::Esconder, ..Default::default() }).contains(&2));
    assert_eq!(ids(&core, Filtro { ja_vi: Mostrar::So, ..Default::default() }), [3]);
    assert!(!ids(&core, Filtro { ja_vi: Mostrar::Esconder, ..Default::default() }).contains(&3));
    assert_eq!(ids(&core, Filtro { quero_assistir: Mostrar::So, ..Default::default() }), [5]);
    assert!(!ids(&core, Filtro { quero_assistir: Mostrar::Esconder, ..Default::default() }).contains(&5));
}

#[test]
fn nao_me_interessa_movies_show_only_when_asked_for_so_they_can_be_restored() {
    let mut core = core();
    core.marcar(6, Marca::NaoMeInteressa, true, 100);

    assert!(!ids(&core, Filtro::default()).contains(&6));
    assert_eq!(ids(&core, Filtro { nao_me_interessa: true, ..Default::default() }), [6]);
}

#[test]
fn onde_assistir_keeps_movies_on_the_owners_services() {
    let mut core = core();
    let filtro = || Filtro { no_meu_streaming: true, ..Default::default() };
    assert_eq!(ids(&core, filtro()), [] as [u32; 0], "no services chosen yet");

    core.set_servicos(&[8]);
    assert_eq!(ids(&core, filtro()), [4, 1]);
    // Renting is not subscription.
    core.set_servicos(&[337]);
    assert_eq!(ids(&core, filtro()), [2]);
}

#[test]
fn search_matches_titles_director_and_actor_ignoring_case_and_accents() {
    let core = core();
    let search = |q: &str| ids(&core, Filtro { busca: q.into(), ..Default::default() });
    assert_eq!(search("amelie"), [5], "pt-BR title without its accent");
    assert_eq!(search("Fabuleux"), [5], "original title");
    assert_eq!(search("MATRIX"), [1]);
    assert_eq!(search("nolan"), [2, 6], "director");
    assert_eq!(search("keanu"), [1], "actor");
    assert_eq!(search("nolan memento"), [6], "every word");
    assert_eq!(search("  "), [4, 2, 3, 1, 6, 5, 7, 8]);
}

#[test]
fn filters_combine() {
    let core = core();
    let filtro = Filtro { incluir: vec![878], ano_min: Some(2005), busca: "origem".into(), ..Default::default() };
    assert_eq!(ids(&core, filtro), [2]);
    let filtro = Filtro { idioma: Some("en".into()), duracao_max: Some(140), excluir: vec![28], ..Default::default() };
    assert_eq!(ids(&core, filtro), [6]);
}

#[test]
fn populares_raises_well_known_movies_and_garimpo_obscure_ones() {
    let core = core();
    let position = |garimpo: f32, id: u32| ids(&core, Filtro { garimpo, ..Default::default() }).iter().position(|&m| m == id).unwrap();

    // Para você: 4 (900 votes) above 1 (30000 votes), 2 (35000 votes) above 3 (7000 votes).
    assert!(position(0.0, 4) < position(0.0, 1));
    assert!(position(-1.0, 1) < position(-1.0, 4), "Populares");
    assert!(position(1.0, 3) < position(1.0, 2), "Garimpo");
    // Movies without a nota prevista stay last either way.
    assert_eq!(position(1.0, 8), 7);
}

#[test]
fn results_come_in_short_pages() {
    let core = Core::open(include_bytes!("../fixtures/catalog.json")).unwrap();
    let first = core.explorar(&Filtro::default(), 0);
    assert_eq!(first.movies.len(), 30);
    assert!(first.has_more);
    let filtered = core.explorar(&Filtro { busca: "guerra nas estrelas".into(), ..Default::default() }, 0);
    assert_eq!(filtered.movies.len(), 2);
    assert!(!filtered.has_more);
}
