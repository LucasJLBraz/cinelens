//! Perfil's statistics, on a fixture history.

use cinelens_core::Core;

/// Genres: 18 Drama, 35 Comédia, 878 Ficção científica. Movie 4 has too few votes for its
/// public score to count.
const CATALOG: &str = r#"{"format_version": 1, "data_version": "t",
    "genres": [{"id": 18, "name": "Drama"}, {"id": 35, "name": "Comédia"}, {"id": 878, "name": "Ficção científica"}],
    "movies": [
    {"tmdb_id": 1, "title": "Um", "original_title": "A", "year": 1994, "genre_ids": [18], "directors": [{"id": 10, "name": "Diretora Um"}], "vote_count": 5000, "vote_average": 8.0, "poster_path": null},
    {"tmdb_id": 2, "title": "Dois", "original_title": "B", "year": 1999, "genre_ids": [18, 35], "directors": [{"id": 10, "name": "Diretora Um"}], "vote_count": 5000, "vote_average": 6.0, "poster_path": null},
    {"tmdb_id": 3, "title": "Três", "original_title": "C", "year": 2005, "genre_ids": [35], "directors": [{"id": 20, "name": "Diretor Dois"}], "vote_count": 5000, "vote_average": 7.0, "poster_path": null},
    {"tmdb_id": 4, "title": "Quatro", "original_title": "D", "year": 2012, "genre_ids": [878], "directors": [{"id": 20, "name": "Diretor Dois"}], "vote_count": 20, "vote_average": 9.0, "poster_path": null}
]}"#;

/// 2025-01-15 and 2026-03-10, noon UTC.
const JAN_2025: i64 = 1_736_942_400;
const MAR_2026: i64 = 1_773_144_000;

/// Movie 1 went from 3 to 5; movie 999 is outside the catálogo. Notas given in the app carry
/// the nota prevista revealed when they were given.
fn core() -> Core {
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    let blob = format!(
        r#"{{"notas": {{
        "1": [{{"nota": 3.0, "at": {JAN_2025}, "movielens_pred": null}}, {{"nota": 5.0, "at": {MAR_2026}, "movielens_pred": null}}],
        "2": [{{"nota": 4.0, "at": {JAN_2025}, "movielens_pred": null, "prevista": 3.5}}],
        "3": [{{"nota": 1.0, "at": {MAR_2026}, "movielens_pred": null, "prevista": 2.0}}],
        "4": [{{"nota": 4.5, "at": {MAR_2026}, "movielens_pred": null}}],
        "999": [{{"nota": 2.0, "at": {JAN_2025}, "movielens_pred": 3.0}}]
    }}, "quero_assistir": []}}"#
    );
    core.load_owner_data(blob.as_bytes()).unwrap();
    core
}

fn grupos(grupos: &[cinelens_core::Grupo]) -> Vec<(&str, usize, f32)> {
    grupos.iter().map(|g| (g.nome.as_str(), g.notas, g.media)).collect()
}

#[test]
fn distribution_counts_current_notas_by_half_star() {
    // 1.0, 2.0, 4.0, 4.5, 5.0; movie 1's old 3.0 no longer counts.
    assert_eq!(core().estatisticas().distribuicao, [0, 1, 0, 1, 0, 0, 0, 1, 1, 1]);
}

#[test]
fn notas_by_genre_decade_and_director() {
    let e = core().estatisticas();
    // Most notas first, then the higher mean.
    assert_eq!(grupos(&e.por_genero), [("Drama", 2, 4.5), ("Comédia", 2, 2.5), ("Ficção científica", 1, 4.5)]);
    assert_eq!(grupos(&e.por_decada), [("1990", 2, 4.5), ("2000", 1, 1.0), ("2010", 1, 4.5)]);
    assert_eq!(grupos(&e.por_direcao), [("Diretora Um", 2, 4.5), ("Diretor Dois", 2, 2.75)]);
}

#[test]
fn movies_rated_per_month_with_the_empty_months_between() {
    let por_mes = core().estatisticas().por_mes;
    // Jan 2025 to Mar 2026: movies 1, 2 and 999, then 1, 3 and 4.
    assert_eq!(por_mes.len(), 15);
    assert_eq!(por_mes[0], ("2025-01".to_string(), 3));
    assert!(por_mes[1..14].iter().all(|(_, n)| *n == 0), "{por_mes:?}");
    assert_eq!(por_mes[14], ("2026-03".to_string(), 3));
}

#[test]
fn harsher_or_more_generous_than_the_public() {
    // Movies 1–3 (4 has too few votes): the owner's 5, 4, 1 against the public's 4, 3, 3.5.
    let vs = core().estatisticas().vs_publico.unwrap();
    assert!((vs - (-1.0 / 6.0)).abs() < 1e-5, "{vs}");
}

#[test]
fn biggest_disagreements_with_the_public_first() {
    let e = core().estatisticas();
    let d: Vec<(u32, f32, f32)> = e.discordancias.iter().map(|d| (d.card.tmdb_id, d.nota, d.publico)).collect();
    assert_eq!(d, [(3, 1.0, 3.5), (1, 5.0, 4.0), (2, 4.0, 3.0)]);
}

#[test]
fn taste_over_time_from_the_nota_history() {
    let e = core().estatisticas();
    // Every nota given counts in its year, movie 1's old 3.0 in 2025 too.
    let anos: Vec<(i64, usize, f32)> = e.por_ano.iter().map(|a| (a.ano, a.notas, a.media)).collect();
    assert_eq!(anos, [(2025, 3, 3.0), (2026, 3, 3.5)]);
    let mudancas: Vec<(u32, f32, f32)> = e.mudancas.iter().map(|m| (m.card.tmdb_id, m.de, m.para)).collect();
    assert_eq!(mudancas, [(1, 3.0, 5.0)]);
}

#[test]
fn accuracy_compares_each_nota_with_the_prediction_revealed_then() {
    // Movie 2 predicted 3.5, given 4; movie 3 predicted 2, given 1.
    let acerto = core().estatisticas().acerto.unwrap();
    assert_eq!((acerto.notas, acerto.mae), (2, 0.75));
}

#[test]
fn accuracy_keeps_the_prediction_stored_at_rating_time() {
    let model = |bias: f32| {
        format!(r#"{{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05, "content": [],
            "movies": [{{"tmdb_id": 1, "bias": {bias}, "factors": [0.0], "ratings": 50000}}]}}"#)
    };
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    assert!(core.estatisticas().acerto.is_none(), "no notas given in the app");
    core.load_model(model(0.5).as_bytes()).unwrap();
    let prevista = core.movie(1).unwrap().card.nota_prevista.unwrap();
    core.rate(1, 5.0, MAR_2026).unwrap();
    // A new model predicts differently; what was revealed then still stands.
    core.load_model(model(-1.0).as_bytes()).unwrap();

    let acerto = core.estatisticas().acerto.unwrap();
    assert_eq!((acerto.notas, acerto.mae), (1, 5.0 - prevista));
}

#[test]
fn a_nota_out_of_range_in_the_owner_data_is_left_out() {
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    let blob = br#"{"notas": {"1": [{"nota": 0.0, "at": 0, "movielens_pred": null}],
        "2": [{"nota": 4.0, "at": 0, "movielens_pred": null}]}, "quero_assistir": []}"#;
    core.load_owner_data(blob).unwrap();
    assert_eq!(core.estatisticas().distribuicao, [0, 0, 0, 0, 0, 0, 0, 1, 0, 0]);
}

#[test]
fn minhas_notas_lists_the_most_recently_rated_first() {
    let mut core = Core::open(CATALOG.as_bytes()).unwrap();
    core.rate(1, 4.0, JAN_2025).unwrap();
    core.rate(2, 3.0, JAN_2025 + 60).unwrap();
    core.rate(999, 2.0, JAN_2025 + 90).unwrap();
    // Changing movie 1's nota makes it the most recent again.
    core.rate(1, 5.0, MAR_2026).unwrap();
    let notas: Vec<(u32, f32, i64)> = core.minhas_notas().iter().map(|n| (n.card.tmdb_id, n.nota, n.at)).collect();
    // Movie 999 is outside the catálogo: nothing to show or open.
    assert_eq!(notas, [(1, 5.0, MAR_2026), (2, 3.0, JAN_2025 + 60)]);
}
