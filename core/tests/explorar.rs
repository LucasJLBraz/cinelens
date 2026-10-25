use cinelens_core::{Core, Filtro};

const FIXTURE: &[u8] = include_bytes!("../fixtures/catalog.json");

#[test]
fn explorar_returns_first_page_of_fixture_catalogo() {
    let core = Core::open(FIXTURE).unwrap();
    let page = core.explorar(&Filtro::default(), 0);

    assert_eq!(page.movies.len(), 30);
    assert!(page.has_more);
    let first = &page.movies[0];
    assert_eq!(first.tmdb_id, 550);
    assert_eq!(first.title, "Clube da Luta");
    assert_eq!(first.original_title, "Fight Club");
    assert_eq!(first.year, Some(1999));
    assert_eq!(
        first.poster_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w342/mCICnh7QBH0gzYaTQChBDDVIKdm.jpg")
    );
}

#[test]
fn explorar_last_page_holds_the_rest() {
    let core = Core::open(FIXTURE).unwrap();
    let page = core.explorar(&Filtro::default(), 1);

    assert_eq!(page.movies.len(), 12);
    assert!(!page.has_more);
}

#[test]
fn open_refuses_catalogo_with_unknown_format_version() {
    let artifact = br#"{"format_version": 2, "data_version": "x", "movies": []}"#;
    assert!(Core::open(artifact).is_err());
}

#[test]
fn new_core_has_no_catalogo_until_one_is_loaded() {
    let mut core = Core::default();
    assert_eq!(core.data_version(), None);
    assert!(core.explorar(&Filtro::default(), 0).movies.is_empty());

    core.load_catalog(FIXTURE).unwrap();
    assert_eq!(core.data_version(), Some("fixture-2026-10-25"));
    assert_eq!(core.explorar(&Filtro::default(), 0).movies.len(), 30);
}

#[test]
fn loading_catalogo_with_unknown_format_version_keeps_the_current_one() {
    let mut core = Core::open(FIXTURE).unwrap();
    let newer = br#"{"format_version": 2, "data_version": "x", "movies": []}"#;

    assert!(core.load_catalog(newer).is_err());
    assert_eq!(core.data_version(), Some("fixture-2026-10-25"));
    assert_eq!(core.explorar(&Filtro::default(), 0).movies.len(), 30);
}
