//! How the nota prevista is shown ("Mostrar previsão como"), and the reveal after a nota.

use cinelens_core::{Core, Filtro, DisplayMode, Exibicao, Faixa};

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
    core.explorar(&Filtro::default(), 0).movies.into_iter().map(|m| m.exibicao).collect()
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
fn oculto_shows_nothing_but_keeps_the_order() {
    let mut core = owner();
    let order: Vec<u32> = core.explorar(&Filtro::default(), 0).movies.iter().map(|m| m.tmdb_id).collect();
    core.set_display_mode(DisplayMode::Oculto, 0);

    assert!(exibicoes(&core).iter().all(Option::is_none));
    assert_eq!(core.explorar(&Filtro::default(), 0).movies.iter().map(|m| m.tmdb_id).collect::<Vec<_>>(), order);
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
    core.set_display_mode(DisplayMode::Oculto, 0);
    assert_eq!(core.rate(27205, 4.0, 200).unwrap(), None);

    let mut core = owner();
    // Clube da Luta is outside the model.
    assert_eq!(core.rate(550, 4.0, 200).unwrap(), None);
}

#[test]
fn display_mode_survives_a_restart() {
    let mut core = owner();
    core.set_display_mode(DisplayMode::Oculto, 0);

    let mut restarted = Core::open(CATALOG).unwrap();
    restarted.load_owner_data(&core.owner_data()).unwrap();
    assert_eq!(restarted.display_mode(), DisplayMode::Oculto);
}

/// Owner data saved before stars moved behind a tap on the movie page.
fn with_estrelas(core: &Core) -> Vec<u8> {
    let data = String::from_utf8(core.owner_data()).unwrap();
    assert!(data.contains(r#""display_mode":"Oculto""#), "{data}");
    data.replace(r#""display_mode":"Oculto""#, r#""display_mode":"Estrelas""#).into_bytes()
}

#[test]
fn a_saved_estrelas_reads_as_faixa() {
    let mut core = owner();
    core.set_display_mode(DisplayMode::Oculto, 0);

    let mut restarted = Core::open(CATALOG).unwrap();
    restarted.load_owner_data(&with_estrelas(&core)).unwrap();
    assert_eq!(restarted.display_mode(), DisplayMode::Faixa);
}

#[test]
fn an_estrelas_arriving_through_sync_reads_as_faixa() {
    struct Blob(Option<Vec<u8>>);
    impl cinelens_core::SyncStorage for Blob {
        fn get(&mut self) -> Result<Option<Vec<u8>>, String> {
            Ok(self.0.clone())
        }
        fn put(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.0 = Some(bytes.to_vec());
            Ok(())
        }
    }
    let mut old_phone = owner();
    old_phone.set_display_mode(DisplayMode::Oculto, 50);
    let mut drive = Blob(Some(with_estrelas(&old_phone)));

    let mut tablet = owner();
    tablet.sync(&mut drive).unwrap();
    assert_eq!(tablet.display_mode(), DisplayMode::Faixa);
}

#[test]
fn the_movie_page_shows_the_nota_prevista_and_its_faixa_whatever_the_mode() {
    let mut core = owner();
    core.set_display_mode(DisplayMode::Oculto, 0);
    let top = core.explorar(&Filtro::default(), 0).movies[0].tmdb_id;

    let page = core.movie(top).unwrap();
    assert!(page.card.nota_prevista.is_some());
    assert_eq!(page.faixa, Some(Faixa::ApostaForte));
    // A movie with a nota has no nota prevista, so no faixa either.
    assert_eq!(core.movie(603).unwrap().faixa, None);
}
