use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrendingPage {
    pub items: Vec<TrendingItem>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pagination {
    pub page: u64,
    pub limit: u64,
    pub page_count: u64,
    pub item_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TrendingItem {
    Movie { watchers: u64, movie: Movie },
    Show { watchers: u64, show: Show },
}

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
