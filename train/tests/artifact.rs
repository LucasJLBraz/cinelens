//! Seam 2: the training job, run on a tiny ratings fixture, produces a model the core loads
//! and predicts with.

use cinelens_core::{content_features, Core, Direcao, Filtro};
use cinelens_train::{add_genome, catalog_titles, read_slugs, train, Letterboxd};

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
/// 113 (sci-fi) and 114 (drama) are 2026 releases nobody rated yet, which share an original
/// title. Sci-fi is genre 878 by director 900, drama genre 18 by director 901.
fn catalog() -> Vec<u8> {
    let movies: Vec<String> = (101..=114)
        .map(|id| {
            let sci_fi = matches!(id, 101..=104 | 111 | 112 | 113);
            let (genre, director) = if sci_fi { (878, 900) } else { (18, 901) };
            let original = if id >= 113 { "Twin".to_string() } else { format!("Movie {id}") };
            format!(
                r#"{{"tmdb_id":{id},"title":"Filme {id}","original_title":"{original}","year":2000,"poster_path":null,"imdb_id":"tt{:07}","genre_ids":[{genre}],"directors":[{{"id":{director},"name":"X"}}],"cast":[],"original_language":"en"}}"#,
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
/// tastes over the MovieLens movies and three releases MovieLens lacks, all sci-fi. The MovieLens
/// movies and 111 join through their Wikidata slugs (`SLUGS`); 112 through its title and year
/// alone; "Twin" matches two catálogo movies, so it joins neither. Every user also logged a
/// movie without rating it. Years are floats, as in the published sample.
fn letterboxd_csv() -> String {
    let mut csv = String::from("username,film_slug,lb_film_id,film_name,film_year,rating,liked\n");
    for user in 1..=20u32 {
        let likes_sci_fi = user % 2 == 0;
        let row = |slug: &str, name: &str, rating: f32| format!("user{user},{slug},1,\"{name}\",2000.0,{rating},False\n");
        for movie in 1..=8u32 {
            let rating = if likes_sci_fi == (movie <= 4) { 4.5 } else { 1.0 };
            csv += &row(&format!("lb-{}", 100 + movie), "Unrelated, name", rating);
        }
        let sci_fi = if likes_sci_fi { 4.5 } else { 1.0 };
        csv += &row("lb-111", "Unrelated, name", sci_fi);
        csv += &row("not-in-wikidata", "MOVIE: 112!", sci_fi);
        csv += &row("twin", "Twin", sci_fi);
        csv += &format!("user{user},lb-103,1,Seen,2000,,true\n");
    }
    csv
}

/// Letterboxd slug → TMDB id, as the Model workflow fetches it from Wikidata.
const SLUGS: &str = "slug,tmdb\nlb-101,101\nlb-102,102\nlb-103,103\nlb-104,104\n\
    lb-105,105\nlb-106,106\nlb-107,107\nlb-108,108\nlb-111,111\nbroken,http://www.wikidata.org/x\n";

fn with_letterboxd() -> Core {
    let letterboxd = Letterboxd {
        interactions: Box::new(std::io::Cursor::new(letterboxd_csv())),
        slugs: read_slugs(SLUGS.as_bytes()).unwrap(),
        titles: catalog_titles(&catalog()).unwrap(),
    };
    let model = train(ratings_csv().as_bytes(), LINKS.as_bytes(), Some(letterboxd), &[], "2026-10-25").unwrap();
    let mut core = Core::open(&catalog()).unwrap();
    core.load_model(&serde_json::to_vec(&model).unwrap()).unwrap();
    core
}

#[test]
fn letterboxd_joins_through_the_wikidata_slugs() {
    let mut core = with_letterboxd();
    core.rate(101, 5.0, 100).unwrap();
    core.rate(105, 1.0, 100).unwrap();

    let prevista = |id| core.movie(id).unwrap().card.nota_prevista.unwrap();
    assert!(prevista(111) > prevista(106) + 1.0);
}

#[test]
fn letterboxd_joins_through_title_and_year_only_when_one_movie_matches() {
    let mut core = with_letterboxd();
    core.rate(101, 5.0, 100).unwrap();
    core.rate(105, 1.0, 100).unwrap();

    assert!(core.movie(112).unwrap().card.nota_prevista.unwrap() > core.movie(106).unwrap().card.nota_prevista.unwrap() + 1.0);
    assert_eq!(core.movie(113).unwrap().card.nota_prevista, None);
    assert_eq!(core.movie(114).unwrap().card.nota_prevista, None);
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
    let bad = "username,film_slug,lb_film_id,film_name,film_year,rating,liked\nu,lb-101,1,n,2000,abc,false\n";
    let letterboxd = Letterboxd {
        interactions: Box::new(bad.as_bytes()),
        slugs: read_slugs(SLUGS.as_bytes()).unwrap(),
        titles: Default::default(),
    };
    assert!(train(ratings_csv().as_bytes(), LINKS.as_bytes(), Some(letterboxd), &[], "x").is_err());
}

/// Genome tags 1 and 2 feed fotografia; tag 3 feeds no aspect; 4–9 one tag of every other aspect.
/// Only the sci-fi movies are in the genome, 103 and 104 better filmed than 101.
const GENOME_TAGS: &str = "tagId,tag\n1,cinematography\n2,beautifully filmed\n3,\"zombies, lots of\"\n\
    4,dark\n5,great acting\n6,storytelling\n7,great soundtrack\n8,fast paced\n9,great dialogue\n";
const GENOME_SCORES: &str = "movieId,tagId,relevance\n\
    1,1,0.5\n1,2,0.5\n1,3,0.1\n2,1,0.2\n2,2,0.2\n3,1,0.9\n3,2,0.9\n4,1,0.8\n4,2,0.7\n";

#[test]
fn the_genome_subset_steers_parecidos_mas() {
    let content = content_features(&catalog()).unwrap();
    let mut model = train(ratings_csv().as_bytes(), LINKS.as_bytes(), None, &content, "2026-10-25").unwrap();
    add_genome(&mut model, GENOME_TAGS.as_bytes(), GENOME_SCORES.as_bytes(), LINKS.as_bytes()).unwrap();
    let mut core = Core::open(&catalog()).unwrap();
    core.load_model(&serde_json::to_vec(&model).unwrap()).unwrap();

    let mut better: Vec<u32> = core.parecidos_mas(101, Direcao::MelhorFotografia).iter().map(|m| m.tmdb_id).collect();
    better.sort();
    assert_eq!(better, [103, 104]);
}

#[test]
fn malformed_genome_fails_the_job() {
    let mut model = train(ratings_csv().as_bytes(), LINKS.as_bytes(), None, &[], "x").unwrap();
    let bad = "movieId,tagId,relevance\n1,1,abc\n";
    assert!(add_genome(&mut model, GENOME_TAGS.as_bytes(), bad.as_bytes(), LINKS.as_bytes()).is_err());
    let renamed = GENOME_TAGS.replace("great dialogue", "dialogues");
    assert!(add_genome(&mut model, renamed.as_bytes(), GENOME_SCORES.as_bytes(), LINKS.as_bytes()).is_err());
}
