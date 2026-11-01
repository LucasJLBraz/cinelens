//! How the app identifies an obra (ADR 0003).

use std::fmt;

use serde::{Deserialize, Serialize};

const SERIE_BIT: u32 = 1 << 31;

/// A filme's id is its TMDB id; a série's is its TMDB id with the high bit set, because TMDB
/// numbers the two separately. Saved and sent as the plain number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObraId(u32);

impl ObraId {
    pub const fn filme(tmdb_id: u32) -> Self {
        Self(tmdb_id)
    }

    pub const fn serie(tmdb_id: u32) -> Self {
        Self(tmdb_id | SERIE_BIT)
    }

    /// As a série's: a série that becomes an anime keeps its notas, marks and sync keys. Which of
    /// the two an id names, only the catálogo knows.
    pub const fn anime(tmdb_id: u32) -> Self {
        Self::serie(tmdb_id)
    }

    /// A série or an anime.
    pub const fn is_serie(self) -> bool {
        self.0 & SERIE_BIT != 0
    }

    /// The number TMDB knows this obra by.
    pub const fn tmdb_id(self) -> u32 {
        self.0 & !SERIE_BIT
    }
}

impl fmt::Display for ObraId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// One row of `Fribb/anime-lists`, which joins MyAnimeList and AniList to TMDB; the catalog and
/// training jobs read it.
#[derive(Deserialize)]
pub struct Mapeamento {
    pub mal_id: Option<u32>,
    pub anilist_id: Option<u32>,
    /// `TV`, `ONA`, `MOVIE`, `SPECIAL`, `OVA`...
    #[serde(rename = "type", default)]
    pub tipo: Option<String>,
    #[serde(rename = "themoviedb_id", default)]
    tmdb: Option<TmdbRef>,
    #[serde(default)]
    season: Option<Season>,
}

#[derive(Deserialize)]
struct TmdbRef {
    tv: Option<u32>,
    movie: Option<u32>,
}

#[derive(Deserialize)]
struct Season {
    tmdb: Option<u32>,
}

impl Mapeamento {
    /// A season-like entry: a part of a run, not a special or an OVA derived from one.
    pub fn serializado(&self) -> bool {
        matches!(self.tipo.as_deref(), Some("TV" | "ONA"))
    }

    pub fn tmdb_tv(&self) -> Option<u32> {
        self.tmdb.as_ref()?.tv
    }

    pub fn tmdb_filme(&self) -> Option<u32> {
        self.tmdb.as_ref()?.movie
    }

    /// TMDB's season number; `None` for a run the mapping does not split.
    pub fn temporada(&self) -> Option<u32> {
        self.season.as_ref()?.tmdb
    }
}

pub fn read_mapeamento(bytes: &[u8]) -> Result<Vec<Mapeamento>, String> {
    serde_json::from_slice(bytes).map_err(|e| format!("anime-lists: {e}"))
}
