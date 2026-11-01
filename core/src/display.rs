//! How a nota prevista is shown (the "Mostrar previsão como" setting), kept apart from the
//! recommender so it can change without touching it. Faixa is marked for later refinement.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum DisplayMode {
    /// A band relative to the owner's own predictions. Stars live only behind a tap on the movie
    /// page, so a saved "Estrelas" reads as Faixa.
    #[default]
    #[serde(alias = "Estrelas")]
    Faixa,
    /// Nothing but the order (the movie page still shows the nota prevista, behind a tap).
    Oculto,
}

/// Best first.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Faixa {
    #[serde(rename = "Aposta forte")]
    ApostaForte,
    #[serde(rename = "Boa chance")]
    BoaChance,
    Arriscado,
    #[serde(rename = "Provavelmente não")]
    ProvavelmenteNao,
}

/// A nota prevista ready to show, with its confiança.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "modo")]
pub enum Exibicao {
    Faixa { faixa: Faixa, confianca: f32 },
}

/// Share of the owner's predictions above a nota prevista at which each band ends: Aposta forte
/// is about the top 10%.
// ponytail: fixed cut points; refine once the owner has used Faixa for a while.
const BANDS: [(f32, Faixa); 3] = [(0.10, Faixa::ApostaForte), (0.35, Faixa::BoaChance), (0.70, Faixa::Arriscado)];

/// The lowest nota prevista that guarantees each band at worst: a high nota never reads as a
/// verdict against the obra, wherever it falls among the owner's predictions.
const PISOS: [(f32, Faixa); 2] = [(4.0, Faixa::BoaChance), (3.0, Faixa::Arriscado)];

/// `ranked` holds every current nota prevista, highest first.
pub(crate) fn exibicao(mode: DisplayMode, nota: f32, confianca: f32, ranked: &[f32]) -> Option<Exibicao> {
    match mode {
        DisplayMode::Oculto => None,
        DisplayMode::Faixa => Some(Exibicao::Faixa { faixa: faixa(nota, ranked), confianca }),
    }
}

/// The faixa of a nota prevista; `ranked` as in `exibicao`.
pub(crate) fn faixa(nota: f32, ranked: &[f32]) -> Faixa {
    let above = ranked.partition_point(|&p| p > nota) as f32 / ranked.len().max(1) as f32;
    let posicao = BANDS.iter().find(|(end, _)| above < *end).map_or(Faixa::ProvavelmenteNao, |b| b.1);
    match PISOS.iter().find(|(piso, _)| nota >= *piso) {
        Some(&(_, piso)) if piso < posicao => piso,
        _ => posicao,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hundred predictions from 5.0 down, so every nota below sits under most of them.
    #[test]
    fn a_high_nota_never_gets_a_negative_faixa() {
        let altas: Vec<f32> = (0..100).map(|i| 5.0 - i as f32 / 200.0).collect();
        assert_eq!(faixa(4.1, &altas), Faixa::BoaChance);
        assert_eq!(faixa(4.0, &altas), Faixa::BoaChance);
        assert_eq!(faixa(3.9, &altas), Faixa::Arriscado);
        assert_eq!(faixa(3.0, &altas), Faixa::Arriscado);
        assert_eq!(faixa(2.9, &altas), Faixa::ProvavelmenteNao);
    }

    #[test]
    fn position_still_sets_the_faixa_above_the_floor() {
        let ranked = [4.8, 4.6, 4.4, 4.2];
        assert_eq!(faixa(4.8, &ranked), Faixa::ApostaForte);
        let baixas = [3.0, 2.5, 2.0, 1.5, 1.0];
        assert_eq!(faixa(1.0, &baixas), Faixa::ProvavelmenteNao);
    }
}
