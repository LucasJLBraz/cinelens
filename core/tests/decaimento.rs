//! Time decay: older notas weigh less in the fold-in, at a speed calibrated on the owner's notas.

use cinelens_core::{Core, Decay};

const CATALOG: &[u8] = include_bytes!("../fixtures/catalog.json");
/// Sci-fi: 603, 27205, 157336, 693134, 11. Crime/drama: 238, 240, 278, 769, 424.
const MODEL: &[u8] = include_bytes!("../fixtures/model.json");
const DAY: i64 = 86_400;

fn with_model() -> Core {
    let mut core = Core::open(CATALOG).unwrap();
    core.load_model(MODEL).unwrap();
    core
}

fn prevista(core: &Core, tmdb_id: u32) -> f32 {
    core.movie(tmdb_id).unwrap().card.nota_prevista.unwrap()
}

/// Loved sci-fi and disliked drama five years ago; the other way round in the last weeks.
fn changed_taste(core: &mut Core) {
    for id in [603, 157336] {
        core.rate(id, 5.0, 0).unwrap();
    }
    for id in [278, 769] {
        core.rate(id, 1.0, 0).unwrap();
    }
    let now = 5 * 365 * DAY;
    core.rate(27205, 1.0, now).unwrap();
    core.rate(11, 1.0, now).unwrap();
    core.rate(238, 5.0, now).unwrap();
    core.rate(240, 5.0, now).unwrap();
}

#[test]
fn with_decay_on_a_recent_nota_pattern_dominates_an_old_contradicting_one() {
    let mut core = with_model();
    core.set_decay(Decay::HalfLifeDays(180), 0);
    changed_taste(&mut core);

    // Duna 2 is sci-fi, A Lista de Schindler drama.
    assert!(prevista(&core, 424) > prevista(&core, 693134) + 1.0);
}

#[test]
fn disabling_decay_gives_every_nota_the_same_weight() {
    let mut core = with_model();
    core.set_decay(Decay::Off, 0);
    changed_taste(&mut core);

    let mut undated = with_model();
    undated.set_decay(Decay::HalfLifeDays(180), 0);
    for (id, nota) in [(603, 5.0), (157336, 5.0), (278, 1.0), (769, 1.0), (27205, 1.0), (11, 1.0), (238, 5.0), (240, 5.0)] {
        undated.rate(id, nota, 0).unwrap();
    }
    for id in [424, 693134] {
        assert!((prevista(&core, id) - prevista(&undated, id)).abs() < 1e-5, "{id}");
    }
}

#[test]
fn decay_is_calibrated_on_the_owners_notas_by_default() {
    let mut core = with_model();
    assert_eq!(core.decay().setting, Decay::Auto);

    // Ten notas of a taste that flipped: five old ones, five recent of the opposite taste.
    for (k, (id, nota)) in [(603, 5.0), (27205, 5.0), (157336, 5.0), (238, 1.0), (240, 1.0)].into_iter().enumerate() {
        core.rate(id, nota, k as i64 * DAY).unwrap();
    }
    for (k, (id, nota)) in [(693134, 1.0), (11, 1.0), (278, 5.0), (769, 5.0), (424, 5.0)].into_iter().enumerate() {
        core.rate(id, nota, (2000 + k as i64) * DAY).unwrap();
    }

    assert!(core.decay().half_life_days.is_some(), "a flipped taste calls for decay");
}

#[test]
fn few_notas_are_not_enough_to_calibrate() {
    let mut core = with_model();
    core.rate(603, 5.0, 0).unwrap();
    core.rate(278, 1.0, 1000 * DAY).unwrap();
    assert_eq!(core.decay().half_life_days, None);
}

#[test]
fn decay_setting_survives_a_restart() {
    let mut core = with_model();
    core.set_decay(Decay::HalfLifeDays(365), 0);

    let mut restarted = with_model();
    restarted.load_owner_data(&core.owner_data()).unwrap();
    assert_eq!(restarted.decay().setting, Decay::HalfLifeDays(365));
    assert_eq!(restarted.decay().half_life_days, Some(365));
}

#[test]
fn owner_data_saved_before_settings_existed_still_loads() {
    let mut core = with_model();
    core.load_owner_data(br#"{"notas": {}, "quero_assistir": []}"#).unwrap();
    assert_eq!(core.decay().setting, Decay::Auto);
}
