//! Reading the MovieLens export (ratings, logs, wishlist; tags ignored) for the importação.

use std::collections::HashMap;

use serde::Deserialize;

use crate::{Error, NotaEntry};

const RATINGS_HEADER: &str = "movie_id,imdb_id,tmdb_id,rating,average_rating,title";
const LOGS_HEADER: &str = "datetime,login_id,action_type,log_json";
const WISHLIST_HEADER: &str = "movie_id,imdb_id,tmdb_id,average_rating,title";
const TAGS_HEADER: &str = "movie_id,imdb_id,tmdb_id,title,tag";

pub(crate) struct Export {
    /// One per row of the ratings file.
    pub notas: Vec<ImportedNota>,
    pub quero_assistir: Vec<u32>,
}

pub(crate) struct ImportedNota {
    /// `None` when MovieLens has no TMDB id for the movie.
    pub tmdb_id: Option<u32>,
    pub history: Vec<NotaEntry>,
}

#[derive(Deserialize)]
struct RatingLog {
    #[serde(rename = "movieId")]
    movie_id: u32,
    rating: Option<f32>,
    action: String,
    pred: Option<f64>,
}

fn invalid(msg: impl std::fmt::Display) -> Error {
    Error::InvalidImport(msg.to_string())
}

/// Files are told apart by their header row, so the owner can pick them in any order.
pub(crate) fn read(files: &[&str]) -> Result<Export, Error> {
    let (mut ratings, mut logs, mut quero_assistir) = (None, None, None);
    for file in files {
        let mut rows = parse_csv(file.trim_start_matches('\u{feff}'));
        let header = if rows.is_empty() { String::new() } else { rows.remove(0).join(",") };
        match header.as_str() {
            RATINGS_HEADER => ratings = Some(rows),
            LOGS_HEADER => logs = Some(rows),
            WISHLIST_HEADER => quero_assistir = Some(rows),
            TAGS_HEADER => {}
            _ => return Err(invalid(format!("unknown file with header `{header}`"))),
        }
    }
    let ratings = ratings.ok_or_else(|| invalid("missing ratings file"))?;
    let logs = logs.ok_or_else(|| invalid("missing logs file"))?;

    let mut history: HashMap<u32, Vec<NotaEntry>> = HashMap::new();
    let mut last_logged = 0;
    for row in &logs {
        let [datetime, _, action_type, json] = &row[..] else {
            return Err(invalid("logs row with wrong column count"));
        };
        let at = parse_datetime(datetime)?;
        last_logged = last_logged.max(at);
        if action_type != "rating" {
            continue;
        }
        let log: RatingLog = serde_json::from_str(json).map_err(invalid)?;
        // REMOVE lines need no handling: a removed nota is absent from the ratings file, and an
        // earlier one given again keeps its place in the history.
        // A -1 rating is MovieLens hiding the movie, not a nota.
        if let ("ADD", Some(nota)) = (log.action.as_str(), log.rating.filter(|&r| r != -1.0)) {
            if !crate::is_valid_nota(nota) {
                return Err(invalid(format!("logged nota {nota} out of range")));
            }
            history.entry(log.movie_id).or_default().push(NotaEntry {
                nota,
                at,
                movielens_pred: log.pred,
            });
        }
    }

    let mut notas = Vec::with_capacity(ratings.len());
    for row in &ratings {
        let [movie_id, _, tmdb_id, rating, ..] = &row[..] else {
            return Err(invalid("ratings row with wrong column count"));
        };
        let movie_id: u32 = movie_id.parse().map_err(invalid)?;
        let nota: f32 = rating.parse().map_err(invalid)?;
        if !crate::is_valid_nota(nota) {
            return Err(invalid(format!("nota {nota} out of range")));
        }
        let mut entries = history.remove(&movie_id).unwrap_or_default();
        entries.sort_by_key(|e| e.at);
        if entries.last().map(|e| e.nota) != Some(nota) {
            // The logs should always end on the current nota; if not, date it at the export's
            // last log line rather than dropping it.
            entries.push(NotaEntry { nota, at: last_logged, movielens_pred: None });
        }
        notas.push(ImportedNota { tmdb_id: tmdb_id.parse().ok(), history: entries });
    }

    let quero_assistir = quero_assistir
        .unwrap_or_default()
        .iter()
        .filter_map(|row| row.get(2)?.parse().ok())
        .collect();
    Ok(Export { notas, quero_assistir })
}

/// MovieLens log time, `2024-09-07 20:11:40.0`, as Unix seconds (read as UTC).
fn parse_datetime(s: &str) -> Result<i64, Error> {
    let bad = || invalid(format!("bad log datetime `{s}`"));
    let field = |range: std::ops::Range<usize>| -> Result<i64, Error> {
        s.get(range).and_then(|f| f.parse().ok()).ok_or_else(bad)
    };
    let (y, m, d) = (field(0..4)?, field(5..7)?, field(8..10)?);
    let (hh, mm, ss) = (field(11..13)?, field(14..16)?, field(17..19)?);
    // Days since 1970-01-01, from Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Ok(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// RFC 4180 CSV: quoted fields may hold commas, `""` and newlines.
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let (mut row, mut field) = (Vec::new(), String::new());
    let (mut quoted, mut chars) = (false, text.chars().peekable());
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') => quoted = true,
            (false, ',') => row.push(std::mem::take(&mut field)),
            (false, '\r') => {}
            (false, '\n') => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            (false, c) => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}
