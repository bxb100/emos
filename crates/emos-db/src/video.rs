use serde::Deserialize;

/// A video stored in the local catalog.
#[derive(Debug, Deserialize)]
pub struct Video {
    pub todb_id: i64,
    pub tmdb_id: i64,
    pub video_id: i64,
    pub video_type: Option<String>,
    pub video_title: Option<String>,
    pub genres: Option<String>,
}
