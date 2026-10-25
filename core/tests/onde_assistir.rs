//! Where to watch in Brazil, and the services the owner subscribes to.

use cinelens_core::Core;

const CATALOG: &str = r#"{"format_version": 1, "data_version": "t",
    "providers": [{"id": 2, "name": "Apple TV"}, {"id": 8, "name": "Netflix"}],
    "movies": [
        {"tmdb_id": 1, "title": "A", "original_title": "A", "year": 1999, "poster_path": null,
         "watch": {"link": "https://www.themoviedb.org/movie/1/watch?locale=BR", "flatrate": [8], "rent": [2], "buy": [2, 99]}},
        {"tmdb_id": 2, "title": "B", "original_title": "B", "year": 2001, "poster_path": null}
    ]}"#;

#[test]
fn the_movie_page_lists_subscription_rent_and_buy_options() {
    let core = Core::open(CATALOG.as_bytes()).unwrap();
    let onde = core.movie(1).unwrap().onde_assistir.unwrap();

    let names = |p: &[cinelens_core::Provedor]| p.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(",");
    assert_eq!(names(&onde.assinatura), "Netflix");
    assert_eq!(names(&onde.aluguel), "Apple TV");
    // A provider the catálogo does not name is left out.
    assert_eq!(names(&onde.compra), "Apple TV");
    assert_eq!(onde.link.as_deref(), Some("https://www.themoviedb.org/movie/1/watch?locale=BR"));
    assert!(core.movie(2).unwrap().onde_assistir.is_none());
}

#[test]
fn subscribed_services_are_saved_with_the_owner_data() {
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    assert!(core.servicos().is_empty());

    core.set_servicos(&[8]);

    let mut restarted = Core::open(CATALOG.as_bytes()).unwrap();
    restarted.load_owner_data(&core.owner_data()).unwrap();
    assert_eq!(restarted.servicos(), [8]);
}
