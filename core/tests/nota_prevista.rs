use cinelens_core::{Core, Filtro};

const CATALOG: &[u8] = include_bytes!("../fixtures/catalog.json");
/// Two tastes: sci-fi (603 Matrix, 27205 Inception, 157336 Interstellar, 693134 Duna 2, 11 Star
/// Wars) and crime/drama (238 and 240 O Poderoso Chefão, 278 Um Sonho de Liberdade, 769 Os Bons
/// Companheiros, 424 A Lista de Schindler). Other catálogo movies are not in the model.
const MODEL: &[u8] = include_bytes!("../fixtures/model.json");

fn with_model() -> Core {
    let mut core = Core::open(CATALOG).unwrap();
    core.load_model(MODEL).unwrap();
    core
}

fn prevista(core: &Core, tmdb_id: u32) -> f32 {
    core.movie(tmdb_id).unwrap().card.nota_prevista.unwrap()
}

#[test]
fn rating_movies_shifts_predictions_for_similar_movies() {
    let mut core = with_model();
    let (inception, godfather) = (prevista(&core, 27205), prevista(&core, 238));

    core.rate(603, 5.0, 100).unwrap();
    core.rate(157336, 5.0, 100).unwrap();
    core.rate(278, 2.0, 100).unwrap();
    core.rate(769, 2.0, 100).unwrap();

    assert!(prevista(&core, 27205) > inception + 0.5, "Inception up");
    assert!(prevista(&core, 238) < godfather - 0.5, "O Poderoso Chefão down");
    assert!(prevista(&core, 693134) > prevista(&core, 240));
}

#[test]
fn rated_movies_and_movies_outside_the_model_have_no_nota_prevista() {
    let mut core = with_model();
    core.rate(603, 5.0, 100).unwrap();

    assert_eq!(core.movie(603).unwrap().card.nota_prevista, None);
    // Clube da Luta is in the catálogo but not in the model.
    assert_eq!(core.movie(550).unwrap().card.nota_prevista, None);
}

#[test]
fn nota_prevista_stays_on_the_nota_scale() {
    let mut core = with_model();
    for id in [603, 27205, 157336, 11] {
        core.rate(id, 5.0, 100).unwrap();
    }
    for id in [238, 240, 278, 769] {
        core.rate(id, 0.5, 100).unwrap();
    }

    let p = prevista(&core, 693134);
    assert!((0.5..=5.0).contains(&p), "{p}");
}

#[test]
fn explorar_puts_highest_nota_prevista_first() {
    let mut core = with_model();
    core.rate(603, 5.0, 100).unwrap();
    core.rate(278, 1.5, 100).unwrap();

    let ids: Vec<u32> = core.explorar(&Filtro::default(), 0).movies.iter().map(|m| m.tmdb_id).collect();
    let previstas: Vec<f32> =
        core.explorar(&Filtro::default(), 0).movies.iter().map_while(|m| m.nota_prevista).collect();

    assert_eq!(previstas.len(), 8, "every unrated movie in the model comes first");
    assert!(previstas.windows(2).all(|w| w[0] >= w[1]));
    assert!([27205, 157336, 11].contains(&ids[0]));
}

#[test]
fn notas_imported_or_loaded_from_disk_also_drive_predictions() {
    let mut core = with_model();
    core.rate(603, 5.0, 100).unwrap();
    core.rate(278, 1.5, 100).unwrap();
    let expected = prevista(&core, 27205);

    let mut restarted = Core::open(CATALOG).unwrap();
    restarted.load_owner_data(&core.owner_data()).unwrap();
    restarted.load_model(MODEL).unwrap();
    assert_eq!(prevista(&restarted, 27205), expected);

    let mut model_first = with_model();
    model_first.load_owner_data(&core.owner_data()).unwrap();
    assert_eq!(prevista(&model_first, 27205), expected);
}

#[test]
fn without_a_model_explorar_keeps_catalogo_order() {
    let core = Core::open(CATALOG).unwrap();
    let page = core.explorar(&Filtro::default(), 0);
    assert_eq!(page.movies[0].tmdb_id, 550);
    assert!(page.movies.iter().all(|m| m.nota_prevista.is_none()));
    assert_eq!(core.model_version(), None);
}

#[test]
fn loading_model_with_unknown_format_version_keeps_the_current_one() {
    let mut core = with_model();
    let newer = br#"{"format_version": 3, "data_version": "x", "global_mean": 3, "lambda": 1, "movies": [], "content": []}"#;

    assert!(core.load_model(newer).is_err());
    assert!(core.load_model(b"not json").is_err());
    assert_eq!(core.model_version(), Some("fixture-2026-10-25"));
    assert!(core.movie(27205).unwrap().card.nota_prevista.is_some());
}
