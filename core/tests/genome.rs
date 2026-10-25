//! The tag genome subset in the model: "parecidos, mas…" and o que pesou.

use cinelens_core::{Aspecto, Core, Direcao, Filtro};

/// 1–5 are sci-fi, most alike to 1 in id order; 6–8 are dramas.
const CATALOG: &str = r#"{"format_version": 1, "data_version": "t", "movies": [
    {"tmdb_id": 1, "title": "Base", "original_title": "Base", "year": 2000, "runtime": 130, "poster_path": null},
    {"tmdb_id": 2, "title": "Dois", "original_title": "Two", "year": 1990, "runtime": 150, "poster_path": null},
    {"tmdb_id": 3, "title": "Três", "original_title": "Three", "year": 2010, "runtime": 100, "poster_path": null},
    {"tmdb_id": 4, "title": "Quatro", "original_title": "Four", "year": 2020, "runtime": 120, "poster_path": null},
    {"tmdb_id": 5, "title": "Cinco", "original_title": "Five", "year": 1985, "runtime": 90, "poster_path": null},
    {"tmdb_id": 6, "title": "Seis", "original_title": "Six", "year": 2015, "runtime": 140, "poster_path": null},
    {"tmdb_id": 7, "title": "Sete", "original_title": "Seven", "year": 1970, "poster_path": null},
    {"tmdb_id": 8, "title": "Oito", "original_title": "Eight", "year": 1960, "poster_path": null}
]}"#;

/// Genome scores are leve, fotografia, atuações, roteiro, trilha, ritmo, diálogos; 4 and 8 have none.
const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05, "content": [],
    "movies": [
        {"tmdb_id": 1, "bias": 0.0, "factors": [1.0, 0.0], "ratings": 10000},
        {"tmdb_id": 2, "bias": 0.0, "factors": [0.95, 0.05], "ratings": 10000},
        {"tmdb_id": 3, "bias": 0.0, "factors": [0.9, 0.1], "ratings": 10000},
        {"tmdb_id": 4, "bias": 0.0, "factors": [0.85, 0.15], "ratings": 10000},
        {"tmdb_id": 5, "bias": 0.0, "factors": [0.8, 0.2], "ratings": 10000},
        {"tmdb_id": 6, "bias": 0.0, "factors": [0.0, 1.0], "ratings": 10000},
        {"tmdb_id": 7, "bias": 0.0, "factors": [0.1, 0.9], "ratings": 10000},
        {"tmdb_id": 8, "bias": 0.0, "factors": [0.15, 0.85], "ratings": 10000}
    ],
    "genome": {"aspects": ["leve", "fotografia", "atuacoes", "roteiro", "trilha", "ritmo", "dialogos"], "movies": [
        {"tmdb_id": 1, "scores": [0.2, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]},
        {"tmdb_id": 2, "scores": [0.1, 0.9, 0.2, 0.5, 0.5, 0.5, 0.5]},
        {"tmdb_id": 3, "scores": [0.8, 0.3, 0.9, 0.5, 0.5, 0.5, 0.5]},
        {"tmdb_id": 5, "scores": [0.6, 0.7, 0.5, 0.5, 0.5, 0.5, 0.5]},
        {"tmdb_id": 6, "scores": [0.5, 0.95, 0.5, 0.5, 0.5, 0.5, 0.5]},
        {"tmdb_id": 7, "scores": [0.0, 0.0, 1.0, 0.5, 0.5, 0.5, 0.5]}
    ]}}"#;

fn core() -> Core {
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    core.load_model(MODEL.as_bytes()).unwrap();
    core
}

fn ids(core: &Core, tmdb_id: u32, direcao: Direcao) -> Vec<u32> {
    core.parecidos_mas(tmdb_id, direcao).iter().map(|m| m.tmdb_id).collect()
}

#[test]
fn each_chip_keeps_the_parecidos_that_go_its_way_most_alike_first() {
    let core = core();
    assert_eq!(ids(&core, 1, Direcao::MaisRecente), [3, 4, 6]);
    assert_eq!(ids(&core, 1, Direcao::MaisCurto), [3, 4, 5]);
    assert_eq!(ids(&core, 1, Direcao::MelhorFotografia), [2, 5, 6]);
    assert_eq!(ids(&core, 1, Direcao::MaisLeve), [3, 5, 6]);
}

#[test]
fn a_movie_without_genome_data_compares_against_its_parecidos() {
    let core = core();
    // Quatro has no fotografia score: what beats its parecidos' median (0.7) is better.
    assert_eq!(ids(&core, 4, Direcao::MelhorFotografia), [2, 6]);
    assert!(core.parecidos_mas(999, Direcao::MaisLeve).is_empty());
}

fn para_voce(core: &Core) -> Vec<u32> {
    core.explorar(&Filtro::default(), 0).movies.iter().map(|m| m.tmdb_id).collect()
}

fn before(ranked: &[u32], a: u32, b: u32) -> bool {
    ranked.iter().position(|&m| m == a) < ranked.iter().position(|&m| m == b)
}

#[test]
fn o_que_pesou_pulls_recomendacoes_toward_movies_strong_in_it() {
    let mut core = core();
    core.rate(1, 5.0, 100).unwrap();
    let prevista = core.movie(3).unwrap().card.nota_prevista;
    assert!(before(&para_voce(&core), 2, 3));

    core.set_pesou(1, &[Aspecto::Atuacoes]);

    assert!(before(&para_voce(&core), 3, 2), "Três is strong in atuações");
    assert_eq!(core.movie(3).unwrap().card.nota_prevista, prevista, "the nota prevista itself stays");
    assert_eq!(core.movie(1).unwrap().pesou, [Aspecto::Atuacoes]);
    // Quatro has no genome data: still ranked, just not pulled either way.
    assert!(para_voce(&core).contains(&4));

    core.set_pesou(1, &[]);
    assert!(before(&para_voce(&core), 2, 3));
}

#[test]
fn o_que_pesou_never_lifts_a_movie_the_owner_would_probably_not_like() {
    let mut core = core();
    core.rate(1, 5.0, 100).unwrap();
    core.rate(6, 1.0, 100).unwrap();
    assert!(before(&para_voce(&core), 8, 7));

    core.set_pesou(1, &[Aspecto::Atuacoes]);

    // Sete is the strongest in atuações, but a drama, which this owner rates low.
    assert!(before(&para_voce(&core), 8, 7));
}

#[test]
fn o_que_pesou_and_hiding_its_prompt_survive_a_restart() {
    let mut core = core();
    assert!(!core.pesou_oculto());
    core.rate(1, 5.0, 100).unwrap();
    core.set_pesou(1, &[Aspecto::Atuacoes, Aspecto::Fotografia]);
    core.set_pesou_oculto(true);

    let mut restarted = Core::open(CATALOG.as_bytes()).unwrap();
    restarted.load_model(MODEL.as_bytes()).unwrap();
    restarted.load_owner_data(&core.owner_data()).unwrap();
    assert_eq!(restarted.movie(1).unwrap().pesou, [Aspecto::Fotografia, Aspecto::Atuacoes]);
    assert!(restarted.pesou_oculto());
    assert_eq!(para_voce(&restarted), para_voce(&core));
}

#[test]
fn a_model_without_genome_still_loads_and_ranks() {
    let model = MODEL.split(r#""genome""#).next().unwrap().trim_end().trim_end_matches(',').to_string() + "}";
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    core.load_model(model.as_bytes()).unwrap();
    core.rate(1, 5.0, 100).unwrap();
    let ranked = para_voce(&core);
    core.set_pesou(1, &[Aspecto::Atuacoes]);
    assert_eq!(para_voce(&core), ranked);
    assert!(core.parecidos_mas(1, Direcao::MelhorFotografia).is_empty());
    assert_eq!(ids(&core, 1, Direcao::MaisRecente), [3, 4, 6]);
}
