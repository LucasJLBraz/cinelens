//! The weekly notification about a new release with a high nota prevista.

use cinelens_core::{Core, DisplayMode};

/// 2026-10-26 00:00 UTC.
const HOJE: i64 = 1_792_972_800;
const DIA: i64 = 86_400;

/// Before any nota each nota prevista is 3.5 + bias. Out (2026-10-16) and Dois (2026-10-20) are
/// new releases; Velho (1999) and Futuro (2026-12-01) score higher but are not. `fillers` low
/// movies decide how many sit in Aposta forte (about the top 10%).
fn core(fillers: u32) -> Core {
    let movie = |id: u32, title: &str, date: &str| {
        format!(r#"{{"tmdb_id": {id}, "title": "{title}", "original_title": "{title}", "poster_path": null, "release_date": "{date}"}}"#)
    };
    let factors = |id: u32, bias: f32| format!(r#"{{"tmdb_id": {id}, "bias": {bias}, "factors": [0.0], "ratings": 50000}}"#);
    let mut movies = vec![
        movie(1, "Um", "2026-10-16"),
        movie(2, "Dois", "2026-10-20"),
        movie(3, "Velho", "1999-03-31"),
        movie(4, "Futuro", "2026-12-01"),
    ];
    let mut model = vec![factors(1, 1.0), factors(2, 0.9), factors(3, 1.2), factors(4, 1.5)];
    for id in 100..100 + fillers {
        movies.push(movie(id, "Enchimento", "2000-01-01"));
        model.push(factors(id, -1.0));
    }
    let catalog = format!(r#"{{"format_version": 1, "data_version": "t", "movies": [{}]}}"#, movies.join(","));
    let model = format!(
        r#"{{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05, "content": [], "movies": [{}]}}"#,
        model.join(",")
    );
    let mut core = Core::open(catalog.as_bytes()).unwrap();
    core.load_model(model.as_bytes()).unwrap();
    core
}

/// The notification due at `at`, shown.
fn notified(core: &mut Core, at: i64) -> Option<u32> {
    let id = core.notificacao_devida(at)?.tmdb_id;
    core.notificado(id, at);
    Some(id)
}

#[test]
fn the_best_new_release_with_a_high_nota_prevista_at_most_once_a_week() {
    let mut core = core(40);

    assert_eq!(notified(&mut core, HOJE), Some(1));
    assert_eq!(notified(&mut core, HOJE), None);
    assert_eq!(notified(&mut core, HOJE + 6 * DIA), None, "a week has not passed");
    assert_eq!(notified(&mut core, HOJE + 7 * DIA), Some(2), "Um was already announced");
    assert_eq!(notified(&mut core, HOJE + 14 * DIA), None);
}

#[test]
fn a_notification_not_shown_stays_due() {
    let mut core = core(40);
    assert_eq!(core.notificacao_devida(HOJE).map(|n| n.tmdb_id), Some(1));
    assert_eq!(notified(&mut core, HOJE + DIA), Some(1));
}

#[test]
fn not_for_a_new_release_below_aposta_forte() {
    // Only Futuro is in the top 10%.
    let mut core = core(5);
    assert_eq!(notified(&mut core, HOJE), None);
}

#[test]
fn not_for_a_release_the_owner_rated_or_saw() {
    let mut core = core(40);
    core.rate(1, 4.0, HOJE).unwrap();
    core.marcar(2, cinelens_core::Marca::JaVi, true, HOJE);
    assert_eq!(notified(&mut core, HOJE), None);
}

#[test]
fn the_text_follows_the_display_mode_with_no_number() {
    let texto = |mode: DisplayMode| {
        let mut core = core(40);
        core.set_display_mode(mode, HOJE);
        core.notificacao_devida(HOJE).unwrap().texto
    };
    assert_eq!(texto(DisplayMode::Faixa), "Saiu Um, Aposta forte");
    assert_eq!(texto(DisplayMode::Oculto), "Saiu Um");
}

#[test]
fn can_be_turned_off_and_the_choice_syncs_with_the_owner_data() {
    let mut core = core(40);
    assert!(core.notificacao());
    core.set_notificacao(false, HOJE);
    assert!(!core.notificacao());
    assert_eq!(notified(&mut core, HOJE), None);

    let mut restarted = self::core(40);
    restarted.load_owner_data(&core.owner_data()).unwrap();
    assert_eq!(notified(&mut restarted, HOJE), None);
    restarted.set_notificacao(true, HOJE + 1);
    assert_eq!(notified(&mut restarted, HOJE + 1), Some(1));
}

#[test]
fn a_notification_on_one_device_counts_on_the_others() {
    struct Blob(Option<Vec<u8>>);
    impl cinelens_core::SyncStorage for Blob {
        fn get(&mut self) -> Result<Option<Vec<u8>>, String> {
            Ok(self.0.clone())
        }
        fn put(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.0 = Some(bytes.to_vec());
            Ok(())
        }
    }
    let (mut phone, mut tablet, mut drive) = (core(40), core(40), Blob(None));
    assert_eq!(notified(&mut phone, HOJE), Some(1));
    phone.sync(&mut drive).unwrap();
    tablet.sync(&mut drive).unwrap();

    assert_eq!(notified(&mut tablet, HOJE + DIA), None);
    assert_eq!(notified(&mut tablet, HOJE + 7 * DIA), Some(2));
}
