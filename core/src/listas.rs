//! The listas prontas (design doc §7.3): named filtros that Início shows as rows, and
//! that open Buscar with them set.

use crate::{Faixa, Filtro, Mostrar, Tipo};

/// "Previsão alta": Boa chance or better.
const PREVISAO_ALTA: Option<Faixa> = Some(Faixa::BoaChance);
/// "Últimos 6 meses".
const LANCAMENTO_DIAS: i64 = 182;
/// A broadcast season in Japan, about three months.
const TEMPORADA_DIAS: i64 = 90;
/// About three months of weekly episódios, the usual short anime.
const CURTO_EPISODIOS: u32 = 13;
// ponytail: fixed cuts for "pouco vistos" and "muito bem avaliados"; tune once the owner has
// browsed the lists for a while.
const POUCO_VISTO: u32 = 1_000;
const BEM_AVALIADO: f32 = 7.5;
/// Metacritic's "generally favorable" upper half; also Rotten Tomatoes's share.
const ACLAMADO: u8 = 75;

/// Every lista pronta in Início's order; `at` (Unix seconds) is today. Without any critics'
/// scores in the catálogo, Aclamados pela crítica is the public's alone and named so.
pub(crate) fn listas(at: i64, tem_critica: bool) -> Vec<(&'static str, Filtro)> {
    let obras = Filtro { com_nota: Mostrar::Esconder, ja_vi: Mostrar::Esconder, assistindo: Mostrar::Esconder, acima_do_piso: true, ..Default::default() };
    let filmes = Filtro { tipo: Some(Tipo::Filme), ..obras.clone() };
    let series = Filtro { tipo: Some(Tipo::Serie), ..obras.clone() };
    let animes = Filtro { tipo: Some(Tipo::Anime), ..obras.clone() };
    let terminadas = Filtro { situacao: Some("terminada".into()), ..series.clone() };
    vec![
        ("Em alta, do seu jeito", Filtro { em_alta: true, ..obras.clone() }),
        // The owner's own marks: no floor.
        ("Da sua lista, no seu streaming", Filtro { quero_assistir: Mostrar::So, no_meu_streaming: true, acima_do_piso: false, ..obras.clone() }),
        ("No seu streaming hoje", Filtro { no_meu_streaming: true, ..obras.clone() }),
        (
            "Lançamentos para você",
            Filtro { lancado_desde: Some(date(at - LANCAMENTO_DIAS * 86_400)), faixa_min: PREVISAO_ALTA, ..obras.clone() },
        ),
        ("Com quem você é fã", Filtro { com_fa: true, ..obras.clone() }),
        ("Joias escondidas", Filtro { votos_max: Some(POUCO_VISTO), faixa_min: PREVISAO_ALTA, ..obras.clone() }),
        (
            if tem_critica { "Aclamados pela crítica" } else { "Aclamados pelo público" },
            Filtro { critica_min: Some(ACLAMADO), ..obras.clone() },
        ),
        ("Feito no Brasil", Filtro { pais: Some("BR".into()), ..obras }),
        ("Para rever", Filtro { ja_vi: Mostrar::So, faixa_min: PREVISAO_ALTA, acima_do_piso: true, ..Default::default() }),
        ("Filmes para você", filmes.clone()),
        (
            "Sessão da Tarde",
            Filtro {
                // Aventura, Comédia, Família.
                incluir: vec![12, 35, 10751],
                qualquer: true,
                classificacao: ["L", "10", "12"].map(String::from).to_vec(),
                ano_min: Some(1980),
                ano_max: Some(2010),
                garimpo: -1.0,
                ..filmes.clone()
            },
        ),
        // "< 1h40", as Buscar's "Até 1h40" so the chip shows it.
        ("Cabe numa noite", Filtro { duracao_max: Some(100), ..filmes.clone() }),
        ("Clássicos que você não viu", Filtro { ano_max: Some(1979), media_tmdb_min: Some(BEM_AVALIADO), ..filmes }),
        ("Séries para você", series.clone()),
        ("Para maratonar", terminadas.clone()),
        ("Séries em andamento", Filtro { situacao: Some("em_andamento".into()), ..series }),
        ("Uma temporada só", Filtro { temporadas_max: Some(1), ..terminadas }),
        ("Animes para você", animes.clone()),
        ("Desta temporada", Filtro { estreia_entre: Some((date(at - TEMPORADA_DIAS * 86_400), date(at))), ..animes.clone() }),
        ("Animes curtos", Filtro { episodios_max: Some(CURTO_EPISODIOS), ..animes }),
        // Below the main rows; the owner's own, so nothing about it is left out.
        ("Assistindo", Filtro { assistindo: Mostrar::So, ..Default::default() }),
    ]
}

/// The listas for Início's type chip. Tudo shows the mixed ones, and a filme-only, série-only or
/// anime-only one only under its own chip; a chip narrows the mixed ones to its type.
pub(crate) fn listas_de(at: i64, tem_critica: bool, tipo: Option<Tipo>) -> Vec<(&'static str, Filtro)> {
    listas(at, tem_critica)
        .into_iter()
        .filter(|(_, f)| f.tipo.is_none() || f.tipo == tipo)
        .map(|(nome, f)| (nome, Filtro { tipo, ..f }))
        .collect()
}

/// `YYYY-MM-DD` (UTC) of a Unix time.
pub(crate) fn date(secs: i64) -> String {
    let (y, m, d) = civil(secs);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Year, month and day (UTC) of a Unix time: Howard Hinnant's civil_from_days.
pub(crate) fn civil(secs: i64) -> (i64, i64, i64) {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
