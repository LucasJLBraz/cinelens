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
    core.load_saved(Some(FILMES.as_bytes()), Some(SERIES.as_bytes()), None);
    assert_eq!(ids(&core, &Filtro::default()).len(), 3);
}

#[test]
fn invalid_saved_series_leave_the_filmes() {
    let mut core = Core::default();
    core.load_saved(Some(FILMES.as_bytes()), Some(b"nope"), None);
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
        core.load_saved(Some(FILMES.as_bytes()), Some(SERIES.as_bytes()), Some(MODEL.as_bytes()));
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
        sem.load_saved(Some(FILMES.as_bytes()), None, Some(MODEL.as_bytes()));
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
    fn series_stay_out_of_the_destaque_listas_and_notification_for_now() {
        let core = with_series();
        let at = 1_800_000_000;
        assert!(core.inicio(at, 0).destaques.iter().all(|d| !d.card.id.is_serie()));
        assert!(core.inicio(at, 0).fileiras.iter().flat_map(|f| &f.movies).all(|m| !m.id.is_serie()));
        assert!(core.notificacao_devida(at).is_none_or(|n| !n.id.is_serie()));
        assert!(core.rodada(10).iter().all(|m| !m.id.is_serie()));
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
        core.load_saved(Some(filmes.as_bytes()), Some(series.as_bytes()), Some(model.as_bytes()));
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
