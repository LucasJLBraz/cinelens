//! How the nota prevista is shown ("Mostrar previsão como"), and the reveal after a nota.

use cinelens_core::{Core, DisplayMode, Exibicao, Faixa};

const CATALOG: &[u8] = include_bytes!("../fixtures/catalog.json");
/// Sci-fi: 603, 27205, 157336, 693134, 11. Crime/drama: 238, 240, 278, 769, 424.
const MODEL: &[u8] = include_bytes!("../fixtures/model.json");

/// Loves sci-fi, dislikes drama; eight movies of the model left to predict.
fn owner() -> Core {
    let mut core = Core::open(CATALOG).unwrap();
    core.load_model(MODEL).unwrap();
    core.rate(603, 5.0, 100).unwrap();
    core.rate(278, 1.5, 100).unwrap();
    core
}

fn exibicoes(core: &Core) -> Vec<Option<Exibicao>> {
    core.explorar(0).movies.into_iter().map(|m| m.exibicao).collect()
}

#[test]
fn faixa_is_the_default_with_bands_from_the_owners_own_predictions() {
    let core = owner();
    assert_eq!(core.display_mode(), DisplayMode::Faixa);

    let faixas: Vec<Faixa> = exibicoes(&core)
        .into_iter()
        .map_while(|p| match p {
            Some(Exibicao::Faixa { faixa, .. }) => Some(faixa),
            _ => None,
        })
        .collect();
    // Of eight predictions, highest first: the top 10% is one movie, then up to 35%, 70%, rest.
    use Faixa::*;
    assert_eq!(
        faixas,
        [ApostaForte, BoaChance, BoaChance, Arriscado, Arriscado, Arriscado, ProvavelmenteNao, ProvavelmenteNao]
    );
}

#[test]
fn estrelas_shows_the_decimal_the_most_likely_half_star_and_the_confianca() {
    let mut core = owner();
    core.set_display_mode(DisplayMode::Estrelas);

    let card = core.movie(27205).unwrap().card;
    let Some(Exibicao::Estrelas { nota, meia_estrela, confianca }) = card.exibicao else {
        panic!("{:?}", card.exibicao);
    };
    assert_eq!(nota, card.nota_prevista.unwrap());
    assert_eq!(meia_estrela, (nota * 2.0).round() / 2.0);
    assert!((nota - meia_estrela).abs() <= 0.25);
    assert_eq!(Some(confianca), card.confianca);
}

#[test]
fn oculto_shows_nothing_but_keeps_the_order() {
    let mut core = owner();
    let order: Vec<u32> = core.explorar(0).movies.iter().map(|m| m.tmdb_id).collect();
    core.set_display_mode(DisplayMode::Oculto);

    assert!(exibicoes(&core).iter().all(Option::is_none));
    assert_eq!(core.explorar(0).movies.iter().map(|m| m.tmdb_id).collect::<Vec<_>>(), order);
}

#[test]
fn after_a_nota_the_app_reveals_what_it_predicted_in_the_chosen_mode() {
    let mut core = owner();
    let before = core.movie(27205).unwrap().card;

    let revealed = core.rate(27205, 4.0, 200).unwrap();

    assert_eq!(revealed, before.exibicao);
    assert!(matches!(revealed, Some(Exibicao::Faixa { .. })));
}

#[test]
fn the_prediction_at_rating_time_is_stored_with_the_nota() {
    let mut core = owner();
    let predicted = core.movie(27205).unwrap().card.nota_prevista;
    core.rate(27205, 4.0, 200).unwrap();

    let history = core.movie(27205).unwrap().history;
    assert_eq!(history.last().unwrap().prevista, predicted);

    // Re-rating: the movie had a nota, so there was nothing on screen to store.
    core.rate(27205, 4.5, 300).unwrap();
    assert_eq!(core.movie(27205).unwrap().history.last().unwrap().prevista, None);
}

#[test]
fn no_reveal_when_hidden_or_without_a_prediction() {
    let mut core = owner();
    core.set_display_mode(DisplayMode::Oculto);
    assert_eq!(core.rate(27205, 4.0, 200).unwrap(), None);

    let mut core = owner();
    // Clube da Luta is outside the model.
    assert_eq!(core.rate(550, 4.0, 200).unwrap(), None);
}

#[test]
fn display_mode_survives_a_restart() {
    let mut core = owner();
    core.set_display_mode(DisplayMode::Estrelas);

    let mut restarted = Core::open(CATALOG).unwrap();
    restarted.load_owner_data(&core.owner_data()).unwrap();
    assert_eq!(restarted.display_mode(), DisplayMode::Estrelas);
}
