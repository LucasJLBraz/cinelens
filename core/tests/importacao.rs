use cinelens_core::{Core, ImportSummary, NotaEntry};

const CATALOG: &[u8] = include_bytes!("../fixtures/catalog.json");
const RATINGS: &str = include_str!("../fixtures/movielens/ratings.csv");
const LOGS: &str = include_str!("../fixtures/movielens/logs.csv");
const WISHLIST: &str = include_str!("../fixtures/movielens/wishlist.csv");
const TAGS: &str = include_str!("../fixtures/movielens/tags.csv");

fn imported() -> Core {
    let mut core = Core::open(CATALOG).unwrap();
    core.import_movielens(&[TAGS, WISHLIST, LOGS, RATINGS]).unwrap();
    core
}

#[test]
fn importacao_summary_counts_notas_matched_by_tmdb_id() {
    let mut core = Core::open(CATALOG).unwrap();
    let summary = core.import_movielens(&[RATINGS, LOGS, WISHLIST, TAGS]).unwrap();

    assert_eq!(summary, ImportSummary { notas: 4, matched: 3, unmatched: 1, quero_assistir: 1 });
}

#[test]
fn imported_nota_keeps_full_history_dated_from_the_logs() {
    let page = imported().movie(550).unwrap();

    assert_eq!(page.nota, Some(4.5));
    assert_eq!(
        page.history,
        vec![
            // 2024-09-07 20:12:01 UTC
            NotaEntry { nota: 4.0, at: 1725739921, movielens_pred: Some(4.204193673714679), prevista: None },
            // 2026-05-22 20:26:11 UTC
            NotaEntry { nota: 4.5, at: 1779481571, movielens_pred: Some(4.3), prevista: None },
        ]
    );
}

#[test]
fn importacao_ignores_ratings_removed_on_movielens() {
    // Movie 2571 has a quoted title with a comma; 174055 was rated then removed.
    let page = imported().movie(603).unwrap();
    assert_eq!(page.nota, Some(5.0));
    assert_eq!(page.history.len(), 1);
}

#[test]
fn movies_hidden_on_movielens_are_not_notas() {
    // MovieLens logs hiding a movie as a -1 rating; Fight Club was hidden between its notas.
    let history = imported().movie(550).unwrap().history;
    let notas: Vec<f32> = history.iter().map(|e| e.nota).collect();
    assert_eq!(notas, vec![4.0, 4.5]);
}

#[test]
fn wishlist_lands_in_quero_assistir() {
    let core = imported();
    assert!(core.movie(238).unwrap().quero_assistir);
    assert!(!core.movie(550).unwrap().quero_assistir);
}

#[test]
fn importacao_needs_ratings_and_logs() {
    let mut core = Core::open(CATALOG).unwrap();
    assert!(core.import_movielens(&[RATINGS, WISHLIST]).is_err());
    assert!(core.import_movielens(&[LOGS]).is_err());
    assert!(core.movie(550).unwrap().history.is_empty());
}

#[test]
fn owner_gives_a_nota_and_rerating_keeps_the_previous_one() {
    let mut core = Core::open(CATALOG).unwrap();
    assert_eq!(core.movie(278).unwrap().nota, None);

    core.rate(278, 3.5, 100).unwrap();
    core.rate(278, 4.5, 200).unwrap();

    let page = core.movie(278).unwrap();
    assert_eq!(page.nota, Some(4.5));
    let notas: Vec<f32> = page.history.iter().map(|e| e.nota).collect();
    assert_eq!(notas, vec![3.5, 4.5]);
}

#[test]
fn nota_must_be_half_stars_from_half_to_five() {
    let mut core = Core::open(CATALOG).unwrap();
    for bad in [0.0, 1.2, 5.5, -1.0, f32::NAN] {
        assert!(core.rate(278, bad, 100).is_err(), "{bad}");
    }
    assert_eq!(core.movie(278).unwrap().nota, None);
}

#[test]
fn reimporting_does_not_duplicate_and_keeps_newer_in_app_nota() {
    let mut core = imported();
    // After the export's last log line.
    core.rate(550, 2.0, 1_800_000_000).unwrap();

    let again = core.import_movielens(&[RATINGS, LOGS, WISHLIST]).unwrap();

    assert_eq!(again.notas, 4);
    let page = core.movie(550).unwrap();
    assert_eq!(page.history.len(), 3);
    assert_eq!(page.nota, Some(2.0));
    assert_eq!(core.movie(603).unwrap().history.len(), 1);
}

#[test]
fn owner_data_survives_a_restart() {
    let core = imported();
    let saved = core.owner_data();

    let mut restarted = Core::open(CATALOG).unwrap();
    restarted.load_owner_data(&saved).unwrap();

    assert_eq!(restarted.movie(550).unwrap().history, core.movie(550).unwrap().history);
    assert!(restarted.movie(238).unwrap().quero_assistir);
    assert!(restarted.load_owner_data(b"not json").is_err());
    assert_eq!(restarted.movie(550).unwrap().nota, Some(4.5));
}
