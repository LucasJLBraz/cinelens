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
    /// Words that must all appear in the pt-BR or original title, a director or an actor,
    /// exactly or with a typo or two; matching titles come first.
    pub busca: String,
    /// Populares ↔ Garimpo, -1 to 1: how much popularity raises (below 0) or lowers (above 0)
    /// a recomendação. 0 is the nota prevista alone.
    pub garimpo: f32,
}

/// A `Filtro` ready to test movies against.
pub(crate) struct Matcher<'a> {
    filtro: &'a Filtro,
    words: Vec<String>,
    /// The words joined by single spaces, to recognize a whole title.
    frase: String,
    incluir: HashSet<u32>,
    excluir: HashSet<u32>,
}

impl<'a> Matcher<'a> {
    pub fn new(filtro: &'a Filtro) -> Matcher<'a> {
        Matcher {
            filtro,
            words: fold(&filtro.busca).split_whitespace().map(str::to_string).collect(),
            frase: fold(&filtro.busca).split_whitespace().collect::<Vec<_>>().join(" "),
            incluir: filtro.incluir.iter().copied().collect(),
            excluir: filtro.excluir.iter().copied().collect(),
        }
    }

    /// Whether movie `i` passes, as its `relevancia`.
    pub fn passes(&self, core: &Core, i: usize) -> Option<(bool, usize)> {
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
        if (self.incluir.is_empty() || if f.qualquer { genres() > 0 } else { genres() == self.incluir.len() })
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
        {
            self.relevancia(core, i)
        } else {
            None
        }
    }

    /// How well the busca matches movie `i`, lower first; `None` when some word is missing.
    /// A title that is the whole busca first, then by word: exact in a title, exact in a name,
    /// a typo in a title, a typo in a name.
    fn relevancia(&self, core: &Core, i: usize) -> Option<(bool, usize)> {
        if self.words.is_empty() {
            return Some((false, 0));
        }
        let b = &core.busca[i];
        let mut custo = 0;
        for w in &self.words {
            custo += if b.titulos.contains(w.as_str()) {
                0
            } else if b.pessoas.contains(w.as_str()) {
                1
            } else {
                // An exact word would have been found above, so 1 is the best a typo can do.
                let typo = |text: &str| {
                    let mut best = None;
                    for d in text.split_whitespace().filter_map(|t| typo(w, t)) {
                        if d == 1 {
                            return Some(1);
                        }
                        best = Some(best.map_or(d, |b: usize| b.min(d)));
                    }
                    best
                };
                typo(&b.titulos).map(|d| 2 * d).or_else(|| typo(&b.pessoas).map(|d| 2 * d + 1))?
            };
        }
        Some((!b.titulos.lines().any(|t| t == self.frase), custo))
    }
}

/// What search looks in, folded: the titles, also without spaces ("cidadededeus"), and the
/// directors' and actors' names.
pub(crate) struct Busca {
    titulos: String,
    pessoas: String,
}

pub(crate) fn searchable(m: &Movie) -> Busca {
    let palavras = |t: &str| fold(t).split_whitespace().collect::<Vec<_>>().join(" ");
    let (title, original) = (palavras(&m.title), palavras(&m.original_title));
    let juntos = |t: &str| t.replace(' ', "");
    // Composers are not searched: owners look up who directed or acted.
    let people: Vec<&str> = m.directors.iter().chain(&m.cast).map(|p| p.name.as_str()).collect();
    Busca {
        titulos: format!("{title}\n{original}\n{} {}", juntos(&title), juntos(&original)),
        pessoas: fold(&people.join("\n")),
    }
}

/// The edits (a letter added, dropped, changed, or two swapped) that turn `w` into the start of
/// `t`, when few enough for `w`'s length: none under 4 letters, 1 up to 7, then 2. Starting
/// at `t`'s start forgives a word still being typed.
// ponytail: compares bytes, so a non-Latin letter counts as several edits; and the first letter
// must be right, which keeps a busca over the whole catálogo fast.
fn typo(w: &str, t: &str) -> Option<usize> {
    let (w, t) = (w.as_bytes(), t.as_bytes());
    let k = match w.len() {
        0..=3 => return None,
        4..=7 => 1,
        _ => 2,
    };
    if w[0] != t[0] || t.len() + k < w.len() {
        return None;
    }
    let n = t.len().min(w.len() + k);
    // Rows i-2, i-1 and i of d[i][j], the edits from w[..i] to t[..j] (optimal string
    // alignment), on the stack: a busca compares hundreds of thousands of words.
    const MAX: usize = 40;
    if n >= MAX {
        return None;
    }
    let (mut antes, mut cima, mut linha) = ([0; MAX], [0; MAX], [0; MAX]);
    (0..=n).for_each(|j| linha[j] = j);
    for i in 1..=w.len() {
        (antes, cima, linha) = (cima, linha, antes);
        linha[0] = i;
        for j in 1..=n {
            let mut e = (cima[j] + 1).min(linha[j - 1] + 1).min(cima[j - 1] + usize::from(w[i - 1] != t[j - 1]));
            if i > 1 && j > 1 && w[i - 1] == t[j - 2] && w[i - 2] == t[j - 1] {
                e = e.min(antes[j - 2] + 1);
            }
            linha[j] = e;
        }
        if linha[..=n].iter().all(|&e| e > k) {
            return None;
        }
    }
    linha[..=n].iter().copied().min().filter(|&e| e <= k)
}

/// Lowercase without accents or punctuation, so "amelie" finds "Amélie" and "homem aranha"
/// finds "Homem-Aranha".
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
            c if c.is_alphanumeric() || c == '\n' => c,
            _ => ' ',
        })
        .collect()
}
