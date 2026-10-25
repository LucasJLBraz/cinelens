//! Seam 2: the training job, run on a tiny ratings fixture, produces a model the core loads
//! and predicts with.

use cinelens_core::Core;
use cinelens_train::train;

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

fn catalog() -> Vec<u8> {
    let movies: Vec<String> = (101..=110)
        .map(|id| format!(r#"{{"tmdb_id":{id},"title":"Filme {id}","original_title":"Movie {id}","year":2000,"poster_path":null}}"#))
        .collect();
    format!(r#"{{"format_version":1,"data_version":"t","movies":[{}]}}"#, movies.join(",")).into_bytes()
}

fn trained_core() -> Core {
    let model = train(ratings_csv().as_bytes(), LINKS.as_bytes(), "2026-10-25").unwrap();
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
    let ids: Vec<u32> = core.explorar(0).movies.iter().take(3).map(|m| m.tmdb_id).collect();
    assert!(ids.iter().all(|id| (102..=104).contains(id)), "{ids:?}");
}

#[test]
fn model_leaves_out_movies_with_few_ratings() {
    let core = trained_core();
    assert_eq!(core.movie(109).unwrap().card.nota_prevista, None);
    assert!(core.movie(108).unwrap().card.nota_prevista.is_some());
}

#[test]
fn malformed_ratings_fail_the_job() {
    let bad = "userId,movieId,rating,timestamp\n1,1,abc,1700000000\n";
    assert!(train(bad.as_bytes(), LINKS.as_bytes(), "x").is_err());
}
