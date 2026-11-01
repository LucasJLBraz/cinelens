//! Séries come from their own asset and live beside filmes in the same catálogo.

use cinelens_core::{Aspecto, Core, Filtro, ObraId, Tipo};

/// A filme and a série share TMDB number 7.
const FILMES: &str = r#"{"format_version": 1, "data_version": "f1", "genres": [{"id": 878, "name": "Ficção científica"}], "trending": [], "providers": [{"id": 8, "name": "Netflix", "logo_path": null}], "movies": [
    {"tmdb_id": 7, "title": "O Filme", "original_title": "The Movie", "year": 2000, "poster_path": null, "vote_count": 900, "genre_ids": [878], "watch": {"link": null, "flatrate": [8]}},
    {"tmdb_id": 8, "title": "Outro Filme", "original_title": "Other Movie", "year": 2001, "poster_path": null, "vote_count": 100, "genre_ids": [878]}
]}"#;

const SERIES: &str = r#"{"format_version": 1, "data_version": "s1", "genres": [], "trending": [2147483655], "providers": [{"id": 9, "name": "Max", "logo_path": null}], "movies": [
    {"tmdb_id": 2147483655, "work_type": "tv", "title": "A Série", "original_title": "The Show", "year": 2010, "poster_path": null, "vote_count": 500, "runtime": 45, "genre_ids": [878], "watch": {"link": null, "flatrate": [9]}}
]}"#;

fn core() -> Core {
    let mut core = Core::open(FILMES.as_bytes()).unwrap();
    core.load_series(SERIES.as_bytes()).unwrap();
    core
}

fn ids(core: &Core, filtro: &Filtro) -> Vec<ObraId> {
    core.explorar(filtro, 0).movies.iter().map(|m| m.id).collect()
}

const FILME: ObraId = ObraId::filme(7);
const SERIE: ObraId = ObraId::serie(7);

#[test]
fn a_filme_and_a_serie_with_the_same_number_are_two_obras() {
    let core = core();
    assert_eq!(core.movie(FILME).unwrap().card.title, "O Filme");
    assert_eq!(core.movie(SERIE).unwrap().card.title, "A Série");
}

#[test]
fn tudo_shows_both_types_most_voted_first() {
    assert_eq!(ids(&core(), &Filtro::default()), [FILME, SERIE, ObraId::filme(8)]);
}

#[test]
fn the_tipo_filter_narrows_to_one_type() {
    let core = core();
    let tipo = |t| Filtro { tipo: Some(t), ..Default::default() };
    assert_eq!(ids(&core, &tipo(Tipo::Serie)), [SERIE]);
    assert_eq!(ids(&core, &tipo(Tipo::Filme)), [FILME, ObraId::filme(8)]);
}

#[test]
fn search_finds_a_serie_by_portuguese_and_original_title() {
    let core = core();
    for busca in ["a serie", "the show"] {
        let filtro = Filtro { busca: busca.into(), ..Default::default() };
        assert_eq!(ids(&core, &filtro), [SERIE], "{busca}");
    }
}

#[test]
fn the_duration_filter_reads_episode_length_on_a_serie() {
    let filtro = Filtro { tipo: Some(Tipo::Serie), duracao_max: Some(60), ..Default::default() };
    assert_eq!(ids(&core(), &filtro), [SERIE]);
}

#[test]
fn a_new_catalogo_of_either_type_leaves_the_other_in_place() {
    let mut core = core();
    core.load_catalog(FILMES.as_bytes()).unwrap();
    assert!(core.movie(SERIE).is_some());
    core.load_series(SERIES.as_bytes()).unwrap();
    assert!(core.movie(FILME).is_some());
    assert_eq!(ids(&core, &Filtro::default()).len(), 3);
    assert_eq!(core.data_version(), Some("f1"));
    assert_eq!(core.series_version(), Some("s1"));
}

#[test]
fn services_and_trending_come_from_both_assets() {
    let core = core();
    let nomes: Vec<&str> = core.provedores().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(nomes, ["Max", "Netflix"]);
    let filtro = Filtro { em_alta: true, ..Default::default() };
    assert_eq!(ids(&core, &filtro), [SERIE]);
}

#[test]
fn an_asset_with_the_wrong_type_of_obra_is_refused() {
    let mut core = core();
    assert!(core.load_series(FILMES.as_bytes()).is_err());
    assert!(core.load_catalog(SERIES.as_bytes()).is_err());
    assert_eq!(ids(&core, &Filtro::default()).len(), 3);
}

#[test]
fn a_launch_loads_the_saved_series_too() {
    let mut core = Core::default();
    core.load_saved(Some(FILMES.as_bytes()), Some(SERIES.as_bytes()), None, None, None);
    assert_eq!(ids(&core, &Filtro::default()).len(), 3);
}

#[test]
fn invalid_saved_series_leave_the_filmes() {
    let mut core = Core::default();
    core.load_saved(Some(FILMES.as_bytes()), Some(b"nope"), None, None, None);
    assert_eq!(ids(&core, &Filtro::default()).len(), 2);
    assert_eq!(core.series_version(), None);
}

mod previsoes {
    use cinelens_core::{Core, Faixa, Filtro, Marca, Mostrar, ObraId, Reacao, Tipo};

    /// Filmes 1 (sci-fi by 900) and 4 (drama by 901) are in the model. Séries: 1 is sci-fi by
    /// creator 900, 2 drama by creator 901, 3 shares nothing with the model, 4 sci-fi with an
    /// unmapped creator.
    const FILMES: &str = r#"{"format_version": 1, "data_version": "f", "movies": [
        {"tmdb_id": 1, "title": "A", "original_title": "A", "year": 1999, "poster_path": null, "genre_ids": [878], "directors": [{"id": 900, "name": "X"}], "original_language": "en"},
        {"tmdb_id": 4, "title": "D", "original_title": "D", "year": 1999, "poster_path": null, "genre_ids": [18], "directors": [{"id": 901, "name": "Y"}], "original_language": "en"},
        {"tmdb_id": 7, "title": "G", "original_title": "G", "year": 2026, "poster_path": null, "genre_ids": [878], "directors": [{"id": 900, "name": "X"}], "original_language": "en"}
    ]}"#;

    const SERIES: &str = r#"{"format_version": 1, "data_version": "s", "movies": [
        {"tmdb_id": 2147483649, "title": "S1", "original_title": "S1", "year": 2020, "poster_path": null, "genre_ids": [878], "directors": [{"id": 900, "name": "X"}], "original_language": "en"},
        {"tmdb_id": 2147483650, "title": "S2", "original_title": "S2", "year": 2020, "poster_path": null, "genre_ids": [18], "directors": [{"id": 901, "name": "Y"}], "original_language": "en"},
        {"tmdb_id": 2147483651, "title": "S3", "original_title": "S3", "year": 2020, "poster_path": null, "genre_ids": [99], "directors": [{"id": 902, "name": "Z"}], "original_language": "xx"},
        {"tmdb_id": 2147483652, "title": "S4", "original_title": "S4", "year": 2020, "poster_path": null, "genre_ids": [878], "directors": [{"id": 903, "name": "W"}], "original_language": "xx"}
    ]}"#;

    const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05,
        "movies": [
            {"tmdb_id": 1, "bias": 0.3, "factors": [1.0, 0.0], "ratings": 50000},
            {"tmdb_id": 4, "bias": 0.3, "factors": [0.0, 1.0], "ratings": 40000}
        ],
        "content": [
            {"feature": "g878", "bias": 0.2, "factors": [0.9, 0.0]},
            {"feature": "d900", "bias": 0.2, "factors": [1.0, 0.0]},
            {"feature": "g18", "bias": 0.2, "factors": [0.0, 0.9]},
            {"feature": "d901", "bias": 0.2, "factors": [0.0, 1.0]}
        ]
    }"#;

    fn with_series() -> Core {
        let mut core = Core::default();
        core.load_saved(Some(FILMES.as_bytes()), Some(SERIES.as_bytes()), None, Some(MODEL.as_bytes()), None);
        core.rate(ObraId::filme(1), 5.0, 100).unwrap();
        core.rate(ObraId::filme(4), 1.0, 100).unwrap();
        core
    }

    fn prevista(core: &Core, id: ObraId) -> Option<f32> {
        core.movie(id).unwrap().card.nota_prevista
    }

    #[test]
    fn a_nota_on_a_filme_predicts_a_serie_through_genres_and_creators() {
        let core = with_series();
        let (scifi, drama) = (prevista(&core, ObraId::serie(1)).unwrap(), prevista(&core, ObraId::serie(2)).unwrap());
        assert!(scifi > drama + 1.0, "{scifi} vs {drama}");
    }

    #[test]
    fn a_creator_the_owner_likes_raises_the_serie() {
        let core = with_series();
        let (com_criador, sem) = (prevista(&core, ObraId::serie(1)).unwrap(), prevista(&core, ObraId::serie(4)).unwrap());
        assert!(com_criador > sem, "{com_criador} vs {sem}");
    }

    #[test]
    fn a_serie_has_content_level_confianca() {
        let core = with_series();
        let confianca = core.movie(ObraId::serie(1)).unwrap().card.confianca.unwrap();
        assert!(confianca > 0.0 && confianca <= 0.3, "{confianca}");
    }

    #[test]
    fn a_serie_with_unknown_content_has_no_nota_prevista() {
        assert_eq!(prevista(&with_series(), ObraId::serie(3)), None);
    }

    #[test]
    fn faixa_on_a_serie_is_set_among_series_and_stars_stay_raw() {
        let core = with_series();
        // Filme 7 has the same content, so the same raw number.
        let best = core.movie(ObraId::serie(1)).unwrap();
        assert_eq!(best.card.nota_prevista, prevista(&core, ObraId::filme(7)));
        assert_eq!(best.faixa, Some(Faixa::ApostaForte));
        assert_eq!(core.movie(ObraId::serie(2)).unwrap().faixa, Some(Faixa::Arriscado));
    }

    #[test]
    fn the_faixa_filter_compares_a_serie_among_series() {
        let core = with_series();
        let filtro = Filtro { tipo: Some(Tipo::Serie), faixa_min: Some(Faixa::ApostaForte), ..Default::default() };
        let ids: Vec<ObraId> = core.explorar(&filtro, 0).movies.iter().map(|m| m.id).collect();
        assert_eq!(ids, [ObraId::serie(1)]);
    }

    /// A filme: id, nota prevista, the same with its nota left out, faixa.
    type Visto = (u32, Option<f32>, Option<f32>, Option<Faixa>);

    fn filmes_seen(core: &Core) -> Vec<Visto> {
        [1, 4, 7]
            .map(|id| {
                let page = core.movie(ObraId::filme(id)).unwrap();
                (id, page.card.nota_prevista, page.prevista_sem_a_nota, page.faixa)
            })
            .to_vec()
    }

    #[test]
    fn predictions_for_filmes_are_the_same_with_and_without_series() {
        let mut sem = Core::default();
        sem.load_saved(Some(FILMES.as_bytes()), None, None, Some(MODEL.as_bytes()), None);
        sem.rate(ObraId::filme(1), 5.0, 100).unwrap();
        sem.rate(ObraId::filme(4), 1.0, 100).unwrap();
        assert_eq!(filmes_seen(&with_series()), filmes_seen(&sem));
    }

    #[test]
    fn a_nota_or_a_mark_on_a_serie_changes_no_prediction_for_a_filme() {
        let mut core = with_series();
        let before = filmes_seen(&core);
        core.rate(ObraId::serie(1), 0.5, 200).unwrap();
        core.marcar(ObraId::serie(2), Marca::NaoMeInteressa, true, 200);
        assert_eq!(filmes_seen(&core), before);
    }

    #[test]
    fn the_accuracy_gate_measures_only_filmes() {
        let mut core = with_series();
        let before = core.accuracy(0.5).unwrap().held_out;
        core.rate(ObraId::serie(1), 0.5, 200).unwrap();
        assert_eq!(core.accuracy(0.5).unwrap().held_out, before);
    }

    #[test]
    fn a_rated_serie_keeps_a_prediction_with_its_nota_left_out() {
        let mut core = with_series();
        core.rate(ObraId::serie(1), 1.0, 200).unwrap();
        let page = core.movie(ObraId::serie(1)).unwrap();
        assert_eq!(page.card.nota_prevista, None);
        assert!(page.prevista_sem_a_nota.is_some());
    }

    #[test]
    fn series_stay_out_of_the_listas_of_filmes_and_of_a_first_notification() {
        let core = with_series();
        let at = 1_800_000_000;
        let inicio = core.inicio(at, 0);
        let de_filmes = inicio.fileiras.iter().filter(|f| f.filtro.tipo == Some(Tipo::Filme));
        assert!(de_filmes.flat_map(|f| &f.movies).all(|m| !m.id.is_serie()));
        assert!(core.notificacao_devida(at).is_none_or(|n| !n.id.is_serie()));
        assert!(core.rodada(Tipo::Filme, 10).iter().all(|m| !m.id.is_serie()));
    }

    fn fold_in_com(notas: &[(ObraId, f32)]) -> Core {
        let mut core = with_series();
        for (i, &(id, nota)) in notas.iter().enumerate() {
            core.rate(id, nota, 200 + i as i64).unwrap();
        }
        core
    }

    #[test]
    fn a_reacao_counts_as_its_fixed_nota() {
        for (reacao, nota) in [(Reacao::NaoGostei, 2.5), (Reacao::Gostei, 4.0), (Reacao::Amei, 5.0)] {
            let mut core = with_series();
            core.reagir(ObraId::serie(1), reacao, 200).unwrap();
            let page = core.movie(ObraId::serie(1)).unwrap();
            assert_eq!(page.nota, Some(nota));
            assert_eq!(page.reacao, Some(reacao));
        }
    }

    #[test]
    fn a_reacao_and_the_same_stars_weigh_the_same() {
        let mut reagiu = with_series();
        reagiu.reagir(ObraId::serie(1), Reacao::Gostei, 200).unwrap();
        let estrelas = fold_in_com(&[(ObraId::serie(1), 4.0)]);
        assert_eq!(prevista(&reagiu, ObraId::serie(2)), prevista(&estrelas, ObraId::serie(2)));
    }

    #[test]
    fn a_nota_on_a_serie_improves_predictions_for_series() {
        let before = prevista(&with_series(), ObraId::serie(4)).unwrap();
        let after = prevista(&fold_in_com(&[(ObraId::serie(1), 5.0)]), ObraId::serie(4)).unwrap();
        assert!(after > before, "{after} vs {before}");
    }

    #[test]
    fn a_nota_on_a_serie_weighs_more_than_one_on_a_filme() {
        // Filme 1 and série 1 look the same to the model, so at equal weight 1.0 and 5.0 would
        // cancel out at 3.0 for a third obra that looks the same too.
        let filmes = r#"{"format_version": 1, "data_version": "f", "movies": [
            {"tmdb_id": 1, "title": "F", "original_title": "F", "year": 2000, "poster_path": null}
        ]}"#;
        let series = r#"{"format_version": 1, "data_version": "s", "movies": [
            {"tmdb_id": 2147483649, "title": "S1", "original_title": "S1", "year": 2020, "poster_path": null, "genre_ids": [1]},
            {"tmdb_id": 2147483650, "title": "S2", "original_title": "S2", "year": 2020, "poster_path": null, "genre_ids": [1]}
        ]}"#;
        let model = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05,
            "movies": [{"tmdb_id": 1, "bias": 0.0, "factors": [1.0], "ratings": 50000}],
            "content": [{"feature": "g1", "bias": 0.0, "factors": [1.0]}]
        }"#;
        let mut core = Core::default();
        core.load_saved(Some(filmes.as_bytes()), Some(series.as_bytes()), None, Some(model.as_bytes()), None);
        core.rate(ObraId::filme(1), 1.0, 100).unwrap();
        core.rate(ObraId::serie(1), 5.0, 100).unwrap();
        let p = prevista(&core, ObraId::serie(2)).unwrap();
        assert!(p > 3.5, "{p}");
    }

    #[test]
    fn a_nota_on_a_serie_outside_the_catalogo_teaches_nothing() {
        let before = prevista(&with_series(), ObraId::serie(1));
        let core = fold_in_com(&[(ObraId::serie(999), 0.5)]);
        assert_eq!(prevista(&core, ObraId::serie(1)), before);
    }

    #[test]
    fn a_rated_serie_is_predicted_without_its_own_nota() {
        let sem = prevista(&with_series(), ObraId::serie(1)).unwrap();
        let core = fold_in_com(&[(ObraId::serie(1), 0.5)]);
        assert_eq!(core.movie(ObraId::serie(1)).unwrap().prevista_sem_a_nota, Some(sem));
    }

    #[test]
    fn changing_a_reacao_keeps_the_previous_in_history() {
        let mut core = with_series();
        core.reagir(ObraId::serie(1), Reacao::Amei, 200).unwrap();
        core.reagir(ObraId::serie(1), Reacao::NaoGostei, 300).unwrap();
        let page = core.movie(ObraId::serie(1)).unwrap();
        let history: Vec<_> = page.history.iter().map(|e| (e.nota, e.reacao, e.at)).collect();
        assert_eq!(history, [(5.0, Some(Reacao::Amei), 200), (2.5, Some(Reacao::NaoGostei), 300)]);
        assert_eq!(page.reacao, Some(Reacao::NaoGostei));
    }

    #[test]
    fn minhas_notas_shows_the_answer_as_given() {
        let mut core = with_series();
        core.reagir(ObraId::serie(1), Reacao::Gostei, 200).unwrap();
        core.rate(ObraId::serie(2), 3.5, 300).unwrap();
        let notas = core.minhas_notas();
        let dada = |id| notas.iter().find(|n| n.card.id == id).map(|n| (n.nota, n.reacao)).unwrap();
        assert_eq!(dada(ObraId::serie(1)), (4.0, Some(Reacao::Gostei)));
        assert_eq!(dada(ObraId::serie(2)), (3.5, None));
    }

    #[test]
    fn stars_after_a_reacao_are_shown_as_stars() {
        let mut core = with_series();
        core.reagir(ObraId::serie(1), Reacao::Gostei, 200).unwrap();
        core.rate(ObraId::serie(1), 4.0, 300).unwrap();
        assert_eq!(core.movie(ObraId::serie(1)).unwrap().reacao, None);
    }

    #[test]
    fn a_reacao_reveals_the_prediction_it_was_hidden_behind() {
        let mut core = with_series();
        let antes = core.movie(ObraId::serie(1)).unwrap().card;
        let revealed = core.reagir(ObraId::serie(1), Reacao::Gostei, 200).unwrap();
        assert_eq!(revealed, antes.exibicao);
        let page = core.movie(ObraId::serie(1)).unwrap();
        assert_eq!(page.card.nota_prevista, None);
        assert_eq!(page.history[0].prevista, antes.nota_prevista);
    }

    #[test]
    fn a_filme_has_half_star_notas_only() {
        let mut core = with_series();
        assert!(core.reagir(ObraId::filme(7), Reacao::Gostei, 200).is_err());
        assert_eq!(core.movie(ObraId::filme(7)).unwrap().nota, None);
    }

    #[test]
    fn ja_vi_takes_a_serie_out_of_recomendacoes_and_teaches_nothing() {
        let mut core = with_series();
        let filtro = Filtro { tipo: Some(Tipo::Serie), ja_vi: Mostrar::Esconder, ..Default::default() };
        let before = prevista(&core, ObraId::serie(2));
        assert!(core.explorar(&filtro, 0).movies.iter().any(|m| m.id == ObraId::serie(1)));
        core.marcar(ObraId::serie(1), Marca::JaVi, true, 200);
        assert!(core.movie(ObraId::serie(1)).unwrap().ja_vi);
        assert!(!core.explorar(&filtro, 0).movies.iter().any(|m| m.id == ObraId::serie(1)));
        assert_eq!(prevista(&core, ObraId::serie(2)), before);
    }

    #[test]
    fn a_reacao_survives_saving_and_sync_data() {
        let mut core = with_series();
        core.reagir(ObraId::serie(1), Reacao::Amei, 200).unwrap();
        let mut outro = with_series();
        outro.load_owner_data(&core.owner_data()).unwrap();
        assert_eq!(outro.movie(ObraId::serie(1)).unwrap().reacao, Some(Reacao::Amei));
    }

    #[test]
    fn a_filme_is_never_explained_by_a_loved_serie() {
        let mut core = with_series();
        core.rate(ObraId::serie(1), 5.0, 200).unwrap();
        assert!(!core.movie(ObraId::filme(7)).unwrap().porque.contains(&"S1".to_string()));
    }
}

#[test]
fn o_que_pesou_on_a_serie_does_not_reorder_filmes() {
    let catalog = r#"{"format_version": 1, "data_version": "f", "movies": [
        {"tmdb_id": 10, "title": "A", "original_title": "A", "year": 2000, "poster_path": null},
        {"tmdb_id": 11, "title": "B", "original_title": "B", "year": 2000, "poster_path": null}
    ]}"#;
    let model = r#"{"format_version": 2, "data_version": "m", "global_mean": 3.5, "lambda": 0.05,
        "movies": [
            {"tmdb_id": 10, "bias": 0.5, "factors": [1.0], "ratings": 50000},
            {"tmdb_id": 11, "bias": 0.4, "factors": [1.0], "ratings": 50000}
        ],
        "content": [],
        "genome": {"aspects": ["fotografia"], "movies": [{"tmdb_id": 10, "scores": [0.1]}, {"tmdb_id": 11, "scores": [0.9]}]}
    }"#;
    let mut core = Core::open(catalog.as_bytes()).unwrap();
    core.load_model(model.as_bytes()).unwrap();
    let before = ids(&core, &Filtro::default());
    assert_eq!(before, [ObraId::filme(10), ObraId::filme(11)]);

    core.set_pesou(ObraId::serie(1), &[Aspecto::Fotografia], 1);
    assert_eq!(ids(&core, &Filtro::default()), before);

    // The same mark on a filme does reorder them, so the test can tell.
    core.set_pesou(ObraId::filme(11), &[Aspecto::Fotografia], 2);
    assert_ne!(ids(&core, &Filtro::default()), before);
}

#[test]
fn a_page_links_to_tmdb_by_type_and_number() {
    let core = core();
    assert_eq!(core.movie(FILME).unwrap().tmdb_url, "https://www.themoviedb.org/movie/7");
    assert_eq!(core.movie(SERIE).unwrap().tmdb_url, "https://www.themoviedb.org/tv/7");
}

mod ranking_misto {
    use cinelens_core::{Core, Filtro, Marca, ObraId};

    /// Filmes 1 > 2 > 3, all predicted above every série; séries 1 > 2 through content alone.
    const FILMES: &str = r#"{"format_version": 1, "data_version": "f", "movies": [
        {"tmdb_id": 1, "title": "A", "original_title": "A", "year": 1999, "poster_path": null, "vote_count": 900, "genre_ids": [878]},
        {"tmdb_id": 2, "title": "B", "original_title": "B", "year": 1999, "poster_path": null, "vote_count": 800, "genre_ids": [878]},
        {"tmdb_id": 3, "title": "C", "original_title": "C", "year": 1999, "poster_path": null, "vote_count": 700, "genre_ids": [878]}
    ]}"#;

    const SERIES: &str = r#"{"format_version": 1, "data_version": "s", "movies": [
        {"tmdb_id": 2147483649, "title": "S1", "original_title": "S1", "year": 2020, "poster_path": null, "vote_count": 10, "genre_ids": [878]},
        {"tmdb_id": 2147483650, "title": "S2", "original_title": "S2", "year": 2020, "poster_path": null, "vote_count": 5, "genre_ids": [18]}
    ]}"#;

    const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05,
        "movies": [
            {"tmdb_id": 1, "bias": 1.3, "factors": [0.0, 0.0], "ratings": 50000},
            {"tmdb_id": 2, "bias": 1.2, "factors": [0.0, 0.0], "ratings": 50000},
            {"tmdb_id": 3, "bias": 1.1, "factors": [0.0, 0.0], "ratings": 50000}
        ],
        "content": [
            {"feature": "g878", "bias": 0.2, "factors": [0.0, 0.0]},
            {"feature": "g18", "bias": 0.1, "factors": [0.0, 0.0]}
        ]
    }"#;

    const S1: ObraId = ObraId::serie(1);
    const S2: ObraId = ObraId::serie(2);

    fn core() -> Core {
        let mut core = Core::default();
        core.load_saved(Some(FILMES.as_bytes()), Some(SERIES.as_bytes()), None, Some(MODEL.as_bytes()), None);
        core
    }

    fn tudo(core: &Core, garimpo: f32) -> Vec<ObraId> {
        let filtro = Filtro { garimpo, ..Default::default() };
        core.explorar(&filtro, 0).movies.iter().map(|m| m.id).collect()
    }

    #[test]
    fn para_voce_on_tudo_compares_each_obra_within_its_type() {
        let core = core();
        let prevista = |id| core.movie(id).unwrap().card.nota_prevista.unwrap();
        assert!(prevista(S1) < prevista(ObraId::filme(3)), "the série must be predicted lower for this to show anything");
        // Tops tie at their type's first place; S2 is halfway down, ahead of the last of three.
        assert_eq!(tudo(&core, 0.0), [ObraId::filme(1), S1, ObraId::filme(2), S2, ObraId::filme(3)]);
    }

    #[test]
    fn garimpo_on_tudo_compares_within_type_too() {
        // Garimpo lifts the less voted S2 over S1, and it stays level with the best filme.
        let order = tudo(&core(), 1.0);
        assert_eq!(order[..2], [ObraId::filme(1), S2]);
    }

    #[test]
    fn a_filter_on_tudo_compares_among_what_passes_whatever_the_garimpo() {
        let core = core();
        let order = |garimpo| {
            let filtro = Filtro { votos_max: Some(800), garimpo, ..Default::default() };
            core.explorar(&filtro, 0).movies.iter().map(|m| m.id).collect::<Vec<_>>()
        };
        let esperado = [ObraId::filme(2), S1, ObraId::filme(3), S2];
        assert_eq!(order(0.0), esperado);
        assert_eq!(order(0.001), esperado);
    }

    #[test]
    fn the_destaque_mixes_filmes_and_series() {
        let inicio = core().inicio(0, 0);
        let ids: Vec<ObraId> = inicio.destaques.iter().map(|d| d.card.id).collect();
        assert_eq!(ids[..2], [ObraId::filme(1), S1]);
    }

    #[test]
    fn agora_nao_sets_aside_a_serie_in_the_destaque() {
        let mut core = core();
        core.marcar(ObraId::filme(1), Marca::AgoraNao, true, 0);
        assert_eq!(core.destaque(1).unwrap().id, S1);
        core.marcar(S1, Marca::AgoraNao, true, 0);
        assert_ne!(core.destaque(1).unwrap().id, S1);
    }

    #[test]
    fn inicio_has_rows_of_series_set_to_the_serie_filter() {
        let core = core();
        let inicio = core.inicio(0, 0);
        let row = inicio.fileiras.iter().find(|f| f.nome == "Séries para você").unwrap();
        let ids: Vec<ObraId> = row.movies.iter().map(|m| m.id).collect();
        assert_eq!(ids, [S1, S2]);
        let explorar: Vec<ObraId> = core.explorar(&row.filtro, 0).movies.iter().map(|m| m.id).collect();
        assert_eq!(explorar, ids);
    }
}

mod piso {
    use cinelens_core::{Core, Filtro, ObraId, Tipo};

    /// Five séries with votes, averaging 5 to 9: the lowest quarter is the one at 5. Série 6 has
    /// the lowest average of all but few votes.
    fn series(medias: [f32; 5]) -> String {
        let serie = |n: u32, votos: u32, media: f32| {
            format!(
                r#"{{"tmdb_id": {}, "title": "S{n}", "original_title": "S{n}", "year": 2020, "poster_path": null, "genre_ids": [18], "vote_count": {votos}, "vote_average": {media}}}"#,
                ObraId::serie(n)
            )
        };
        let mut todas: Vec<String> = medias.iter().zip(1..).map(|(&m, n)| serie(n, 600 - n * 100, m)).collect();
        todas.push(serie(6, 49, 1.0));
        format!(r#"{{"format_version": 1, "data_version": "s", "movies": [{}]}}"#, todas.join(","))
    }

    const FILMES: &str = r#"{"format_version": 1, "data_version": "f", "movies": [
        {"tmdb_id": 1, "title": "F", "original_title": "F", "year": 2000, "poster_path": null, "genre_ids": [18], "vote_count": 60, "vote_average": 8.0}
    ]}"#;

    const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05,
        "movies": [{"tmdb_id": 1, "bias": 0.0, "factors": [1.0], "ratings": 50000}],
        "content": [{"feature": "g18", "bias": 0.0, "factors": [1.0]}]}"#;

    const RUIM: ObraId = ObraId::serie(1);

    fn core_com(medias: [f32; 5]) -> Core {
        let mut core = Core::default();
        core.load_saved(Some(FILMES.as_bytes()), Some(series(medias).as_bytes()), None, Some(MODEL.as_bytes()), None);
        core
    }

    fn core() -> Core {
        core_com([5.0, 6.0, 7.0, 8.0, 9.0])
    }

    fn ids(core: &Core, filtro: &Filtro) -> Vec<ObraId> {
        core.explorar(filtro, 0).movies.iter().map(|m| m.id).collect()
    }

    #[test]
    fn a_serie_page_shows_the_nota_do_publico_from_50_votes() {
        let core = core();
        assert_eq!(core.movie(ObraId::serie(5)).unwrap().publico, Some(4.5));
        assert_eq!(core.movie(ObraId::serie(6)).unwrap().publico, None);
        // A filme still needs more votes to speak for the public.
        assert_eq!(core.movie(ObraId::filme(1)).unwrap().publico, None);
    }

    #[test]
    fn explorar_filters_by_nota_do_publico() {
        let filtro = Filtro { tipo: Some(Tipo::Serie), media_tmdb_min: Some(8.0), ..Default::default() };
        assert_eq!(ids(&core(), &filtro), [ObraId::serie(4), ObraId::serie(5)]);
    }

    #[test]
    fn below_the_floor_a_serie_is_in_no_destaque_or_row() {
        let core = core();
        let inicio = core.inicio(0, 0);
        assert!(inicio.destaques.iter().all(|d| d.card.id != RUIM));
        assert!(inicio.fileiras.iter().flat_map(|f| &f.movies).all(|m| m.id != RUIM));
        // Not even as a row's filler, nor in what its "ver tudo" opens.
        let fileira = inicio.fileiras.iter().find(|f| f.nome == "Séries para você").unwrap();
        assert!(!fileira.movies.is_empty());
        assert!(!ids(&core, &fileira.filtro).contains(&RUIM));
    }

    #[test]
    fn below_the_floor_a_serie_stays_in_search_and_after_the_recomendacoes() {
        let core = core();
        let busca = Filtro { busca: "S1".into(), ..Default::default() };
        assert_eq!(ids(&core, &busca), [RUIM]);
        // Also from a lista pronta opened in Explorar.
        let da_lista = Filtro { acima_do_piso: true, ..busca };
        assert_eq!(ids(&core, &da_lista), [RUIM]);
        let series = ids(&core, &Filtro { tipo: Some(Tipo::Serie), ..Default::default() });
        assert_eq!(series.last(), Some(&RUIM));
        assert_eq!(series.len(), 6);
    }

    #[test]
    fn the_cut_comes_from_the_catalogo() {
        // The same 5.0 is above the floor among worse séries; 3.0 is the one below it.
        let core = core_com([5.0, 3.0, 5.5, 6.0, 6.5]);
        let inicio = core.inicio(0, 0);
        let na_fileira = |id| inicio.fileiras.iter().flat_map(|f| &f.movies).any(|m| m.id == id);
        assert!(na_fileira(RUIM));
        assert!(!na_fileira(ObraId::serie(2)));
    }

    #[test]
    fn with_fewer_than_50_votes_a_serie_escapes_the_floor() {
        let core = core();
        let fileira = core.inicio(0, 0).fileiras.into_iter().find(|f| f.nome == "Séries para você").unwrap();
        assert!(fileira.movies.iter().any(|m| m.id == ObraId::serie(6)));
    }

    #[test]
    fn the_nota_do_publico_does_not_enter_the_nota_prevista() {
        let (baixa, alta) = (core_com([5.0, 6.0, 7.0, 8.0, 9.0]), core_com([9.0, 9.0, 9.0, 9.0, 9.0]));
        let prevista = |core: &Core| core.movie(RUIM).unwrap().card.nota_prevista;
        assert!(prevista(&baixa).is_some());
        assert_eq!(prevista(&baixa), prevista(&alta));
    }
}

mod assistindo {
    use cinelens_core::{Core, Filtro, Marca, Mostrar, ObraId, Reacao, SyncStorage, Tipo};

    const FILMES: &str = r#"{"format_version": 1, "data_version": "f", "movies": [
        {"tmdb_id": 1, "title": "F", "original_title": "F", "year": 2000, "poster_path": null, "genre_ids": [18], "vote_count": 900}
    ]}"#;

    const SERIES: &str = r#"{"format_version": 1, "data_version": "s", "movies": [
        {"tmdb_id": 2147483649, "title": "S1", "original_title": "S1", "year": 2020, "poster_path": null, "genre_ids": [18], "vote_count": 800},
        {"tmdb_id": 2147483650, "title": "S2", "original_title": "S2", "year": 2020, "poster_path": null, "genre_ids": [18], "vote_count": 700}
    ]}"#;

    const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05,
        "movies": [{"tmdb_id": 1, "bias": 0.0, "factors": [1.0], "ratings": 50000}],
        "content": [{"feature": "g18", "bias": 0.0, "factors": [1.0]}]}"#;

    const S1: ObraId = ObraId::serie(1);
    const S2: ObraId = ObraId::serie(2);

    fn core() -> Core {
        let mut core = Core::default();
        core.load_saved(Some(FILMES.as_bytes()), Some(SERIES.as_bytes()), None, Some(MODEL.as_bytes()), None);
        core
    }

    fn assistindo(core: &Core) -> Vec<ObraId> {
        let filtro = Filtro { assistindo: Mostrar::So, ..Default::default() };
        core.explorar(&filtro, 0).movies.iter().map(|m| m.id).collect()
    }

    fn fileira(core: &Core, nome: &str) -> Vec<ObraId> {
        let fileira = core.inicio(0, 0).fileiras.into_iter().find(|f| f.nome == nome).unwrap();
        fileira.movies.iter().map(|m| m.id).collect()
    }

    #[test]
    fn the_mark_exists_only_on_series() {
        let mut core = core();
        core.marcar(ObraId::filme(1), Marca::Assistindo, true, 100);
        assert!(!core.movie(ObraId::filme(1)).unwrap().assistindo);
        assert_eq!(assistindo(&core), []);
        core.marcar(S1, Marca::Assistindo, true, 100);
        assert!(core.movie(S1).unwrap().assistindo);
        core.marcar(S1, Marca::Assistindo, false, 200);
        assert!(!core.movie(S1).unwrap().assistindo);
    }

    #[test]
    fn it_takes_the_serie_out_of_recomendacoes() {
        let mut core = core();
        core.marcar(S1, Marca::Assistindo, true, 100);
        let inicio = core.inicio(0, 0);
        assert!(inicio.destaques.iter().all(|d| d.card.id != S1));
        assert_eq!(fileira(&core, "Séries para você"), [S2]);
        let series = Filtro { tipo: Some(Tipo::Serie), ..Default::default() };
        let ids: Vec<ObraId> = core.explorar(&series, 0).movies.iter().map(|m| m.id).collect();
        assert_eq!(ids, [S2, S1]);
    }

    #[test]
    fn it_takes_the_serie_out_of_quero_assistir() {
        let mut core = core();
        core.marcar(S1, Marca::QueroAssistir, true, 100);
        core.marcar(S1, Marca::Assistindo, true, 200);
        let page = core.movie(S1).unwrap();
        assert!(page.assistindo && !page.quero_assistir);
    }

    #[test]
    fn quero_assistir_takes_the_serie_out_of_assistindo() {
        let mut core = core();
        core.marcar(S1, Marca::Assistindo, true, 100);
        core.marcar(S1, Marca::QueroAssistir, true, 200);
        let page = core.movie(S1).unwrap();
        assert!(!page.assistindo && page.quero_assistir);
    }

    #[test]
    fn it_teaches_nothing() {
        let mut core = core();
        core.rate(ObraId::filme(1), 5.0, 50).unwrap();
        let before = core.movie(S2).unwrap().card.nota_prevista;
        core.marcar(S1, Marca::Assistindo, true, 100);
        assert_eq!(core.movie(S2).unwrap().card.nota_prevista, before);
    }

    #[test]
    fn a_reacao_does_not_clear_it() {
        let mut core = core();
        core.marcar(S1, Marca::Assistindo, true, 100);
        core.reagir(S1, Reacao::Gostei, 200).unwrap();
        let page = core.movie(S1).unwrap();
        assert!(page.assistindo);
        assert_eq!(page.reacao, Some(Reacao::Gostei));
    }

    #[test]
    fn inicio_has_a_last_row_empty_until_marked() {
        let mut core = core();
        assert_eq!(fileira(&core, "Assistindo"), []);
        core.marcar(S1, Marca::Assistindo, true, 100);
        assert_eq!(fileira(&core, "Assistindo"), [S1]);
        assert_eq!(core.inicio(0, 0).fileiras.last().unwrap().nome, "Assistindo");
    }

    #[test]
    fn explorar_filters_by_it() {
        let mut core = core();
        core.marcar(S1, Marca::Assistindo, true, 100);
        assert_eq!(assistindo(&core), [S1]);
        let sem = Filtro { assistindo: Mostrar::Esconder, tipo: Some(Tipo::Serie), ..Default::default() };
        let ids: Vec<ObraId> = core.explorar(&sem, 0).movies.iter().map(|m| m.id).collect();
        assert_eq!(ids, [S2]);
    }

    struct Nuvem(Option<Vec<u8>>);

    impl SyncStorage for Nuvem {
        fn get(&mut self) -> Result<Option<Vec<u8>>, String> {
            Ok(self.0.clone())
        }

        fn put(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.0 = Some(bytes.to_vec());
            Ok(())
        }
    }

    #[test]
    fn it_syncs_per_item_with_the_quero_assistir_it_removed() {
        let (mut celular, mut tablet, mut nuvem) = (core(), core(), Nuvem(None));
        celular.marcar(S1, Marca::QueroAssistir, true, 100);
        celular.sync(&mut nuvem).unwrap();
        tablet.sync(&mut nuvem).unwrap();
        assert!(tablet.movie(S1).unwrap().quero_assistir);

        tablet.marcar(S1, Marca::Assistindo, true, 200);
        tablet.sync(&mut nuvem).unwrap();
        celular.sync(&mut nuvem).unwrap();
        let page = celular.movie(S1).unwrap();
        assert!(page.assistindo && !page.quero_assistir);

        celular.marcar(S1, Marca::Assistindo, false, 300);
        celular.sync(&mut nuvem).unwrap();
        tablet.sync(&mut nuvem).unwrap();
        assert!(!tablet.movie(S1).unwrap().assistindo);
    }
}

mod pagina {
    use cinelens_core::{Core, Direcao, Filtro, ObraId, Tipo};

    /// Séries 1 and 2 are sci-fi, 3 is drama, 4 is a sci-fi with too little known to predict;
    /// creator 900 made série 1 and filme 1.
    const FILMES: &str = r#"{"format_version": 1, "data_version": "f", "movies": [
        {"tmdb_id": 1, "title": "F1", "original_title": "F1", "year": 1999, "poster_path": null, "genre_ids": [878], "directors": [{"id": 900, "name": "X"}]},
        {"tmdb_id": 2, "title": "F2", "original_title": "F2", "year": 1999, "poster_path": null, "genre_ids": [878], "vote_count": 10}
    ]}"#;

    const SERIES: &str = r#"{"format_version": 1, "data_version": "s", "movies": [
        {"tmdb_id": 2147483649, "title": "S1", "original_title": "S1", "year": 2020, "poster_path": null, "genre_ids": [878], "directors": [{"id": 900, "name": "X"}], "runtime": 45, "number_of_seasons": 3, "situacao": "terminada", "trailer_youtube": "yt", "watch": {"link": "l", "flatrate": []}},
        {"tmdb_id": 2147483650, "title": "S2", "original_title": "S2", "year": 2020, "poster_path": null, "genre_ids": [878], "situacao": "em_andamento", "vote_count": 10},
        {"tmdb_id": 2147483651, "title": "S3", "original_title": "S3", "year": 2020, "poster_path": null, "genre_ids": [18], "situacao": "em_andamento"}
    ]}"#;

    const MODEL: &str = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05,
        "movies": [
            {"tmdb_id": 1, "bias": 0.0, "factors": [1.0, 0.0], "ratings": 50000},
            {"tmdb_id": 2, "bias": 0.0, "factors": [0.9, 0.1], "ratings": 50000}
        ],
        "content": [
            {"feature": "g878", "bias": 0.0, "factors": [1.0, 0.0]},
            {"feature": "g18", "bias": 0.0, "factors": [0.0, 1.0]}
        ]
    }"#;

    fn core() -> Core {
        let mut core = Core::default();
        core.load_saved(Some(FILMES.as_bytes()), Some(SERIES.as_bytes()), None, Some(MODEL.as_bytes()), None);
        core
    }

    #[test]
    fn the_page_shows_seasons_situacao_episode_length_onde_assistir_and_trailer() {
        let page = core().movie(ObraId::serie(1)).unwrap();
        assert_eq!(page.temporadas, Some(3));
        assert_eq!(page.situacao.as_deref(), Some("terminada"));
        assert_eq!(page.runtime, Some(45));
        assert_eq!(page.trailer_youtube.as_deref(), Some("yt"));
        assert!(page.onde_assistir.is_some());
    }

    #[test]
    fn a_filme_has_no_seasons_or_situacao() {
        let page = core().movie(ObraId::filme(1)).unwrap();
        assert_eq!((page.temporadas, page.situacao), (None, None));
    }

    #[test]
    fn parecidos_on_a_serie_are_the_closest_content_and_only_series() {
        let core = core();
        let ids: Vec<ObraId> = core.parecidos(ObraId::serie(1), 0).iter().map(|m| m.id).collect();
        assert_eq!(ids, [ObraId::serie(2), ObraId::serie(3)]);
    }

    #[test]
    fn parecidos_on_a_filme_stay_filmes() {
        let core = core();
        let ids: Vec<ObraId> = core.parecidos(ObraId::filme(1), 0).iter().map(|m| m.id).collect();
        assert_eq!(ids, [ObraId::filme(2)]);
    }

    #[test]
    fn parecidos_mas_has_nothing_on_a_serie() {
        let core = core();
        for direcao in [Direcao::MaisLeve, Direcao::MaisCurto, Direcao::MaisRecente, Direcao::MelhorFotografia] {
            assert!(core.parecidos_mas(ObraId::serie(1), direcao).is_empty());
        }
    }

    #[test]
    fn sou_fa_of_a_creator_raises_their_series_and_filmes() {
        let mut core = core();
        let lugar = |core: &Core, tipo, id| {
            let filtro = Filtro { tipo: Some(tipo), ..Default::default() };
            core.explorar(&filtro, 0).movies.iter().position(|m| m.id == id).unwrap()
        };
        assert_eq!((lugar(&core, Tipo::Serie, ObraId::serie(1)), lugar(&core, Tipo::Filme, ObraId::filme(1))), (1, 1));
        core.set_fa(900, true, 0);
        assert_eq!(lugar(&core, Tipo::Serie, ObraId::serie(1)), 0);
        assert_eq!(lugar(&core, Tipo::Filme, ObraId::filme(1)), 0);
        let obras: Vec<ObraId> = core.pessoa(900).unwrap().movies.iter().map(|m| m.id).collect();
        assert!(obras.contains(&ObraId::serie(1)) && obras.contains(&ObraId::filme(1)));
    }

    #[test]
    fn explorar_filters_by_situacao_and_only_series_have_one() {
        let core = core();
        let ids = |s: &str| -> Vec<ObraId> {
            let filtro = Filtro { situacao: Some(s.into()), ..Default::default() };
            core.explorar(&filtro, 0).movies.iter().map(|m| m.id).collect()
        };
        assert_eq!(ids("terminada"), [ObraId::serie(1)]);
        assert_eq!(ids("em_andamento"), [ObraId::serie(2), ObraId::serie(3)]);
    }
}

mod listas_prontas {
    use cinelens_core::{Core, Marca, ObraId, Tipo};

    /// 2026-10-26 00:00 UTC.
    const HOJE: i64 = 1_792_972_800;

    /// Filme 1 and séries 1 and 2 are good bets, new, little seen and on Netflix; série 2 is below
    /// the floor. Everything else is a filler that predicts low.
    fn core() -> Core {
        let boa = r#""genre_ids": [1], "release_date": "2026-10-01", "watch": {"link": null, "flatrate": [8]}"#;
        let obra = |id: ObraId, resto: &str| format!(r#"{{"tmdb_id": {id}, "title": "O{id}", "original_title": "O{id}", "poster_path": null, {resto}}}"#);
        let mut filmes = vec![obra(ObraId::filme(1), &format!(r#"{boa}, "vote_count": 60"#))];
        filmes.extend((100..120).map(|id| obra(ObraId::filme(id), r#""genre_ids": [2], "vote_count": 60"#)));
        let mut series = vec![
            obra(ObraId::serie(1), &format!(r#"{boa}, "vote_count": 60, "vote_average": 8.0"#)),
            obra(ObraId::serie(2), &format!(r#"{boa}, "vote_count": 60, "vote_average": 2.0"#)),
        ];
        series.extend((100..120).map(|id| obra(ObraId::serie(id), r#""genre_ids": [2], "vote_count": 60, "vote_average": 7.0"#)));
        let artifact = |obras: Vec<String>| format!(r#"{{"format_version": 1, "data_version": "t", "movies": [{}]}}"#, obras.join(","));
        let model = r#"{"format_version": 2, "data_version": "t", "global_mean": 3.5, "lambda": 0.05, "movies": [{"tmdb_id": 999, "bias": 0.0, "factors": [0.0], "ratings": 1}],
            "content": [{"feature": "g1", "bias": 1.0, "factors": [0.0]}, {"feature": "g2", "bias": -1.0, "factors": [0.0]}]}"#;
        let mut core = Core::default();
        core.load_saved(Some(artifact(filmes).as_bytes()), Some(artifact(series).as_bytes()), None, Some(model.as_bytes()), None);
        core.set_servicos(&[8], 0);
        core
    }

    fn fileira(core: &Core, nome: &str) -> Vec<ObraId> {
        let inicio = core.inicio(HOJE, 0);
        inicio.fileiras.iter().find(|f| f.nome == nome).unwrap().movies.iter().map(|m| m.id).collect()
    }

    fn accepts_a_serie_above_the_floor(core: &Core, nome: &str) {
        let obras = fileira(core, nome);
        assert!(obras.contains(&ObraId::serie(1)), "{nome}: {obras:?}");
        assert!(obras.contains(&ObraId::filme(1)), "{nome}: {obras:?}");
        assert!(!obras.contains(&ObraId::serie(2)), "{nome}: below the floor");
    }

    #[test]
    fn no_seu_streaming_hoje_accepts_series() {
        accepts_a_serie_above_the_floor(&core(), "No seu streaming hoje");
    }

    #[test]
    fn joias_escondidas_accept_series() {
        accepts_a_serie_above_the_floor(&core(), "Joias escondidas");
    }

    #[test]
    fn lancamentos_para_voce_count_a_series_first_episode_as_its_release() {
        accepts_a_serie_above_the_floor(&core(), "Lançamentos para você");
    }

    #[test]
    fn para_rever_accepts_series_marked_ja_vi() {
        let mut core = core();
        for id in [ObraId::filme(1), ObraId::serie(1), ObraId::serie(2)] {
            core.marcar(id, Marca::JaVi, true, 0);
        }
        accepts_a_serie_above_the_floor(&core, "Para rever");
    }

    #[test]
    fn the_other_listas_prontas_stay_filmes_only() {
        let inicio = core().inicio(HOJE, 0);
        let filmes: Vec<&str> = inicio.fileiras.iter().filter(|f| f.filtro.tipo == Some(Tipo::Filme)).map(|f| f.nome.as_str()).collect();
        assert_eq!(filmes, ["Em alta, do seu jeito", "Sessão da Tarde", "Cabe numa noite", "Clássicos que você não viu", "Aclamados pelo público"]);
    }
}
