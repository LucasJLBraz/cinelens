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
