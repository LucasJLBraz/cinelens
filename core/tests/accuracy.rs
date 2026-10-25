//! Accuracy harness: MAE of the nota prevista on the owner's most recent notas, hidden from the
//! fold-in. CI runs it on fixtures only; `cinelens-accuracy` runs it on the owner's real export.

use cinelens_core::Core;

const CATALOG: &[u8] = include_bytes!("../fixtures/catalog.json");
/// Sci-fi: 603, 27205, 157336, 693134, 11. Crime/drama: 238, 240, 278, 769, 424.
const MODEL: &[u8] = include_bytes!("../fixtures/model.json");

fn owner_with_recent_notas() -> Core {
    let mut core = Core::open(CATALOG).unwrap();
    core.load_model(MODEL).unwrap();
    core.rate(603, 5.0, 100).unwrap();
    core.rate(157336, 5.0, 100).unwrap();
    core.rate(278, 1.5, 100).unwrap();
    core.rate(769, 1.5, 100).unwrap();
    // The most recent notas, the ones held out.
    core.rate(27205, 5.0, 200).unwrap();
    core.rate(238, 1.0, 200).unwrap();
    core
}

#[test]
fn held_out_most_recent_notas_are_predicted_from_the_older_ones() {
    // A third of the six notas: the two given last.
    let accuracy = owner_with_recent_notas().accuracy(1.0 / 3.0).unwrap();

    assert_eq!((accuracy.held_out, accuracy.notas), (2, 2));
    // Predicting the owner's mean (3.25) would miss by 2.0 on average.
    assert!(accuracy.mae < 1.0, "{}", accuracy.mae);
    assert_eq!(accuracy.movielens_mae, None, "notas given in the app have no MovieLens prediction");
}

#[test]
fn movielens_prediction_on_the_same_notas_is_the_baseline() {
    let mut core = owner_with_recent_notas();
    // An owner data blob as saved by the app: the two recent notas came from the importação,
    // with what MovieLens predicted for them (off by 1.0 each).
    let blob = br#"{"notas": {
        "603": [{"nota": 5.0, "at": 100, "movielens_pred": null}],
        "157336": [{"nota": 5.0, "at": 100, "movielens_pred": null}],
        "278": [{"nota": 1.5, "at": 100, "movielens_pred": null}],
        "769": [{"nota": 1.5, "at": 100, "movielens_pred": null}],
        "27205": [{"nota": 5.0, "at": 200, "movielens_pred": 4.0}],
        "238": [{"nota": 1.0, "at": 200, "movielens_pred": 2.0}]
    }, "quero_assistir": []}"#;
    core.load_owner_data(blob).unwrap();

    let accuracy = core.accuracy(1.0 / 3.0).unwrap();
    assert_eq!(accuracy.movielens_mae, Some(1.0));
    assert!(accuracy.mae < 1.0);
}

#[test]
fn notas_outside_the_model_are_left_out_of_the_measure() {
    let mut core = owner_with_recent_notas();
    // Clube da Luta is not in the model: it is the most recent nota but cannot be predicted.
    core.rate(550, 4.0, 300).unwrap();

    let accuracy = core.accuracy(3.0 / 7.0).unwrap();
    assert_eq!((accuracy.held_out, accuracy.notas), (3, 2));
}

#[test]
fn no_measure_without_a_model_or_notas() {
    let mut core = Core::open(CATALOG).unwrap();
    core.rate(603, 5.0, 100).unwrap();
    assert!(core.accuracy(1.0).is_none(), "no model");

    let mut core = Core::open(CATALOG).unwrap();
    core.load_model(MODEL).unwrap();
    assert!(core.accuracy(1.0).is_none(), "no notas");
}

#[test]
fn measuring_leaves_the_predictions_unchanged() {
    let core = owner_with_recent_notas();
    let before = core.movie(693134).unwrap().card.nota_prevista;
    core.accuracy(1.0 / 3.0);
    assert_eq!(core.movie(693134).unwrap().card.nota_prevista, before);
}
