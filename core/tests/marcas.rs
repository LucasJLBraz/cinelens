//! Já vi, não me interessa and agora não: what each mark does, and that each can be undone.

use cinelens_core::{Core, Filtro, Marca};

const CATALOG: &[u8] = include_bytes!("../fixtures/catalog.json");
/// Sci-fi: 603, 27205, 157336, 693134, 11. Crime/drama: 238, 240, 278, 769, 424.
const MODEL: &[u8] = include_bytes!("../fixtures/model.json");
const DAY: i64 = 86_400;

/// Loves sci-fi, dislikes drama; eight movies of the model left to predict.
fn owner() -> Core {
    let mut core = Core::open(CATALOG).unwrap();
    core.load_model(MODEL).unwrap();
    core.rate(603, 5.0, 100).unwrap();
    core.rate(278, 1.5, 100).unwrap();
    core
}

/// Every movie Explorar lists, in order.
fn explorar(core: &Core) -> Vec<u32> {
    let mut ids = Vec::new();
    for page in 0.. {
        let result = core.explorar(&Filtro::default(), page);
        ids.extend(result.movies.iter().map(|m| m.tmdb_id));
        if !result.has_more {
            return ids;
        }
    }
    unreachable!()
}

/// The recomendações: the movies Explorar ranks by nota prevista, in order.
fn recomendacoes(core: &Core) -> Vec<u32> {
    let mut ids = Vec::new();
    for page in 0.. {
        let result = core.explorar(&Filtro::default(), page);
        let ranked = result.movies.iter().take_while(|m| m.nota_prevista.is_some() && !core.movie(m.tmdb_id).unwrap().ja_vi);
        ids.extend(ranked.map(|m| m.tmdb_id));
        if !result.has_more {
            return ids;
        }
    }
    unreachable!()
}

fn prevista(core: &Core, tmdb_id: u32) -> f32 {
    core.movie(tmdb_id).unwrap().card.nota_prevista.unwrap()
}

#[test]
fn ja_vi_removes_a_movie_from_the_recomendacoes_without_changing_the_profile() {
    let mut core = owner();
    let before = recomendacoes(&core);
    assert_eq!(before[0], 157336);
    let inception = prevista(&core, 27205);

    core.marcar(157336, Marca::JaVi, true, 200);

    let after = recomendacoes(&core);
    assert!(!after.contains(&157336));
    assert_eq!(after, before[1..]);
    assert_eq!(prevista(&core, 27205), inception);
    // Still in the catálogo, with its nota prevista (for Para rever), and on its page.
    assert!(explorar(&core).contains(&157336));
    assert!(core.movie(157336).unwrap().ja_vi);
    assert!(core.movie(157336).unwrap().card.nota_prevista.is_some());

    core.marcar(157336, Marca::JaVi, false, 300);
    assert_eq!(recomendacoes(&core), before);
    assert!(!core.movie(157336).unwrap().ja_vi);
}

#[test]
fn nao_me_interessa_hides_the_movie_everywhere() {
    let mut core = owner();

    core.marcar(157336, Marca::NaoMeInteressa, true, 200);

    assert!(!explorar(&core).contains(&157336));
    assert_ne!(core.destaque(200).map(|m| m.tmdb_id), Some(157336));
    assert!(core.movie(157336).unwrap().nao_me_interessa);

    core.marcar(157336, Marca::NaoMeInteressa, false, 300);
    assert!(explorar(&core).contains(&157336));
    assert!(!core.movie(157336).unwrap().nao_me_interessa);
}

#[test]
fn nao_me_interessa_is_a_weak_negative_signal() {
    let core = owner();
    let inception = prevista(&core, 27205);

    let mut dismissed = owner();
    dismissed.marcar(157336, Marca::NaoMeInteressa, true, 200);
    let mut rated_low = owner();
    rated_low.rate(157336, 1.0, 200).unwrap();

    let after_dismissal = prevista(&dismissed, 27205);
    assert!(after_dismissal < inception, "a similar movie goes down: {after_dismissal} vs {inception}");
    assert!(after_dismissal > prevista(&rated_low, 27205), "but less than a low nota would");

    dismissed.marcar(157336, Marca::NaoMeInteressa, false, 300);
    assert_eq!(prevista(&dismissed, 27205), inception);
}

#[test]
fn agora_nao_hides_a_movie_from_the_destaque_for_a_few_weeks() {
    let mut core = owner();
    let now = 1_000 * DAY;
    let featured = core.destaque(now).unwrap().tmdb_id;
    assert_eq!(featured, 157336);

    core.marcar(featured, Marca::AgoraNao, true, now);

    let next = core.destaque(now + DAY).unwrap().tmdb_id;
    assert_ne!(next, featured);
    assert_eq!(core.destaque(now + 13 * DAY).unwrap().tmdb_id, next);
    assert_eq!(core.destaque(now + 29 * DAY).unwrap().tmdb_id, featured);
    // Only the destaque: still a recomendação in Explorar meanwhile.
    assert_eq!(recomendacoes(&core)[0], featured);

    core.marcar(featured, Marca::AgoraNao, false, now + DAY);
    assert_eq!(core.destaque(now + DAY).unwrap().tmdb_id, featured);
}

#[test]
fn the_destaque_skips_rated_and_ja_vi_movies() {
    let mut core = owner();
    core.marcar(157336, Marca::JaVi, true, 200);
    let featured = core.destaque(200).unwrap().tmdb_id;
    assert_ne!(featured, 157336);
    assert!(core.movie(featured).unwrap().card.nota_prevista.is_some());
}

#[test]
fn quero_assistir_can_be_set_and_cleared() {
    let mut core = owner();
    core.marcar(27205, Marca::QueroAssistir, true, 200);
    assert!(core.movie(27205).unwrap().quero_assistir);
    core.marcar(27205, Marca::QueroAssistir, false, 300);
    assert!(!core.movie(27205).unwrap().quero_assistir);
}

#[test]
fn marks_survive_a_restart() {
    let mut core = owner();
    core.marcar(157336, Marca::JaVi, true, 200);
    core.marcar(27205, Marca::NaoMeInteressa, true, 200);
    core.marcar(11, Marca::AgoraNao, true, 200);

    let mut restarted = Core::open(CATALOG).unwrap();
    restarted.load_model(MODEL).unwrap();
    restarted.load_owner_data(&core.owner_data()).unwrap();
    assert!(restarted.movie(157336).unwrap().ja_vi);
    assert!(restarted.movie(27205).unwrap().nao_me_interessa);
    assert_eq!(restarted.destaque(300).map(|m| m.tmdb_id), core.destaque(300).map(|m| m.tmdb_id));
    assert_eq!(prevista(&restarted, 693134), prevista(&core, 693134));
}
