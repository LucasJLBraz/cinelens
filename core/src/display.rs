//! How a nota prevista is shown (the "Mostrar previsão como" setting), kept apart from the
//! recommender so it can change without touching it. Faixa is marked for later refinement.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum DisplayMode {
    /// The decimal, the most likely half star and the confiança.
    Estrelas,
    /// A band relative to the owner's own predictions.
    #[default]
    Faixa,
    /// Nothing; the order alone.
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
    Estrelas { nota: f32, meia_estrela: f32, confianca: f32 },
    Faixa { faixa: Faixa, confianca: f32 },
}

/// Share of the owner's predictions above a nota prevista at which each band ends: Aposta forte
/// is about the top 10%.
// ponytail: fixed cut points; refine once the owner has used Faixa for a while.
const BANDS: [(f32, Faixa); 3] = [(0.10, Faixa::ApostaForte), (0.35, Faixa::BoaChance), (0.70, Faixa::Arriscado)];

/// `ranked` holds every current nota prevista, highest first.
pub(crate) fn exibicao(mode: DisplayMode, nota: f32, confianca: f32, ranked: &[f32]) -> Option<Exibicao> {
    match mode {
        DisplayMode::Oculto => None,
        DisplayMode::Estrelas => {
            Some(Exibicao::Estrelas { nota, meia_estrela: (nota * 2.0).round() / 2.0, confianca })
        }
        DisplayMode::Faixa => Some(Exibicao::Faixa { faixa: faixa(nota, ranked), confianca }),
    }
}

/// The faixa of a nota prevista; `ranked` as in `exibicao`.
pub(crate) fn faixa(nota: f32, ranked: &[f32]) -> Faixa {
    let above = ranked.partition_point(|&p| p > nota) as f32 / ranked.len().max(1) as f32;
    BANDS.iter().find(|(end, _)| above < *end).map_or(Faixa::ProvavelmenteNao, |b| b.1)
}
