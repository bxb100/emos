use anyhow::Context;
use anyhow::Result;
use emos_utils::ReqwestExt;
use serde::Deserialize;
use serde::Serialize;
use tracing::instrument;

pub use crate::EmosApi;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Root {
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
    pub items: Vec<Item>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    #[serde(rename = "video_id")]
    pub video_id: i64,
    #[serde(rename = "video_type")]
    pub video_type: String,
    #[serde(rename = "video_title")]
    pub video_title: String,
    #[serde(rename = "todb_id")]
    pub todb_id: i64,
    #[serde(rename = "tmdb_id")]
    pub tmdb_id: i64,
    pub genres: Vec<Genre>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genre {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde_with::skip_serializing_none]
pub struct QueryParams<'a> {
    pub tmdb_id: Option<&'a str>,
    pub todb_id: Option<&'a str>,
    pub video_id: Option<&'a str>,
    // tv/movie
    pub r#type: Option<&'a str>,
    // 0, 1
    pub with_genre: Option<u8>,
    pub sort_by: Option<&'a str>,
    // 1-based
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl Default for QueryParams<'_> {
    fn default() -> Self {
        Self {
            tmdb_id: None,
            todb_id: None,
            video_id: None,
            r#type: None,
            with_genre: Some(1),
            sort_by: Some("id"),
            page: Some(1),
            page_size: Some(20),
        }
    }
}

impl EmosApi {
    #[instrument(skip(self, query), fields(
        tmdb_id = query.tmdb_id,
        todb_id = query.todb_id,
        video_id = query.video_id,
        type = query.r#type,
        with_genre = query.with_genre,
        page = query.page,
        page_size = query.page_size,
    ))]
    pub async fn search(&self, query: &QueryParams<'_>) -> Result<Root> {
        let req = self
            .client
            .get(format!("{}/api/video/search", self.base_url))
            .query(&query);

        req.execute().await.context("Failed to search videos")
    }
}

#[cfg(test)]
mod tests {
    use tracing::info;

    use super::*;

    #[tokio::test]
    pub async fn test_list() -> Result<()> {
        tracing_subscriber::fmt().init();

        let data = EmosApi::new()?
            .search(&QueryParams {
                r#type: Some("tv"),
                ..Default::default()
            })
            .await?;

        info!("{:#?}", data);
        Ok(())
    }
}
