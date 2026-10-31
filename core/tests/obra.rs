//! A filme keeps its TMDB id as its id; a série with the same TMDB number stays a separate obra.

use cinelens_core::{Core, Marca, ObraId};

const CATALOG: &[u8] = include_bytes!("../fixtures/catalog.json");

#[test]
fn a_filmes_id_is_its_tmdb_id() {
    let filme = ObraId::filme(603);
    assert_eq!(filme.tmdb_id(), 603);
    assert_eq!(serde_json::to_string(&filme).unwrap(), "603");
    assert_eq!(serde_json::from_str::<ObraId>("603").unwrap(), filme);
}

#[test]
fn a_series_id_has_the_high_bit_and_gives_back_its_tmdb_id() {
    let serie = ObraId::serie(603);
    assert_eq!(serde_json::to_string(&serie).unwrap(), "2147484251");
    assert_eq!(serie.tmdb_id(), 603);
    assert_ne!(serie, ObraId::filme(603));
}

#[test]
fn a_nota_and_a_mark_on_a_serie_leave_the_filme_with_the_same_number_alone() {
    let mut core = Core::open(CATALOG).unwrap();
    let filme = ObraId::filme(603);
    core.rate(filme, 4.0, 100).unwrap();

    core.rate(ObraId::serie(603), 1.0, 200).unwrap();
    core.marcar(ObraId::serie(603), Marca::JaVi, true, 200);

    let page = core.movie(filme).unwrap();
    assert_eq!(page.history.last().unwrap().nota, 4.0);
    assert_eq!(page.history.len(), 1);
    assert!(!page.ja_vi);
}

#[test]
fn notas_saved_under_a_filmes_number_still_load() {
    let mut core = Core::open(CATALOG).unwrap();
    core.rate(ObraId::filme(603), 4.0, 100).unwrap();

    let saved = core.owner_data();
    let text = String::from_utf8_lossy(&saved);
    assert!(text.contains(r#""603":["#), "{text}");

    let mut reopened = Core::open(CATALOG).unwrap();
    reopened.load_owner_data(&saved).unwrap();
    assert_eq!(reopened.movie(ObraId::filme(603)).unwrap().history.len(), 1);
}
