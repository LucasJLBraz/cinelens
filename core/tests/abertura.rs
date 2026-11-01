//! A launch loads the saved catálogo and model together, the same as one after the other.

use cinelens_core::{Core, ObraId};

const CATALOG: &[u8] = include_bytes!("../fixtures/catalog.json");
const MODEL: &[u8] = include_bytes!("../fixtures/model.json");

#[test]
fn loading_both_at_once_predicts_the_same_as_one_after_the_other() {
    let mut juntos = Core::default();
    juntos.load_saved(Some(CATALOG), None, None, Some(MODEL), None);
    let mut separados = Core::open(CATALOG).unwrap();
    separados.load_model(MODEL).unwrap();
    for core in [&mut juntos, &mut separados] {
        core.rate(ObraId::filme(603), 5.0, 100).unwrap();
    }
    let prevista = |core: &Core, id| core.movie(ObraId::filme(id)).unwrap().card.nota_prevista;
    assert!(prevista(&juntos, 157336).is_some());
    for id in [157336, 278, 11] {
        assert_eq!(prevista(&juntos, id), prevista(&separados, id));
    }
}

#[test]
fn an_invalid_model_keeps_the_catalogo() {
    let mut core = Core::default();
    core.load_saved(Some(CATALOG), None, None, Some(b"not a model"), None);
    assert!(core.data_version().is_some());
    assert_eq!(core.model_version(), None);
}

#[test]
fn nothing_saved_stays_a_first_run() {
    let mut core = Core::default();
    core.load_saved(None, None, None, None, None);
    assert_eq!(core.data_version(), None);
    assert_eq!(core.model_version(), None);
}
