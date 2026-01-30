use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize)]
pub struct PagedResult<T> {
    pub page: u64,
    pub results: Vec<T>,
    pub total_pages: u64,
    pub total_results: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "media_type", rename_all = "lowercase")]
pub enum MediaItem {
    Movie(Movie),
    Tv(Tv),
    Person(Person),
    Collection,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Movie {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub original_title: String,
    #[serde(default)]
    pub overview: String,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub release_date: Option<String>,
    #[serde(default)]
    pub vote_average: f64,
    #[serde(default)]
    pub vote_count: u64,
    #[serde(default)]
    pub popularity: f64,
    #[serde(default)]
    pub genre_ids: Vec<u64>,
    pub adult: Option<bool>,
    pub original_language: Option<String>,
    pub video: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tv {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub original_name: String,
    #[serde(default)]
    pub overview: String,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub vote_average: f64,
    #[serde(default)]
    pub vote_count: u64,
    #[serde(default)]
    pub popularity: f64,
    #[serde(default)]
    pub genre_ids: Vec<u64>,
    pub original_language: Option<String>,
    #[serde(default)]
    pub origin_country: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Person {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub original_name: String,
    pub profile_path: Option<String>,
    #[serde(default)]
    pub known_for: Vec<MediaItem>,
    pub gender: Option<u8>,
    pub known_for_department: Option<String>,
    #[serde(default)]
    pub popularity: f64,
    pub adult: Option<bool>,
}
