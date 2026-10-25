//! Seam 2: the training job, run on a tiny ratings fixture, produces a model the core loads
//! and predicts with.

use cinelens_core::{content_features, Core, Filtro};
use cinelens_train::{catalog_imdb_ids, train, Letterboxd};

/// MovieLens ids 1–4 are sci-fi, 5–8 drama; 30 users, half loving each side and disliking the
/// other. Movie 9 has too few ratings to be published; movie 10 has no TMDB id.
fn ratings_csv() -> String {
    let mut csv = String::from("userId,movieId,rating,timestamp\n");
    for user in 1..=30u32 {
        let likes_sci_fi = user % 2 == 0;
        for movie in 1..=10u32 {
            // Each user skips one movie of 1–8, so predictions come from others' ratings.
            if movie <= 8 && movie == user % 8 + 1 || movie == 9 && user > 3 {
                continue;
            }
            let sci_fi = movie <= 4;
            let rating = match (likes_sci_fi == sci_fi, movie) {
                (_, 9 | 10) => 3.0,
                (true, _) => if user % 3 == 0 { 4.5 } else { 5.0 },
                (false, _) => if user % 3 == 0 { 2.0 } else { 1.5 },
            };
            csv += &format!("{user},{movie},{rating},1700000000\n");
        }
    }
    csv
}

const LINKS: &str = "movieId,imdbId,tmdbId\n\
    1,0000001,101\n2,0000002,102\n3,0000003,103\n4,0000004,104\n\
    5,0000005,105\n6,0000006,106\n7,0000007,107\n8,0000008,108\n\
    9,0000009,109\n10,0000010,\n";

/// TMDB ids 101–110 are the MovieLens movies; 111 and 112 are 2025 releases only Letterboxd has;
/// 113 (sci-fi) and 114 (drama) are 2026 releases nobody rated yet. Sci-fi is genre 878 by
/// director 900, drama genre 18 by director 901.
fn catalog() -> Vec<u8> {
    let movies: Vec<String> = (101..=114)
        .map(|id| {
            let sci_fi = matches!(id, 101..=104 | 111 | 112 | 113);
            let (genre, director) = if sci_fi { (878, 900) } else { (18, 901) };
            format!(
                r#"{{"tmdb_id":{id},"title":"Filme {id}","original_title":"Movie {id}","year":2000,"poster_path":null,"imdb_id":"tt{:07}","genre_ids":[{genre}],"directors":[{{"id":{director},"name":"X"}}],"cast":[],"original_language":"en"}}"#,
                id + 8_999_900
            )
        })
        .collect();
    format!(r#"{{"format_version":1,"data_version":"t","movies":[{}]}}"#, movies.join(",")).into_bytes()
}

fn trained_core() -> Core {
    let content = content_features(&catalog()).unwrap();
    let model = train(ratings_csv().as_bytes(), LINKS.as_bytes(), None, &content, "2026-10-25").unwrap();
    let mut core = Core::open(&catalog()).unwrap();
    core.load_model(&serde_json::to_vec(&model).unwrap()).unwrap();
    core
}

#[test]
fn core_predicts_with_the_trained_model() {
    let mut core = trained_core();
    assert_eq!(core.model_version(), Some("2026-10-25"));

    core.rate(101, 5.0, 100).unwrap();
    core.rate(105, 1.0, 100).unwrap();

    let prevista = |id| core.movie(id).unwrap().card.nota_prevista.unwrap();
    for sci_fi in [102, 103, 104] {
        for drama in [106, 107, 108] {
            assert!(prevista(sci_fi) > prevista(drama) + 1.0, "{sci_fi} vs {drama}");
        }
    }
    let ids: Vec<u32> = core.explorar(&Filtro::default(), 0).movies.iter().take(3).map(|m| m.tmdb_id).collect();
    assert!(ids.iter().all(|id| (102..=104).contains(id)), "{ids:?}");
}

#[test]
fn movies_nobody_rated_are_predicted_from_their_content() {
    let mut core = trained_core();
    core.rate(101, 5.0, 100).unwrap();
    core.rate(105, 1.0, 100).unwrap();

    let card = |id| core.movie(id).unwrap().card;
    assert!(card(113).nota_prevista.unwrap() > card(114).nota_prevista.unwrap() + 1.0);
    assert!(card(113).confianca.unwrap() < card(102).confianca.unwrap());
}

#[test]
fn model_leaves_out_movies_with_few_ratings() {
    let core = trained_core();
    // Movie 109 shares its content with the dramas, so it is predicted from that alone.
    assert!(core.movie(109).unwrap().card.confianca.unwrap() < core.movie(108).unwrap().card.confianca.unwrap());
    assert!(core.movie(108).unwrap().card.nota_prevista.is_some());
}

#[test]
fn malformed_ratings_fail_the_job() {
    let bad = "userId,movieId,rating,timestamp\n1,1,abc,1700000000\n";
    assert!(train(bad.as_bytes(), LINKS.as_bytes(), None, &[], "x").is_err());
}

/// 20 Letterboxd users, rating about half a star lower than MovieLens users, with the same two
/// tastes over the MovieLens movies (joined through `links.csv`) and two releases MovieLens
/// lacks: tt9000011 (sci-fi, matched with confidence) and tt9000012, matched with low confidence
/// only. Every user also logged a movie without rating it.
fn letterboxd_csv() -> String {
    let mut csv = String::from("user_id,film_slug,film_name,film_year,rating,liked,tconst,match_rule,confidence\n");
    for user in 1..=20u32 {
        let likes_sci_fi = user % 2 == 0;
        let row = |tconst: &str, rating: f32, confidence: f32| {
            format!("u_{user:05},slug,\"Name, with comma\",2025,{rating},false,{tconst},R1,{confidence}\n")
        };
        for movie in 1..=8u32 {
            let rating = if likes_sci_fi == (movie <= 4) { 4.5 } else { 1.0 };
            csv += &row(&format!("tt{movie:07}"), rating, 1.0);
        }
        csv += &row("tt9000011", if likes_sci_fi { 4.5 } else { 1.0 }, 0.95);
        csv += &row("tt9000012", if likes_sci_fi { 4.5 } else { 1.0 }, 0.3);
        csv += &format!("u_{user:05},seen,Seen,2025,,true,tt0000003,R1,1.0\n");
    }
    csv
}

fn with_letterboxd() -> Core {
    let letterboxd = Letterboxd {
        interactions: Box::new(std::io::Cursor::new(letterboxd_csv())),
        imdb_to_tmdb: catalog_imdb_ids(&catalog()).unwrap(),
    };
    let model = train(ratings_csv().as_bytes(), LINKS.as_bytes(), Some(letterboxd), &[], "2026-10-25").unwrap();
    let mut core = Core::open(&catalog()).unwrap();
    core.load_model(&serde_json::to_vec(&model).unwrap()).unwrap();
    core
}

#[test]
fn letterboxd_brings_releases_movielens_lacks() {
    let mut core = with_letterboxd();
    core.rate(101, 5.0, 100).unwrap();
    core.rate(105, 1.0, 100).unwrap();

    let prevista = |id| core.movie(id).unwrap().card.nota_prevista.unwrap();
    assert!(prevista(111) > prevista(106) + 1.0);
}

#[test]
fn letterboxd_matches_with_low_confidence_are_left_out() {
    let core = with_letterboxd();
    assert!(core.movie(111).unwrap().card.nota_prevista.is_some());
    assert_eq!(core.movie(112).unwrap().card.nota_prevista, None);
}

#[test]
fn letterboxd_users_are_trained_alongside_movielens_users() {
    let mut core = with_letterboxd();
    core.rate(101, 5.0, 100).unwrap();
    core.rate(105, 1.0, 100).unwrap();

    let prevista = |id| core.movie(id).unwrap().card.nota_prevista.unwrap();
    for sci_fi in [102, 103, 104] {
        for drama in [106, 107, 108] {
            assert!(prevista(sci_fi) > prevista(drama) + 1.0, "{sci_fi} vs {drama}");
        }
    }
}

#[test]
fn malformed_letterboxd_fails_the_job() {
    let bad = "user_id,film_slug,film_name,film_year,rating,liked,tconst,match_rule,confidence\nu_00001,s,n,2025,abc,false,tt0000001,R1,1.0\n";
    let letterboxd = Letterboxd { interactions: Box::new(bad.as_bytes()), imdb_to_tmdb: Default::default() };
    assert!(train(ratings_csv().as_bytes(), LINKS.as_bytes(), Some(letterboxd), &[], "x").is_err());
}
