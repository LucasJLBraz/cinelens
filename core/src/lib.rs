//! Cinelens core: the single facade the UI talks to.

use serde::{Deserialize, Serialize};

const CATALOG_FORMAT_VERSION: u32 = 1;
const PAGE_SIZE: usize = 30;
const POSTER_BASE: &str = "https://image.tmdb.org/t/p/w342";

#[derive(Debug)]
pub enum Error {
    InvalidArtifact(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidArtifact(msg) => write!(f, "invalid catálogo artifact: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Deserialize)]
struct CatalogArtifact {
    format_version: u32,
    movies: Vec<Movie>,
}

#[derive(Deserialize)]
struct Movie {
    tmdb_id: u32,
    title: String,
    original_title: String,
    year: u16,
    poster_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MovieCard {
    pub tmdb_id: u32,
    pub title: String,
    pub original_title: String,
    pub year: u16,
    pub poster_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExplorarPage {
    pub movies: Vec<MovieCard>,
    pub has_more: bool,
}

pub struct Core {
    movies: Vec<Movie>,
}

impl Core {
    pub fn open(catalog_artifact: &[u8]) -> Result<Core, Error> {
        let artifact: CatalogArtifact = serde_json::from_slice(catalog_artifact)
            .map_err(|e| Error::InvalidArtifact(e.to_string()))?;
        if artifact.format_version != CATALOG_FORMAT_VERSION {
            return Err(Error::InvalidArtifact(format!(
                "unknown format version {}",
                artifact.format_version
            )));
        }
        Ok(Core { movies: artifact.movies })
    }

    pub fn explorar(&self, page: usize) -> ExplorarPage {
        let start = (page * PAGE_SIZE).min(self.movies.len());
        let end = (start + PAGE_SIZE).min(self.movies.len());
        ExplorarPage {
            movies: self.movies[start..end]
                .iter()
                .map(|m| MovieCard {
                    tmdb_id: m.tmdb_id,
                    title: m.title.clone(),
                    original_title: m.original_title.clone(),
                    year: m.year,
                    poster_url: m.poster_path.as_ref().map(|p| format!("{POSTER_BASE}{p}")),
                })
                .collect(),
            has_more: end < self.movies.len(),
        }
    }
}
