//! Animes: a third type of obra with a model of its own (ADR 0004), Reação and marks as on séries,
//! and notas by temporada.

use std::collections::HashMap;

use cinelens_core::{Core, Filtro, Marca, ObraId, Reacao, SyncStorage, Tipo};

/// 1 and 2 are animated features MyAnimeList also knows; 3 only has content, genre 16 like the
/// animes; 4 is outside both.
const FILMES: &str = r#"{"format_version": 1, "data_version": "f", "genres": [], "movies": [
    {"tmdb_id": 1, "title": "Feature A", "original_title": "A", "year": 2001, "poster_path": null, "vote_count": 900, "genre_ids": [16]},
    {"tmdb_id": 2, "title": "Feature B", "original_title": "B", "year": 2002, "poster_path": null, "vote_count": 800, "genre_ids": [16]},
    {"tmdb_id": 3, "title": "Feature C", "original_title": "C", "year": 2003, "poster_path": null, "vote_count": 700, "genre_ids": [16]},
    {"tmdb_id": 4, "title": "Drama", "original_title": "D", "year": 2004, "poster_path": null, "vote_count": 600, "genre_ids": [18]}
]}"#;

const SERIES: &str = r#"{"format_version": 1, "data_version": "s", "genres": [], "movies": [
    {"tmdb_id": 2147483748, "work_type": "tv", "title": "A Série", "original_title": "S", "year": 2010, "poster_path": null, "vote_count": 500, "genre_ids": [18]}
]}"#;

fn id(n: u32) -> ObraId {
    ObraId::anime(n)
}

/// 10: three temporadas with entries, the last with no factors. 11: one long run. 12: no factors,
/// content only. 13: two parts of one temporada in MyAnimeList, so one entry.
fn animes() -> String {
    let temporada = |n: u32, entrada: bool| format!(r#"{{"numero": {n}, "nome": "Temporada {n}", "episodios": 12, "ano": 2020, "entrada": {entrada}}}"#);
    let obra = |n: u32, votes: u32, temporadas: &[String]| {
        format!(
            r#"{{"tmdb_id": {}, "work_type": "anime", "title": "Anime {n}", "original_title": "Original {n}", "year": 2020, "poster_path": null, "vote_count": {votes}, "vote_average": 8.0, "genre_ids": [16], "temporadas": [{}]}}"#,
            id(n),
            temporadas.join(",")
        )
    };
    let movies = [
        obra(10, 400, &[temporada(1, true), temporada(2, true), temporada(3, true)]),
        obra(11, 300, &[temporada(1, false), temporada(2, false)]),
        obra(12, 200, &[]),
        obra(13, 100, &[temporada(1, true)]),
    ];
    format!(r#"{{"format_version": 1, "data_version": "a", "genres": [], "movies": [{}]}}"#, movies.join(","))
}

const MODEL: &str = r#"{"format_version": 2, "data_version": "m", "global_mean": 3.5, "lambda": 0.05,
    "movies": [
        {"tmdb_id": 1, "bias": 0.0, "factors": [1.0, 0.0], "ratings": 1000},
        {"tmdb_id": 2, "bias": 0.0, "factors": [0.0, 1.0], "ratings": 1000},
        {"tmdb_id": 3, "bias": 0.0, "factors": [0.7, 0.7], "ratings": 1000},
        {"tmdb_id": 4, "bias": 0.0, "factors": [0.7, -0.7], "ratings": 1000}
    ],
    "content": [{"feature": "g18", "bias": 0.1, "factors": [0.5, 0.5]}]}"#;

/// 10's first temporada points one way, its second the other way.
const ANIME_MODEL: &str = r#"{"format_version": 1, "data_version": "am", "global_mean": 3.8, "lambda": 0.05,
    "movies": [{"tmdb_id": 1, "bias": 0.0, "factors": [1.0, 0.0], "ratings": 100}],
    "animes": [
        {"tmdb_id": 10, "temporada": 1, "bias": 0.1, "factors": [1.0, 0.0], "ratings": 1000},
        {"tmdb_id": 10, "temporada": 2, "bias": 0.0, "factors": [0.0, 1.0], "ratings": 500},
        {"tmdb_id": 11, "temporada": 1, "bias": 0.2, "factors": [0.9, 0.1], "ratings": 2000},
        {"tmdb_id": 13, "temporada": 1, "bias": 0.0, "factors": [0.0, 1.0], "ratings": 300}
    ],
    "content": [{"feature": "g16", "bias": 0.0, "factors": [0.3, 0.3]}]}"#;

fn core() -> Core {
    let mut core = Core::open(FILMES.as_bytes()).unwrap();
    core.load_series(SERIES.as_bytes()).unwrap();
    core.load_animes(animes().as_bytes()).unwrap();
    core.load_model(MODEL.as_bytes()).unwrap();
    core.load_anime_model(ANIME_MODEL.as_bytes()).unwrap();
    core
}

fn prevista(core: &Core, id: ObraId) -> f32 {
    core.movie(id).unwrap().card.nota_prevista.unwrap()
}

const FILME_BRIDGE: ObraId = ObraId::filme(1);
const FILME_OUTRO: ObraId = ObraId::filme(2);
const SERIE: ObraId = ObraId::serie(100);

/// Predictions for what is neither an anime nor rated in these tests.
fn outros(core: &Core) -> Vec<f32> {
    vec![prevista(core, FILME_OUTRO), prevista(core, ObraId::filme(4)), prevista(core, SERIE)]
}

mod predictions {
    use super::*;

    #[test]
    fn exist_with_no_notas_on_animes() {
        let core = core();
        assert!(core.movie(id(10)).unwrap().card.nota_prevista.is_some());
        assert!(core.movie(id(12)).unwrap().card.nota_prevista.is_some());
    }

    #[test]
    fn a_nota_on_an_anime_never_changes_a_prediction_for_a_filme_or_serie() {
        let mut core = core();
        core.rate(FILME_BRIDGE, 4.0, 100).unwrap();
        let before = outros(&core);
        core.rate(id(10), 0.5, 200).unwrap();
        core.reagir(id(11), Reacao::Amei, 300).unwrap();
        core.rate_temporada(id(10), 2, 5.0, 400).unwrap();
        core.marcar(id(13), Marca::NaoMeInteressa, true, 500);
        assert_eq!(outros(&core), before);
    }

    #[test]
    fn notas_on_series_do_not_change_a_prediction_for_an_anime() {
        let mut core = core();
        let before = prevista(&core, id(10));
        core.rate(SERIE, 0.5, 100).unwrap();
        assert_eq!(prevista(&core, id(10)), before);
    }

    #[test]
    fn a_nota_on_a_filme_moves_animes_through_a_shared_feature() {
        let mut core = core();
        let before = prevista(&core, id(11));
        core.rate(FILME_BRIDGE, 5.0, 100).unwrap();
        core.rate(FILME_OUTRO, 1.0, 100).unwrap();
        // Liked what points like 11, and disliked the opposite: 11 rises against 13.
        assert!(prevista(&core, id(11)) > prevista(&core, id(13)), "{before}");
    }

    #[test]
    fn a_nota_on_a_filme_only_read_through_content_also_counts() {
        let mut sem = core();
        let mut com = core();
        sem.rate(FILME_BRIDGE, 3.0, 100).unwrap();
        com.rate(FILME_BRIDGE, 3.0, 100).unwrap();
        com.rate(ObraId::filme(3), 5.0, 100).unwrap();
        assert!(prevista(&com, id(11)) > prevista(&sem, id(11)));
    }

    #[test]
    fn notas_on_animes_improve_predictions_for_animes() {
        let mut core = core();
        core.rate(id(11), 5.0, 100).unwrap();
        core.rate(id(13), 1.0, 100).unwrap();
        assert!(prevista(&core, id(10)) > 3.0);
        let a = core.movie(id(12)).unwrap();
        assert!(a.card.confianca.unwrap() < core.movie(id(10)).unwrap().card.confianca.unwrap());
    }

    #[test]
    fn the_whole_is_predicted_from_its_first_temporada() {
        let mut core = core();
        // Loved what points like 10's first temporada, disliked what points like its second.
        core.rate(id(11), 5.0, 100).unwrap();
        core.rate(id(13), 1.0, 100).unwrap();
        // Above the owner's mean of 3: read from the second, it would fall below.
        assert!(prevista(&core, id(10)) > 3.1, "{}", prevista(&core, id(10)));
    }

    #[test]
    fn animes_without_factors_come_from_content_at_lower_confianca() {
        let core = core();
        let (factors, content) = (core.movie(id(10)).unwrap().card, core.movie(id(12)).unwrap().card);
        assert!(content.confianca.unwrap() <= 0.3);
        assert!(factors.confianca.unwrap() > 0.3);
    }

    #[test]
    fn without_the_anime_model_animes_are_listed_unpredicted() {
        let mut core = Core::open(FILMES.as_bytes()).unwrap();
        core.load_animes(animes().as_bytes()).unwrap();
        core.load_model(MODEL.as_bytes()).unwrap();
        assert!(core.movie(id(10)).unwrap().card.nota_prevista.is_none());
        assert!(core.movie(FILME_BRIDGE).unwrap().card.nota_prevista.is_some());
    }

    #[test]
    fn faixa_is_set_among_animes() {
        let core = core();
        let filtro = Filtro { tipo: Some(Tipo::Anime), ..Default::default() };
        let first = core.explorar(&filtro, 0).movies.remove(0);
        assert_eq!(core.tipo(first.id), Tipo::Anime);
        assert_eq!(core.movie(first.id).unwrap().faixa, Some(cinelens_core::Faixa::ApostaForte));
    }

    #[test]
    fn a_rated_anime_keeps_a_prediction_with_its_nota_left_out() {
        let mut core = core();
        let before = prevista(&core, id(10));
        core.rate(id(10), 0.5, 100).unwrap();
        assert_eq!(core.movie(id(10)).unwrap().prevista_sem_a_nota, Some(before));
    }

    #[test]
    fn sou_fa_raises_an_anime() {
        let with_person = animes().replace(r#""title": "Anime 13""#, r#""cast": [{"id": 7, "name": "Seiyu"}], "title": "Anime 13""#);
        let mut core = Core::open(FILMES.as_bytes()).unwrap();
        core.load_animes(with_person.as_bytes()).unwrap();
        core.load_anime_model(ANIME_MODEL.as_bytes()).unwrap();
        let filtro = Filtro { tipo: Some(Tipo::Anime), ..Default::default() };
        core.set_fa(7, true, 100);
        assert_eq!(core.explorar(&filtro, 0).movies[0].id, id(13));
    }

    #[test]
    fn the_anime_model_refuses_another_format() {
        let mut core = core();
        assert!(core.load_anime_model(MODEL.as_bytes()).is_err());
        assert_eq!(core.anime_model_version(), Some("am"));
    }

    #[test]
    fn parecidos_of_an_anime_are_animes() {
        let core = core();
        let parecidos = core.parecidos(id(10), 0);
        assert!(!parecidos.is_empty());
        assert!(parecidos.iter().all(|c| core.tipo(c.id) == Tipo::Anime));
    }

    #[test]
    fn saved_files_load_at_once() {
        let mut core = Core::default();
        core.load_saved(Some(FILMES.as_bytes()), Some(SERIES.as_bytes()), Some(animes().as_bytes()), Some(MODEL.as_bytes()), Some(ANIME_MODEL.as_bytes()));
        assert!(core.movie(id(10)).unwrap().card.nota_prevista.is_some());
    }
}

mod reacao_and_marks {
    use super::*;

    #[test]
    fn minhas_notas_shows_the_answer_as_given() {
        let mut core = core();
        core.reagir(id(10), Reacao::Gostei, 100).unwrap();
        core.rate(id(11), 3.5, 200).unwrap();
        let notas = core.minhas_notas();
        assert_eq!((notas[0].card.id, notas[0].nota, notas[0].reacao), (id(11), 3.5, None));
        assert_eq!((notas[1].card.id, notas[1].nota, notas[1].reacao), (id(10), 4.0, Some(Reacao::Gostei)));
    }

    #[test]
    fn marks_work_and_ja_vi_assistindo_teach_nothing() {
        let mut core = core();
        let before = prevista(&core, id(10));
        for marca in [Marca::JaVi, Marca::Assistindo, Marca::QueroAssistir] {
            core.marcar(id(11), marca, true, 100);
        }
        let page = core.movie(id(11)).unwrap();
        assert!(page.ja_vi && page.quero_assistir && !page.assistindo);
        core.marcar(id(13), Marca::Assistindo, true, 100);
        assert!(core.movie(id(13)).unwrap().assistindo);
        assert_eq!(prevista(&core, id(10)), before);
    }

    #[test]
    fn nao_me_interessa_hides_an_anime_and_teaches_as_on_a_serie() {
        let mut core = core();
        let before = prevista(&core, id(10));
        core.marcar(id(13), Marca::NaoMeInteressa, true, 100);
        assert!(!core.explorar(&Filtro::default(), 0).movies.iter().any(|m| m.id == id(13)));
        assert_eq!(prevista(&core, id(10)), before);
    }

    #[test]
    fn assistindo_filter_finds_animes() {
        let mut core = core();
        core.marcar(id(13), Marca::Assistindo, true, 100);
        let filtro = Filtro { assistindo: cinelens_core::Mostrar::So, ..Default::default() };
        assert_eq!(core.explorar(&filtro, 0).movies[0].id, id(13));
    }

    #[test]
    fn marks_and_notas_sync_across_devices() {
        let mut a = core();
        a.marcar(id(10), Marca::Assistindo, true, 100);
        a.reagir(id(11), Reacao::Amei, 100).unwrap();
        a.rate_temporada(id(10), 2, 2.0, 100).unwrap();
        let mut drive = Drive(HashMap::new());
        a.sync(&mut drive).unwrap();
        let mut b = core();
        b.sync(&mut drive).unwrap();
        assert!(b.movie(id(10)).unwrap().assistindo);
        assert_eq!(b.movie(id(11)).unwrap().reacao, Some(Reacao::Amei));
        assert_eq!(b.movie(id(10)).unwrap().lista_temporadas[1].nota, Some(2.0));
    }

    #[test]
    fn removing_a_nota_on_a_temporada_syncs() {
        let mut a = core();
        a.rate_temporada(id(10), 2, 2.0, 100).unwrap();
        let mut drive = Drive(HashMap::new());
        a.sync(&mut drive).unwrap();
        let mut b = core();
        b.sync(&mut drive).unwrap();
        b.remover_nota_temporada(id(10), 2, 200);
        b.sync(&mut drive).unwrap();
        a.sync(&mut drive).unwrap();
        assert_eq!(a.movie(id(10)).unwrap().lista_temporadas[1].nota, None);
    }

    /// One blob, as Drive holds it.
    struct Drive(HashMap<(), Vec<u8>>);

    impl SyncStorage for Drive {
        fn get(&mut self) -> Result<Option<Vec<u8>>, String> {
            Ok(self.0.get(&()).cloned())
        }

        fn put(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.0.insert((), bytes.to_vec());
            Ok(())
        }
    }
}

mod temporadas {
    use super::*;

    #[test]
    fn the_page_lists_tmdbs_temporadas() {
        let t = core().movie(id(10)).unwrap().lista_temporadas;
        let numeros: Vec<u32> = t.iter().map(|t| t.numero).collect();
        assert_eq!(numeros, [1, 2, 3]);
        assert_eq!(t[0].nome, "Temporada 1");
        assert!(t.iter().all(|t| t.avaliavel));
    }

    #[test]
    fn a_nota_on_a_temporada_is_its_own_and_keeps_history() {
        let mut core = core();
        core.rate_temporada(id(10), 2, 4.0, 100).unwrap();
        core.reagir_temporada(id(10), 2, Reacao::NaoGostei, 200).unwrap();
        let page = core.movie(id(10)).unwrap();
        assert_eq!(page.nota, None);
        let t = &page.lista_temporadas[1];
        assert_eq!((t.nota, t.reacao), (Some(2.5), Some(Reacao::NaoGostei)));
        assert_eq!(t.history.iter().map(|e| e.nota).collect::<Vec<_>>(), [4.0, 2.5]);
    }

    #[test]
    fn a_nota_on_a_temporada_wins_over_the_whole_for_that_temporada() {
        // On the whole: loved. 11 points like 10's first temporada, 13 like its second.
        let mut so_o_todo = core();
        so_o_todo.rate(id(10), 5.0, 100).unwrap();
        let mut com_temporada = core();
        com_temporada.rate(id(10), 5.0, 100).unwrap();
        // The second was bad.
        com_temporada.rate_temporada(id(10), 2, 0.5, 200).unwrap();
        assert!(prevista(&com_temporada, id(13)) < prevista(&so_o_todo, id(13)));
        // The whole still stands for the first.
        assert!(prevista(&com_temporada, id(11)) >= prevista(&so_o_todo, id(11)) - 0.5);
        assert_eq!(com_temporada.movie(id(10)).unwrap().nota, Some(5.0));
    }

    #[test]
    fn a_temporada_the_dump_lacks_teaches_nothing_against_the_first() {
        let mut core = core();
        let before = prevista(&core, id(11));
        core.rate_temporada(id(10), 3, 0.5, 100).unwrap();
        assert_eq!(prevista(&core, id(11)), before);
    }

    #[test]
    fn when_every_temporada_has_a_nota_the_whole_no_longer_counts() {
        let notas = |todo: f32| {
            let mut c = core();
            c.rate(id(10), todo, 100).unwrap();
            c.rate_temporada(id(10), 1, 4.0, 200).unwrap();
            c.rate_temporada(id(10), 2, 4.0, 200).unwrap();
            prevista(&c, id(13))
        };
        assert_eq!(notas(0.5), notas(5.0));
    }

    #[test]
    fn a_nota_only_on_a_temporada_takes_the_anime_out_of_recomendacoes() {
        let mut core = core();
        core.rate_temporada(id(10), 2, 4.0, 100).unwrap();
        assert!(core.movie(id(10)).unwrap().card.nota_prevista.is_none());
    }

    #[test]
    fn a_long_run_and_a_single_entry_take_only_a_nota_on_the_whole() {
        let mut core = core();
        // One entry split into no temporadas; TMDB still lists two.
        assert!(core.movie(id(11)).unwrap().lista_temporadas.iter().all(|t| !t.avaliavel));
        assert!(core.rate_temporada(id(11), 1, 4.0, 100).is_err());
        // MyAnimeList's two parts count as one temporada.
        assert!(!core.movie(id(13)).unwrap().lista_temporadas[0].avaliavel);
        assert!(core.reagir_temporada(id(13), 1, Reacao::Amei, 100).is_err());
        assert!(core.rate(id(11), 4.0, 100).is_ok());
    }

    #[test]
    fn neither_a_serie_nor_an_unlisted_temporada_takes_one() {
        let mut core = core();
        assert!(core.rate_temporada(SERIE, 1, 4.0, 100).is_err());
        assert!(core.rate_temporada(id(10), 9, 4.0, 100).is_err());
        assert!(core.movie(SERIE).unwrap().lista_temporadas.is_empty());
    }

    #[test]
    fn an_invalid_nota_is_refused() {
        assert!(core().rate_temporada(id(10), 1, 4.2, 100).is_err());
    }

    #[test]
    fn minhas_notas_names_the_temporada() {
        let mut core = core();
        core.rate(id(10), 5.0, 100).unwrap();
        core.reagir_temporada(id(10), 2, Reacao::Gostei, 200).unwrap();
        let notas = core.minhas_notas();
        assert_eq!(notas[0].temporada.as_deref(), Some("Temporada 2"));
        assert_eq!(notas[0].reacao, Some(Reacao::Gostei));
        assert_eq!(notas[1].temporada, None);
    }

    #[test]
    fn the_rodada_only_asks_about_the_whole_of_animes_with_factors() {
        let mut core = core();
        let asked: Vec<ObraId> = core.rodada(Tipo::Anime, 9).iter().map(|c| c.id).collect();
        assert!(asked.contains(&id(10)) && !asked.contains(&id(12)));
        assert!(asked.iter().all(|&a| core.tipo(a) == Tipo::Anime));
        core.rate_temporada(id(10), 2, 4.0, 100).unwrap();
        assert!(!core.rodada(Tipo::Anime, 9).iter().any(|c| c.id == id(10)));
    }
}
