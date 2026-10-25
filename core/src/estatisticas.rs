//! Perfil's statistics on the owner's notas.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::{filtro::PUBLICO_MIN_VOTOS, is_valid_nota, listas::civil, Core, MovieCard};

/// Movies in each of Perfil's lists.
const LISTA: usize = 10;

/// Notas grouped by something the movies share.
#[derive(Debug, Serialize)]
pub struct Grupo {
    pub nome: String,
    pub notas: usize,
    /// Mean current nota.
    pub media: f32,
}

/// Everything Perfil shows about the owner's notas.
#[derive(Debug, Serialize)]
pub struct Estatisticas {
    /// Movies per current nota, 0.5 to 5 in half stars; a nota out of range is left out.
    pub distribuicao: [usize; 10],
    /// Most notas first, then the higher mean; only movies in the catálogo.
    pub por_genero: Vec<Grupo>,
    /// Decades oldest first, named by their first year.
    pub por_decada: Vec<Grupo>,
    pub por_direcao: Vec<Grupo>,
    /// `YYYY-MM` (UTC) and the movies given a nota that month, every month from the first nota.
    pub por_mes: Vec<(String, usize)>,
    /// The owner's mean nota minus the public's (TMDB's average, in stars) on the same movies:
    /// below zero is harsher. `None` without a movie the public has voted enough on.
    pub vs_publico: Option<f32>,
    /// Those movies, the ones the owner disagrees with the public most first.
    pub discordancias: Vec<Discordancia>,
    /// Every nota given (old ones in the history too) by the year it was given, oldest first.
    pub por_ano: Vec<Ano>,
    /// Movies whose current nota is not their first, furthest from it first.
    pub mudancas: Vec<Mudanca>,
    /// Each nota given in the app against the nota prevista stored when it was given (revealed
    /// then, unless the display mode was Oculto); `None` before any.
    pub acerto: Option<Acerto>,
}

/// Notas given in one year.
#[derive(Debug, Serialize)]
pub struct Ano {
    pub ano: i64,
    pub notas: usize,
    pub media: f32,
}

/// A movie the owner changed their nota for.
#[derive(Debug, Serialize)]
pub struct Mudanca {
    pub card: MovieCard,
    /// The first nota.
    pub de: f32,
    /// The current one.
    pub para: f32,
}

/// How close the nota prevista came to the notas given after it.
#[derive(Debug, Serialize)]
pub struct Acerto {
    pub notas: usize,
    /// Mean absolute error.
    pub mae: f32,
}

/// A movie the owner and the public disagree on.
#[derive(Debug, Serialize)]
pub struct Discordancia {
    pub card: MovieCard,
    pub nota: f32,
    /// TMDB's average, in stars.
    pub publico: f32,
}

/// Month index (`year * 12 + month - 1`) of a Unix time.
fn mes(secs: i64) -> i64 {
    let (y, m, _) = civil(secs);
    y * 12 + m - 1
}

fn media(valores: &[f32]) -> Option<f32> {
    (!valores.is_empty()).then(|| valores.iter().sum::<f32>() / valores.len() as f32)
}

/// Groups `(key, nota)` pairs, keyed order.
fn agrupar<K: Ord>(pares: impl Iterator<Item = (K, f32)>) -> Vec<(K, usize, f32)> {
    let mut somas: BTreeMap<K, (usize, f32)> = BTreeMap::new();
    for (k, nota) in pares {
        let s = somas.entry(k).or_default();
        *s = (s.0 + 1, s.1 + nota);
    }
    somas.into_iter().map(|(k, (n, soma))| (k, n, soma / n as f32)).collect()
}

fn grupos(pares: impl Iterator<Item = (String, f32)>) -> Vec<Grupo> {
    let mut grupos: Vec<Grupo> = agrupar(pares).into_iter().map(|(nome, notas, media)| Grupo { nome, notas, media }).collect();
    grupos.sort_by(|a, b| b.notas.cmp(&a.notas).then(b.media.total_cmp(&a.media)));
    grupos
}

impl Core {
    pub fn estatisticas(&self) -> Estatisticas {
        let atuais: Vec<(u32, f32)> =
            self.owner.notas.iter().filter_map(|(&id, h)| Some((id, h.last()?.nota))).filter(|&(_, nota)| is_valid_nota(nota)).collect();
        let mut distribuicao = [0; 10];
        atuais.iter().for_each(|&(_, nota)| distribuicao[(nota * 2.0) as usize - 1] += 1);
        let no_catalogo: Vec<(&crate::Movie, f32)> =
            atuais.iter().filter_map(|&(id, nota)| Some((&self.movies[*self.index.get(&id)?], nota))).collect();
        let genero = |id: &u32| self.generos.iter().find(|g| g.id == *id).map(|g| g.name.clone());
        let por_genero = grupos(no_catalogo.iter().flat_map(|&(m, nota)| m.genre_ids.iter().filter_map(genero).map(move |g| (g, nota))));
        let por_direcao = grupos(no_catalogo.iter().flat_map(|&(m, nota)| m.directors.iter().map(move |p| (p.name.clone(), nota))));
        let decadas = no_catalogo.iter().filter_map(|&(m, nota)| Some((m.year? / 10 * 10, nota)));
        let por_decada = agrupar(decadas).into_iter().map(|(d, notas, media)| Grupo { nome: d.to_string(), notas, media }).collect();
        let mut meses: BTreeMap<i64, BTreeSet<u32>> = BTreeMap::new();
        for (&id, h) in &self.owner.notas {
            h.iter().for_each(|e| _ = meses.entry(mes(e.at)).or_default().insert(id));
        }
        let por_mes = match (meses.first_key_value(), meses.last_key_value()) {
            (Some((&primeiro, _)), Some((&ultimo, _))) => (primeiro..=ultimo)
                .map(|m| (format!("{:04}-{:02}", m / 12, m % 12 + 1), meses.get(&m).map_or(0, BTreeSet::len)))
                .collect(),
            _ => Vec::new(),
        };
        let discordancias: Vec<(usize, f32, f32)> = atuais
            .iter()
            .filter_map(|&(id, nota)| {
                let i = *self.index.get(&id)?;
                let m = &self.movies[i];
                (m.vote_count >= PUBLICO_MIN_VOTOS).then_some((i, nota, m.vote_average / 2.0))
            })
            .collect();
        let vs_publico = media(&discordancias.iter().map(|&(_, nota, publico)| nota - publico).collect::<Vec<_>>());
        let mudancas = self.owner.notas.iter().filter_map(|(id, h)| {
            let (de, para) = (h.first()?.nota, h.last()?.nota);
            (de != para).then_some((*self.index.get(id)?, de, para))
        });
        let entradas = || self.owner.notas.values().flatten();
        let por_ano = agrupar(entradas().map(|e| (civil(e.at).0, e.nota)));
        let erros: Vec<f32> = entradas().filter_map(|e| Some((e.prevista? - e.nota).abs())).collect();
        Estatisticas {
            distribuicao,
            por_genero,
            por_decada,
            por_direcao,
            por_mes,
            vs_publico,
            discordancias: self.mais_distantes(discordancias).map(|(card, nota, publico)| Discordancia { card, nota, publico }).collect(),
            por_ano: por_ano.into_iter().map(|(ano, notas, media)| Ano { ano, notas, media }).collect(),
            mudancas: self.mais_distantes(mudancas.collect()).map(|(card, de, para)| Mudanca { card, de, para }).collect(),
            acerto: media(&erros).map(|mae| Acerto { notas: erros.len(), mae }),
        }
    }

    /// The `LISTA` movies whose two notas are furthest apart, furthest first; ties keep the
    /// given order.
    fn mais_distantes(&self, mut pares: Vec<(usize, f32, f32)>) -> impl Iterator<Item = (MovieCard, f32, f32)> + '_ {
        pares.sort_by(|a, b| (b.1 - b.2).abs().total_cmp(&(a.1 - a.2).abs()));
        pares.into_iter().take(LISTA).map(|(i, a, b)| (self.card(i), a, b))
    }
}
