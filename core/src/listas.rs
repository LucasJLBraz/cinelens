//! The listas prontas (design doc §7.3): named filtros that Início shows as rows and chips, and
//! that open Explorar with them set.

use crate::{Faixa, Filtro, Mostrar};

/// "Previsão alta": Boa chance or better.
const PREVISAO_ALTA: Option<Faixa> = Some(Faixa::BoaChance);
/// "Últimos 6 meses".
const LANCAMENTO_DIAS: i64 = 182;
// ponytail: fixed cuts for "pouco vistos" and "muito bem avaliados"; tune once the owner has
// browsed the lists for a while.
const POUCO_VISTO: u32 = 1_000;
const BEM_AVALIADO: f32 = 7.5;
/// Metacritic's "generally favorable" upper half; also Rotten Tomatoes's share.
const ACLAMADO: u8 = 75;

/// Every lista pronta in Início's order; `at` (Unix seconds) is today. Without any critics'
/// scores in the catálogo, Aclamados pela crítica is the public's alone and named so.
pub(crate) fn listas(at: i64, tem_critica: bool) -> Vec<(&'static str, Filtro)> {
    let nao_visto = Filtro { com_nota: Mostrar::Esconder, ja_vi: Mostrar::Esconder, ..Default::default() };
    vec![
        ("Em alta, do seu jeito", Filtro { em_alta: true, ..nao_visto.clone() }),
        ("No seu streaming hoje", Filtro { no_meu_streaming: true, ..nao_visto.clone() }),
        (
            "Lançamentos para você",
            Filtro { lancado_desde: Some(date(at - LANCAMENTO_DIAS * 86_400)), faixa_min: PREVISAO_ALTA, ..nao_visto.clone() },
        ),
        ("Joias escondidas", Filtro { votos_max: Some(POUCO_VISTO), faixa_min: PREVISAO_ALTA, ..nao_visto.clone() }),
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
                ..nao_visto.clone()
            },
        ),
        // "< 1h40", as Explorar's "Até 1h40" so the chip shows it.
        ("Cabe numa noite", Filtro { duracao_max: Some(100), ..nao_visto.clone() }),
        ("Clássicos que você não viu", Filtro { ano_max: Some(1979), media_tmdb_min: Some(BEM_AVALIADO), ..nao_visto.clone() }),
        (
            if tem_critica { "Aclamados pela crítica" } else { "Aclamados pelo público" },
            Filtro { critica_min: Some(ACLAMADO), ..nao_visto },
        ),
        ("Para rever", Filtro { ja_vi: Mostrar::So, faixa_min: PREVISAO_ALTA, ..Default::default() }),
    ]
}

/// `YYYY-MM-DD` (UTC) of a Unix time.
fn date(secs: i64) -> String {
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
