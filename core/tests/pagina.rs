//! The full movie page: details, parecidos, the reason line, people, filmographies and sou fã.

use cinelens_core::{Core, Filtro, Marca};

/// Keanu Reeves (20) is in 1, 2, 3 and 8; the Wachowskis (10) directed 1 and 2. 8 is not in the model.
const CATALOG: &str = r#"{"format_version": 1, "data_version": "t",
    "genres": [{"id": 28, "name": "Ação"}, {"id": 878, "name": "Ficção científica"}, {"id": 18, "name": "Drama"}],
    "movies": [
    {"tmdb_id": 1, "title": "Matrix", "original_title": "The Matrix", "year": 1999, "runtime": 136, "poster_path": "/m.jpg", "backdrop_path": "/bm.jpg",
     "genre_ids": [878, 28], "original_language": "en", "overview": "Um hacker descobre a verdade.", "trailer_youtube": "yt1",
     "vote_count": 20000, "vote_average": 8.2, "critic_score": 73,
     "directors": [{"id": 10, "name": "Lana Wachowski"}], "cast": [{"id": 20, "name": "Keanu Reeves"}, {"id": 21, "name": "Carrie-Anne Moss"}],
     "composers": [{"id": 30, "name": "Don Davis"}]},
    {"tmdb_id": 2, "title": "Matrix Reloaded", "original_title": "The Matrix Reloaded", "year": 2003, "poster_path": null, "genre_ids": [878], "vote_count": 100, "vote_average": 7.0,
     "directors": [{"id": 10, "name": "Lana Wachowski"}], "cast": [{"id": 20, "name": "Keanu Reeves"}]},
    {"tmdb_id": 3, "title": "John Wick", "original_title": "John Wick", "year": 2014, "poster_path": null, "genre_ids": [28],
     "directors": [{"id": 11, "name": "Chad Stahelski"}], "cast": [{"id": 20, "name": "Keanu Reeves"}]},
    {"tmdb_id": 4, "title": "Interestelar", "original_title": "Interstellar", "year": 2014, "poster_path": null, "genre_ids": [878],
     "directors": [{"id": 12, "name": "Christopher Nolan"}]},
    {"tmdb_id": 5, "title": "O Poderoso Chefão", "original_title": "The Godfather", "year": 1972, "poster_path": null, "genre_ids": [18],
     "directors": [{"id": 13, "name": "Francis Ford Coppola"}]},
    {"tmdb_id": 6, "title": "Os Bons Companheiros", "original_title": "Goodfellas", "year": 1990, "poster_path": null, "genre_ids": [18],
     "directors": [{"id": 14, "name": "Martin Scorsese"}], "composers": [{"id": 31, "name": "Compositor"}]},
    {"tmdb_id": 7, "title": "Cidade de Deus", "original_title": "Cidade de Deus", "year": 2002, "poster_path": null, "genre_ids": [18],
     "directors": [{"id": 15, "name": "Fernando Meirelles"}]},
    {"tmdb_id": 8, "title": "Velocidade Máxima", "original_title": "Speed", "year": 1994, "poster_path": null, "genre_ids": [28],
     "cast": [{"id": 20, "name": "Keanu Reeves"}]}
]}"#;

/// Sci-fi along the first factor, drama along the second; John Wick in between.
const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05, "content": [],
    "movies": [
        {"tmdb_id": 1, "bias": 0.3, "factors": [1.0, 0.0], "ratings": 50000},
        {"tmdb_id": 2, "bias": 0.0, "factors": [0.9, 0.1], "ratings": 20000},
        {"tmdb_id": 3, "bias": 0.1, "factors": [0.6, 0.4], "ratings": 20000},
        {"tmdb_id": 4, "bias": 0.3, "factors": [1.0, -0.1], "ratings": 40000},
        {"tmdb_id": 5, "bias": 0.5, "factors": [0.0, 1.0], "ratings": 50000},
        {"tmdb_id": 6, "bias": 0.3, "factors": [-0.1, 1.0], "ratings": 30000},
        {"tmdb_id": 7, "bias": 0.3, "factors": [0.1, 0.9], "ratings": 30000}
    ]}"#;

/// Loved Interestelar, disliked O Poderoso Chefão.
fn owner() -> Core {
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    core.load_model(MODEL.as_bytes()).unwrap();
    core.rate(4, 5.0, 100).unwrap();
    core.rate(5, 1.0, 100).unwrap();
    core
}

fn names(people: &[cinelens_core::Pessoa]) -> Vec<&str> {
    people.iter().map(|p| p.name.as_str()).collect()
}

fn para_voce(core: &Core) -> Vec<u32> {
    core.explorar(&Filtro::default(), 0).movies.iter().map(|m| m.tmdb_id).collect()
}

#[test]
fn the_page_carries_every_detail_of_the_movie() {
    let page = owner().movie(1).unwrap();

    assert_eq!(page.backdrop_url.as_deref(), Some("https://image.tmdb.org/t/p/w780/bm.jpg"));
    assert_eq!(page.card.poster_url.as_deref(), Some("https://image.tmdb.org/t/p/w342/m.jpg"));
    assert_eq!((page.card.title.as_str(), page.card.original_title.as_str()), ("Matrix", "The Matrix"));
    assert_eq!((page.card.year, page.runtime), (Some(1999), Some(136)));
    assert_eq!(page.generos, ["Ficção científica", "Ação"]);
    assert_eq!(page.idioma.as_deref(), Some("en"));
    assert_eq!(page.sinopse, "Um hacker descobre a verdade.");
    assert_eq!(page.trailer_youtube.as_deref(), Some("yt1"));
    assert_eq!(names(&page.direcao), ["Lana Wachowski"]);
    assert_eq!(names(&page.elenco), ["Keanu Reeves", "Carrie-Anne Moss"]);
    assert_eq!(names(&page.trilha), ["Don Davis"]);
    assert!(page.card.exibicao.is_some());
}

#[test]
fn the_page_shows_the_publics_score_in_stars_and_the_critics_score() {
    let core = owner();
    let matrix = core.movie(1).unwrap();
    assert_eq!((matrix.publico, matrix.critica), (Some(4.1), Some(73)));

    // Too few TMDB votes to say what the public thinks; no critics' score in the catálogo.
    let reloaded = core.movie(2).unwrap();
    assert_eq!((reloaded.publico, reloaded.critica), (None, None));
}

#[test]
fn parecidos_are_the_movies_most_alike_in_who_liked_them() {
    let mut core = owner();
    let parecidos = |core: &Core| core.movie(2).unwrap().parecidos.iter().map(|m| m.tmdb_id).collect::<Vec<_>>();

    let found = parecidos(&core);
    assert_eq!(found[..3], [1, 4, 3], "the other sci-fi first, then John Wick");
    assert!(!found.contains(&2), "never the movie itself");
    assert!(!found.contains(&8), "nor movies outside the model");
    assert!(found.iter().position(|&m| m == 3) < found.iter().position(|&m| m == 6));

    core.marcar(4, Marca::NaoMeInteressa, true, 200);
    assert!(!parecidos(&core).contains(&4));
}

#[test]
fn the_reason_line_names_movies_the_owner_rated_highly() {
    let core = owner();
    assert_eq!(core.movie(2).unwrap().porque, ["Interestelar"]);
    // The owner disliked the only drama they rated, so a drama has no reason to give.
    assert!(core.movie(6).unwrap().porque.is_empty());
    // Barely alike is no reason either: Cidade de Deus shares almost nothing with Interestelar.
    assert!(core.movie(7).unwrap().porque.is_empty());
}

#[test]
fn a_persons_filmography_is_ordered_by_nota_prevista() {
    let core = owner();
    let keanu = core.pessoa(20).unwrap();

    assert_eq!(keanu.pessoa.name, "Keanu Reeves");
    assert_eq!(keanu.movies.iter().map(|m| m.tmdb_id).collect::<Vec<_>>(), [1, 2, 3, 8]);
    assert!(core.pessoa(999).is_none());
}

#[test]
fn sou_fa_raises_a_persons_movies_in_the_ranking() {
    let mut core = owner();
    let goodfellas = core.movie(6).unwrap().card.nota_prevista;
    let before = para_voce(&core);
    assert!(before.iter().position(|&m| m == 6) > before.iter().position(|&m| m == 7));

    core.set_fa(14, true, 0);

    let after = para_voce(&core);
    assert!(after.iter().position(|&m| m == 6) < after.iter().position(|&m| m == 7), "{after:?}");
    assert_eq!(core.movie(6).unwrap().card.nota_prevista, goodfellas, "the nota prevista itself stays");
    assert!(core.movie(6).unwrap().direcao[0].fa);
    assert!(core.pessoa(14).unwrap().pessoa.fa);

    core.set_fa(14, false, 0);
    assert_eq!(para_voce(&core), before);
}

#[test]
fn sou_fa_works_for_composers_and_survives_a_restart() {
    let mut core = owner();
    core.set_fa(31, true, 0);
    let ranked = para_voce(&core);
    assert!(ranked.iter().position(|&m| m == 6) < ranked.iter().position(|&m| m == 7), "{ranked:?}");

    let mut restarted = Core::open(CATALOG.as_bytes()).unwrap();
    restarted.load_model(MODEL.as_bytes()).unwrap();
    restarted.load_owner_data(&core.owner_data()).unwrap();
    assert_eq!(para_voce(&restarted), ranked);
}
