use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Movie {
    pub title: String,
    pub year: Option<u64>,
    pub ids: MediaIds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Show {
    pub title: String,
    pub year: Option<u64>,
    pub ids: MediaIds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaIds {
    pub trakt: u64,
    pub slug: String,
    pub imdb: Option<String>,
    pub tmdb: Option<u64>,
}
