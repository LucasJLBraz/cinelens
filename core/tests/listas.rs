//! Início: the destaque, and the listas prontas its rows and chips open in Explorar.

use cinelens_core::{Core, Marca};

/// Genres: 12 Aventura, 35 Comédia, 10751 Família, 18 Drama, 878 Ficção científica.
const CATALOG: &str = r#"{"format_version": 1, "data_version": "t", "trending": [8, 6],
    "movies": [
    {"tmdb_id": 1, "title": "Aventura Livre", "original_title": "A", "year": 1985, "release_date": "1985-06-01", "runtime": 110, "genre_ids": [12], "certification": "L", "vote_count": 20000, "vote_average": 7.0, "poster_path": null},
    {"tmdb_id": 2, "title": "Comédia Pesada", "original_title": "B", "year": 1995, "release_date": "1995-06-01", "runtime": 105, "genre_ids": [35], "certification": "16", "vote_count": 15000, "vote_average": 6.8, "poster_path": null, "watch": {"flatrate": [8]}},
    {"tmdb_id": 3, "title": "Família Nova", "original_title": "C", "year": 2015, "release_date": "2015-06-01", "runtime": 101, "genre_ids": [10751], "certification": "L", "vote_count": 12000, "vote_average": 7.1, "poster_path": null},
    {"tmdb_id": 4, "title": "Clássico Aclamado", "original_title": "D", "year": 1957, "release_date": "1957-04-10", "runtime": 120, "genre_ids": [18], "vote_count": 3000, "vote_average": 8.5, "poster_path": null},
    {"tmdb_id": 5, "title": "Clássico Morno", "original_title": "E", "year": 1975, "release_date": "1975-04-10", "runtime": 115, "genre_ids": [18], "vote_count": 2000, "vote_average": 6.0, "critic_score": 85, "poster_path": null},
    {"tmdb_id": 6, "title": "Joia", "original_title": "F", "year": 2020, "release_date": "2020-03-01", "runtime": 95, "genre_ids": [878], "vote_count": 300, "vote_average": 7.4, "critic_score": 60, "poster_path": null},
    {"tmdb_id": 7, "title": "Lançamento", "original_title": "G", "year": 2026, "release_date": "2026-08-01", "runtime": 150, "genre_ids": [878], "vote_count": 10, "vote_average": 8.0, "poster_path": null},
    {"tmdb_id": 8, "title": "Popular Fraco", "original_title": "H", "year": 2010, "release_date": "2010-06-01", "runtime": 90, "genre_ids": [18], "vote_count": 50000, "vote_average": 6.5, "critic_score": 80, "poster_path": null},
    {"tmdb_id": 9, "title": "Comédia Doze", "original_title": "I", "year": 2000, "release_date": "2000-06-01", "runtime": 104, "genre_ids": [35], "certification": "12", "vote_count": 900, "vote_average": 6.9, "poster_path": null, "watch": {"flatrate": [337], "rent": [8]}}
]}"#;

/// No notas: each nota prevista is 3.5 + bias, so Para você is 6, 7, 2, 1, 3, 4, 5, 9, 8; Boa
/// chance or better is the top three (6, 7, 2).
const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05, "content": [],
    "movies": [
        {"tmdb_id": 1, "bias": 0.4, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 2, "bias": 0.5, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 3, "bias": 0.3, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 4, "bias": 0.2, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 5, "bias": 0.1, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 6, "bias": 0.6, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 7, "bias": 0.55, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 8, "bias": -0.5, "factors": [0.0], "ratings": 50000},
        {"tmdb_id": 9, "bias": -0.2, "factors": [0.0], "ratings": 50000}
    ]}"#;

/// 2026-10-25, noon UTC.
const NOW: i64 = 1_792_929_600;
const DAY: i64 = 86_400;

fn core() -> Core {
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    core.load_model(MODEL.as_bytes()).unwrap();
    core
}

/// The movies of Início's row for the lista `nome`.
fn fileira(core: &Core, nome: &str) -> Vec<u32> {
    let inicio = core.inicio(NOW);
    let fileira = inicio.fileiras.iter().find(|f| f.nome == nome).unwrap_or_else(|| panic!("no row {nome}"));
    fileira.movies.iter().map(|m| m.tmdb_id).collect()
}

#[test]
fn every_row_is_explorar_with_its_listas_filtro() {
    let core = core();
    let inicio = core.inicio(NOW);
    let nomes: Vec<&str> = inicio.fileiras.iter().map(|f| f.nome.as_str()).collect();
    assert_eq!(
        nomes,
        [
            "Em alta, do seu jeito",
            "No seu streaming hoje",
            "Lançamentos para você",
            "Joias escondidas",
            "Sessão da Tarde",
            "Cabe numa noite",
            "Clássicos que você não viu",
            "Aclamados pela crítica",
            "Para rever",
        ]
    );
    for f in &inicio.fileiras {
        let explorar: Vec<u32> = core.explorar(&f.filtro, 0).movies.iter().map(|m| m.tmdb_id).collect();
        let row: Vec<u32> = f.movies.iter().map(|m| m.tmdb_id).collect();
        assert_eq!(row, explorar[..row.len()], "{}", f.nome);
    }
}

#[test]
fn em_alta_is_tmdb_trending_reordered_by_taste() {
    assert_eq!(fileira(&core(), "Em alta, do seu jeito"), [6, 8]);
}

#[test]
fn cabe_numa_noite_is_under_1h40() {
    assert_eq!(fileira(&core(), "Cabe numa noite"), [6, 8]);
}

#[test]
fn lancamentos_are_the_last_six_months_with_a_high_nota_prevista() {
    let core = core();
    assert_eq!(fileira(&core, "Lançamentos para você"), [7]);
    let inicio = core.inicio(NOW);
    let filtro = &inicio.fileiras.iter().find(|f| f.nome == "Lançamentos para você").unwrap().filtro;
    assert_eq!(filtro.lancado_desde.as_deref(), Some("2026-04-26"));
    // Out of the window once six months have passed.
    let later = core.inicio(NOW + 200 * DAY);
    assert!(later.fileiras.iter().find(|f| f.nome == "Lançamentos para você").unwrap().movies.is_empty());
}

#[test]
fn joias_escondidas_are_little_seen_with_a_high_nota_prevista() {
    assert_eq!(fileira(&core(), "Joias escondidas"), [6, 7]);
}

#[test]
fn sessao_da_tarde_is_family_fare_rated_up_to_12_from_1980_to_2010() {
    assert_eq!(fileira(&core(), "Sessão da Tarde"), [1, 9]);
}

#[test]
fn classicos_are_before_1980_well_rated_by_the_public_and_unseen() {
    let mut core = core();
    assert_eq!(fileira(&core, "Clássicos que você não viu"), [4]);
    core.marcar(4, Marca::JaVi, true, NOW);
    assert!(fileira(&core, "Clássicos que você não viu").is_empty());
}

#[test]
fn para_rever_lists_ja_vi_movies_with_a_high_nota_prevista() {
    let mut core = core();
    assert!(fileira(&core, "Para rever").is_empty());
    for id in [6, 2, 8] {
        core.marcar(id, Marca::JaVi, true, NOW);
    }
    assert_eq!(fileira(&core, "Para rever"), [6, 2]);
}

#[test]
fn the_destaque_swipes_through_the_top_recomendacoes_not_set_aside() {
    let mut core = core();
    let ids = |core: &Core| core.inicio(NOW).destaques.iter().map(|d| d.card.tmdb_id).collect::<Vec<_>>();
    assert_eq!(ids(&core), [6, 7, 2, 1, 3]);

    core.marcar(7, Marca::AgoraNao, true, NOW);
    core.marcar(1, Marca::JaVi, true, NOW);
    assert_eq!(ids(&core), [6, 2, 3, 4, 5]);
}

#[test]
fn rows_leave_out_movies_already_rated_or_seen() {
    let mut core = core();
    core.rate(6, 4.0, NOW).unwrap();
    core.marcar(8, Marca::JaVi, true, NOW);
    assert!(fileira(&core, "Em alta, do seu jeito").is_empty());
    assert!(fileira(&core, "Cabe numa noite").is_empty());
}

#[test]
fn no_seu_streaming_hoje_is_on_a_service_the_owner_subscribes_to() {
    let mut core = core();
    assert!(fileira(&core, "No seu streaming hoje").is_empty());
    // 9 is only for rent on Netflix (8): not something to watch right now.
    core.set_servicos(&[8], 0);
    assert_eq!(fileira(&core, "No seu streaming hoje"), [2]);
}

#[test]
fn aclamados_pela_critica_use_critic_scores_and_the_public_where_critics_are_missing() {
    // 5 and 8 by critics despite the public; 4 has no critic score but the public loves it; 6 is
    // panned by critics; 7 is loved by too few people to count.
    assert_eq!(fileira(&core(), "Aclamados pela crítica"), [4, 5, 8]);
}

#[test]
fn without_any_critic_scores_the_list_is_aclamados_pelo_publico() {
    let mut core = Core::open(CATALOG.replace("critic_score", "unknown").as_bytes()).unwrap();
    core.load_model(MODEL.as_bytes()).unwrap();
    let nomes: Vec<String> = core.inicio(NOW).fileiras.into_iter().map(|f| f.nome).collect();
    assert!(!nomes.iter().any(|n| n == "Aclamados pela crítica"));
    assert_eq!(fileira(&core, "Aclamados pelo público"), [4]);
}
