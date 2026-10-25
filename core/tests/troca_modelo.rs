//! A new monthly model replaces the current one only if it predicts the owner's recent notas
//! no worse, and within the 0.65 bar.

use cinelens_core::Core;
use serde_json::{json, Value};

const CATALOG: &[u8] = include_bytes!("../fixtures/catalog.json");
/// Sci-fi: 603, 27205, 157336, 693134, 11. Crime/drama: 238, 240, 278, 769, 424.
const MODEL: &[u8] = include_bytes!("../fixtures/model.json");

/// Loves sci-fi, dislikes drama; the most recent notas are the ones measured.
fn owner() -> Core {
    let mut core = Core::open(CATALOG).unwrap();
    core.load_model(MODEL).unwrap();
    let notas = [(603, 5.0, 100), (157336, 5.0, 100), (11, 5.0, 100), (693134, 5.0, 100), (278, 1.0, 100), (769, 1.0, 100), (424, 1.0, 100), (240, 1.0, 100), (27205, 5.0, 200), (238, 1.0, 200)];
    for (id, nota, at) in notas {
        core.rate(id, nota, at).unwrap();
    }
    core
}

/// The fixture model, published again as `version`, with `edit` applied to every movie.
fn model(version: &str, edit: impl Fn(&mut Value)) -> Vec<u8> {
    let mut model: Value = serde_json::from_slice(MODEL).unwrap();
    model["data_version"] = json!(version);
    model["movies"].as_array_mut().unwrap().iter_mut().for_each(edit);
    serde_json::to_vec(&model).unwrap()
}

#[test]
fn a_model_as_good_as_the_current_one_replaces_it() {
    let mut core = owner();
    let rejeicao = core.trocar_modelo(&model("2026-11-03", |_| {})).unwrap();
    assert!(rejeicao.is_none(), "{rejeicao:?}");
    assert_eq!(core.model_version(), Some("2026-11-03"));
}

#[test]
fn a_model_that_predicts_the_owner_worse_is_kept_out_with_the_reason() {
    let mut core = owner();
    let before = core.accuracy(0.2).unwrap().mae;
    // No factors: every movie looks alike, the owner's taste is lost.
    let pior = model("2026-11-03", |m| m["factors"] = json!([0.0, 0.0]));
    let rejeicao = core.trocar_modelo(&pior).unwrap().expect("rejected");
    assert_eq!(rejeicao.data_version, "2026-11-03");
    assert!(rejeicao.mae.unwrap() > 0.65, "{:?}", rejeicao.mae);
    assert!(rejeicao.motivo.contains("0,65"), "{}", rejeicao.motivo);
    assert_eq!(core.model_version(), Some("fixture-2026-10-25"));
    assert_eq!(core.accuracy(0.2).unwrap().mae, before, "still the current model's predictions");
}

#[test]
fn without_notas_to_measure_any_valid_model_replaces_the_current_one() {
    let mut core = Core::open(CATALOG).unwrap();
    core.load_model(MODEL).unwrap();
    assert!(core.trocar_modelo(&model("2026-11-03", |_| {})).unwrap().is_none());
    assert_eq!(core.model_version(), Some("2026-11-03"));
    assert!(core.trocar_modelo(b"not a model").is_err());
    assert_eq!(core.model_version(), Some("2026-11-03"));
}
