//! Speed budgets from design §8, measured at the core facade.
//! `cargo bench -p cinelens-core` exits non-zero when any budget is exceeded.

use std::time::{Duration, Instant};

use cinelens_core::{Core, Filtro};

/// Realistic catálogo size for v1 (feature films above a vote threshold).
const CATALOG_SIZE: usize = 60_000;
/// ml-latest movies with enough ratings to be published.
const MODEL_SIZE: usize = 40_000;
const FACTORS: usize = 32;
/// About the owner's MovieLens export.
const NOTAS: u32 = 300;

fn main() {
    let catalog = synthetic_catalog(CATALOG_SIZE);
    let model = synthetic_model(MODEL_SIZE);
    let mut failed = false;

    failed |= !budget("startup: load catálogo and model", Duration::from_secs(1), || {
        Core::open(&catalog).unwrap().load_model(&model).unwrap();
    });

    let mut core = Core::open(&catalog).unwrap();
    core.load_model(&model).unwrap();
    for id in 0..NOTAS {
        core.rate(id * 97, 0.5 + (id % 10) as f32 / 2.0, 0).unwrap();
    }
    let mut at = 1;
    failed |= !budget("nota: fold-in and ranking", Duration::from_millis(200), || {
        at += 1;
        core.rate(12_345, 4.0, at).unwrap();
        assert!(core.explorar(&Filtro::default(), 0).movies[0].nota_prevista.is_some());
    });

    // Every movie passes every filter, so all of them are checked and every recomendação re-sorted.
    let filtro = Filtro {
        incluir: vec![18, 35],
        excluir: vec![99],
        ano_min: Some(1900),
        duracao_max: Some(180),
        busca: "filme número".into(),
        garimpo: -0.7,
        ..Default::default()
    };
    failed |= !budget("filtro: filter change to updated page", Duration::from_millis(100), || {
        assert_eq!(core.explorar(&filtro, 0).movies.len(), 30);
    });

    let mut id = 0;
    failed |= !budget("movie page: open", Duration::from_millis(100), || {
        id += 1_001;
        let page = core.movie(id).unwrap();
        assert_eq!(page.parecidos.len(), 12);
        assert!(!page.porque.is_empty());
    });

    failed |= !budget("início: destaques and every row", Duration::from_millis(100), || {
        assert!(!core.inicio(0).destaques.is_empty());
    });

    failed |= !budget("rodada: next movies after a nota", Duration::from_millis(100), || {
        assert_eq!(core.rodada(2).len(), 2);
    });

    failed |= !budget("perfil: every statistic", Duration::from_millis(100), || {
        assert!(core.estatisticas().acerto.is_some());
    });

    if failed {
        std::process::exit(1);
    }
}

/// Runs `f` several times and checks the median against `limit`.
fn budget(name: &str, limit: Duration, mut f: impl FnMut()) -> bool {
    let mut runs: Vec<Duration> = (0..9)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .collect();
    runs.sort();
    let median = runs[runs.len() / 2];
    let ok = median <= limit;
    println!("{} {name}: median {median:?} (budget {limit:?})", if ok { "ok  " } else { "FAIL" });
    ok
}

/// Factors in the training job's output format, for catálogo ids `0..n`.
fn synthetic_model(n: usize) -> Vec<u8> {
    let mut seed = 1u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1 << 24) as f32 - 0.5
    };
    let movies: Vec<String> = (0..n)
        .map(|i| {
            let factors: Vec<String> = (0..FACTORS).map(|_| next().to_string()).collect();
            format!(r#"{{"tmdb_id":{i},"bias":{},"factors":[{}],"ratings":100}}"#, next(), factors.join(","))
        })
        .collect();
    format!(
        r#"{{"format_version":2,"data_version":"bench","global_mean":3.5,"lambda":0.05,"movies":[{}],"content":[]}}"#,
        movies.join(",")
    )
    .into_bytes()
}

/// Movies shaped like the catalog job's output, so startup also pays for the fields
/// the core skips.
fn synthetic_catalog(n: usize) -> Vec<u8> {
    let person = |id: usize| format!(r#"{{"id":{id},"name":"Pessoa Com Nome {id}"}}"#);
    let movies: Vec<String> = (0..n)
        .map(|i| {
            let cast: Vec<String> = (0..8).map(|k| person(i * 8 + k)).collect();
            format!(
                r#"{{"tmdb_id":{i},"work_type":"movie","title":"Filme número {i}","original_title":"Movie number {i}","year":{year},"release_date":"{year}-05-17","runtime":112,"genre_ids":[18,35,10749],"original_language":"en","countries":["US","GB"],"overview":"{overview}","poster_path":"/abcdefghijklmnopqrstuvwxyz{i}.jpg","backdrop_path":"/zyxwvutsrqponmlkjihgfedcba{i}.jpg","imdb_id":"tt{i:07}","vote_count":{i},"vote_average":6.9,"popularity":12.3,"directors":[{director}],"composers":[{composer}],"cast":[{cast}],"trailer_youtube":"dQw4w9WgXcQ"}}"#,
                year = 1920 + i % 106,
                overview = "Uma sinopse de tamanho típico, com algumas frases sobre a história. ".repeat(4),
                director = person(1_000_000 + i),
                composer = person(2_000_000 + i),
                cast = cast.join(","),
            )
        })
        .collect();
    format!(
        r#"{{"format_version":1,"data_version":"bench","genres":[],"trending":[],"movies":[{}]}}"#,
        movies.join(",")
    )
    .into_bytes()
}
