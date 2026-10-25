//! Explorar's filters and search.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{display, Core, Faixa, Movie};

/// Votes a TMDB average needs to count as the public's opinion: to stand in for a missing
/// critics' score, and in Perfil's comparisons with the public.
// ponytail: fixed cut; tune once Aclamados pela crítica and Perfil's disagreements have been browsed.
pub(crate) const PUBLICO_MIN_VOTOS: u32 = 500;

/// Hide, or show only, the movies with a mark.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Mostrar {
    #[default]
    Todos,
    Esconder,
    So,
}

/// Explorar's filters; the default is no filter. Every set field must pass.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Filtro {
    /// Genre ids the movie must have: all of them, or any with `qualquer`.
    pub incluir: Vec<u32>,
    pub qualquer: bool,
    /// Genre ids the movie must not have.
    pub excluir: Vec<u32>,
    pub ano_min: Option<u16>,
    pub ano_max: Option<u16>,
    /// Minutes.
    pub duracao_min: Option<u32>,
    pub duracao_max: Option<u32>,
    /// Original language, ISO 639-1.
    pub idioma: Option<String>,
    /// Production country, ISO 3166-1.
    pub pais: Option<String>,
    pub nota_prevista_min: Option<f32>,
    pub confianca_min: Option<f32>,
    /// A nota prevista in this faixa or a better one, whatever the display mode.
    pub faixa_min: Option<Faixa>,
    /// In TMDB trending this week.
    pub em_alta: bool,
    /// Released on this day (`YYYY-MM-DD`) or later.
    pub lancado_desde: Option<String>,
    /// At most this many TMDB votes: little seen.
    pub votos_max: Option<u32>,
    /// TMDB's public average, 0–10.
    pub media_tmdb_min: Option<f32>,
    /// Critics' score (0–100) at least this; a movie without one passes on the public instead:
    /// a TMDB average at least a tenth of it, from enough votes to mean something.
    pub critica_min: Option<u8>,
    /// Brazilian age ratings allowed (L, 10, 12…); empty is any, even none.
    pub classificacao: Vec<String>,
    /// Movies the owner gave a nota.
    pub com_nota: Mostrar,
    pub ja_vi: Mostrar,
    pub quero_assistir: Mostrar,
    /// Only the movies marked não me interessa (hidden otherwise), so they can be restored.
    pub nao_me_interessa: bool,
    /// Only movies on a subscription service the owner has.
    pub no_meu_streaming: bool,
    /// Words that must all appear in the pt-BR or original title, a director or an actor.
    pub busca: String,
    /// Populares ↔ Garimpo, -1 to 1: how much popularity raises (below 0) or lowers (above 0)
    /// a recomendação. 0 is the nota prevista alone.
    pub garimpo: f32,
}

/// A `Filtro` ready to test movies against.
pub(crate) struct Matcher<'a> {
    filtro: &'a Filtro,
    words: Vec<String>,
    incluir: HashSet<u32>,
    excluir: HashSet<u32>,
}

impl<'a> Matcher<'a> {
    pub fn new(filtro: &'a Filtro) -> Matcher<'a> {
        Matcher {
            filtro,
            words: fold(&filtro.busca).split_whitespace().map(str::to_string).collect(),
            incluir: filtro.incluir.iter().copied().collect(),
            excluir: filtro.excluir.iter().copied().collect(),
        }
    }

    pub fn passes(&self, core: &Core, i: usize) -> bool {
        let f = self.filtro;
        let m = &core.movies[i];
        let owner = &core.owner;
        let genres = || m.genre_ids.iter().filter(|g| self.incluir.contains(g)).count();
        let in_range = |v: Option<u32>, min: Option<u32>, max: Option<u32>| {
            (min.is_none() && max.is_none()) || v.is_some_and(|v| min.is_none_or(|min| v >= min) && max.is_none_or(|max| v <= max))
        };
        let previsto = core.previstas[i];
        let rated = core.rated(m.tmdb_id);
        let shown = |mostrar: Mostrar, marked: bool| match mostrar {
            Mostrar::Todos => true,
            Mostrar::Esconder => !marked,
            Mostrar::So => marked,
        };
        let watch = m.watch.as_ref();
        (self.incluir.is_empty() || if f.qualquer { genres() > 0 } else { genres() == self.incluir.len() })
            && !m.genre_ids.iter().any(|g| self.excluir.contains(g))
            && in_range(m.year.map(u32::from), f.ano_min.map(u32::from), f.ano_max.map(u32::from))
            && in_range(m.runtime, f.duracao_min, f.duracao_max)
            && f.idioma.as_ref().is_none_or(|l| m.original_language.as_ref() == Some(l))
            && f.pais.as_ref().is_none_or(|c| m.countries.contains(c))
            && f.nota_prevista_min.is_none_or(|min| previsto.is_some_and(|(p, _)| p >= min))
            && f.confianca_min.is_none_or(|min| previsto.is_some_and(|(_, c)| c >= min))
            && f.faixa_min.is_none_or(|min| previsto.is_some_and(|(p, _)| display::faixa(p, &core.ranked) <= min))
            && (!f.em_alta || core.em_alta.contains(&m.tmdb_id))
            && f.lancado_desde.as_ref().is_none_or(|d| m.release_date.as_ref().is_some_and(|r| r >= d))
            && f.votos_max.is_none_or(|max| m.vote_count <= max)
            && f.media_tmdb_min.is_none_or(|min| m.vote_average >= min)
            && f.critica_min.is_none_or(|min| match m.critic_score {
                Some(score) => score >= min,
                None => m.vote_count >= PUBLICO_MIN_VOTOS && m.vote_average >= f32::from(min) / 10.0,
            })
            && (f.classificacao.is_empty() || m.certification.as_ref().is_some_and(|c| f.classificacao.contains(c)))
            && shown(f.com_nota, rated)
            && shown(f.ja_vi, owner.ja_vi.contains(&m.tmdb_id))
            && shown(f.quero_assistir, owner.quero_assistir.contains(&m.tmdb_id))
            && (!f.no_meu_streaming
                || watch.is_some_and(|w| w.flatrate.iter().any(|p| owner.settings.servicos.contains(p))))
            && self.words.iter().all(|w| core.busca[i].contains(w.as_str()))
    }
}

/// What search looks in: the titles and people's names, folded.
pub(crate) fn searchable(m: &Movie) -> String {
    // Composers are not searched: owners look up who directed or acted.
    let people = m.directors.iter().chain(&m.cast).map(|p| p.name.as_str());
    let texts: Vec<&str> = [m.title.as_str(), m.original_title.as_str()].into_iter().chain(people).collect();
    fold(&texts.join("\n"))
}

/// Lowercase without accents, so "amelie" finds "Amélie".
// ponytail: hand-written table for Portuguese, Spanish and French accents; misses ø, œ and the
// like. Unicode normalization (a new dependency) if titles in other languages fail to match.
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}
