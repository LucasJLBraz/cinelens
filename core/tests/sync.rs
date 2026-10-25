//! Sync between two of the owner's devices through a shared storage: per item, the most
//! recent change wins.

use cinelens_core::{Core, Marca, SyncStorage};

const CATALOG: &[u8] = include_bytes!("../fixtures/catalog.json");

/// The owner's Drive, in memory.
#[derive(Default)]
struct Nuvem(Option<Vec<u8>>);

impl SyncStorage for Nuvem {
    fn get(&mut self) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.clone())
    }

    fn put(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.0 = Some(bytes.to_vec());
        Ok(())
    }
}

fn device() -> Core {
    Core::open(CATALOG).unwrap()
}

/// Both devices sync, then the first again: what both see afterwards.
fn sync_all(a: &mut Core, b: &mut Core, nuvem: &mut Nuvem) {
    a.sync(nuvem).unwrap();
    b.sync(nuvem).unwrap();
    a.sync(nuvem).unwrap();
}

#[test]
fn a_mark_made_on_one_device_reaches_the_other() {
    let (mut a, mut b, mut nuvem) = (device(), device(), Nuvem::default());
    a.marcar(603, Marca::QueroAssistir, true, 100);
    sync_all(&mut a, &mut b, &mut nuvem);
    assert!(b.movie(603).unwrap().quero_assistir);
}

#[test]
fn an_unmark_reaches_the_other_device() {
    let (mut a, mut b, mut nuvem) = (device(), device(), Nuvem::default());
    a.marcar(603, Marca::JaVi, true, 100);
    sync_all(&mut a, &mut b, &mut nuvem);
    b.marcar(603, Marca::JaVi, false, 200);
    sync_all(&mut a, &mut b, &mut nuvem);
    assert!(!a.movie(603).unwrap().ja_vi);
    assert!(!b.movie(603).unwrap().ja_vi);
}

#[test]
fn concurrent_edits_converge_with_the_most_recent_winning_per_item() {
    let (mut a, mut b, mut nuvem) = (device(), device(), Nuvem::default());
    // Offline on both: each changes the same two movies, in a different order.
    a.marcar(603, Marca::QueroAssistir, true, 100);
    b.marcar(603, Marca::QueroAssistir, false, 150);
    b.marcar(238, Marca::NaoMeInteressa, true, 100);
    a.marcar(238, Marca::NaoMeInteressa, false, 150);
    sync_all(&mut a, &mut b, &mut nuvem);
    for core in [&a, &b] {
        assert!(!core.movie(603).unwrap().quero_assistir, "unmarked last");
        assert!(!core.movie(238).unwrap().nao_me_interessa, "unmarked last");
    }
    assert_eq!(a.owner_data(), b.owner_data());
}

#[test]
fn every_nota_given_on_either_device_stays_in_the_history() {
    let (mut a, mut b, mut nuvem) = (device(), device(), Nuvem::default());
    a.rate(603, 3.0, 100).unwrap();
    sync_all(&mut a, &mut b, &mut nuvem);
    b.rate(603, 4.5, 300).unwrap();
    a.rate(603, 4.0, 200).unwrap();
    sync_all(&mut a, &mut b, &mut nuvem);
    for core in [&a, &b] {
        let page = core.movie(603).unwrap();
        let history: Vec<(f32, i64)> = page.history.iter().map(|e| (e.nota, e.at)).collect();
        assert_eq!(history, [(3.0, 100), (4.0, 200), (4.5, 300)]);
        assert_eq!(page.nota, Some(4.5), "the most recent is current");
    }
}

#[test]
fn settings_and_o_que_pesou_merge_per_item() {
    use cinelens_core::{Aspecto, DisplayMode};
    let (mut a, mut b, mut nuvem) = (device(), device(), Nuvem::default());
    a.set_display_mode(DisplayMode::Estrelas, 100);
    b.set_servicos(&[8], 200);
    b.set_pesou(603, &[Aspecto::Roteiro], 100);
    a.set_pesou(603, &[Aspecto::Fotografia], 200);
    sync_all(&mut a, &mut b, &mut nuvem);
    for core in [&a, &b] {
        assert_eq!(core.display_mode(), DisplayMode::Estrelas);
        assert_eq!(core.servicos(), [8]);
        assert_eq!(core.movie(603).unwrap().pesou, [Aspecto::Fotografia]);
    }
}

#[test]
fn the_first_sync_shares_the_owner_data_and_the_next_changes_nothing() {
    let (mut a, mut nuvem) = (device(), Nuvem::default());
    a.marcar(603, Marca::QueroAssistir, true, 100);
    a.sync(&mut nuvem).unwrap();
    let shared = nuvem.0.clone().unwrap();
    assert_eq!(shared, a.owner_data());
    a.sync(&mut nuvem).unwrap();
    assert_eq!(nuvem.0.unwrap(), shared);
}

#[test]
fn settings_chosen_before_sync_existed_survive_a_new_device() {
    let (mut a, mut b, mut nuvem) = (device(), device(), Nuvem::default());
    // Saved by an earlier version: no record of when it changed.
    a.load_owner_data(br#"{"notas": {}, "quero_assistir": [], "settings": {"servicos": [8], "display_mode": "Estrelas"}}"#).unwrap();
    sync_all(&mut b, &mut a, &mut nuvem);
    b.sync(&mut nuvem).unwrap();
    for core in [&a, &b] {
        assert_eq!(core.servicos(), [8]);
        assert_eq!(core.display_mode(), cinelens_core::DisplayMode::Estrelas);
    }
}
