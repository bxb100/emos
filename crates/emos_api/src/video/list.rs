use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tracing::instrument;

pub use crate::EmosApi;
use crate::ResponseExt;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Root {
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
    pub items: Vec<Item>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub item_id: Option<String>,
    pub todb_id: i64,
    pub tmdb_id: i64,
    pub tmdb_url: String,
    pub video_id: i64,
    pub video_type: String,
    pub video_title: String,
    pub video_origin_title: String,
    pub video_description: Option<String>,
    pub video_tagline: Option<String>,
    pub video_image_logo: Option<String>,
    pub video_image_poster: Option<String>,
    pub video_image_backdrop: Option<String>,
    pub video_date_air: Option<String>,
    pub video_is_adult: bool,
    pub seek_is_request: bool,
    pub seek_id: Value,
    pub request_count: i64,
    pub parts_count: i64,
    pub medias_count: i64,
    pub subtitles_count: i64,
    pub titles: Vec<String>,
    pub genres: Vec<Genre>,
    pub is_delete: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genre {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Default, Serialize)]
#[serde_with::skip_serializing_none]
pub struct QueryParams {
    pub tmdb_id: Option<String>,
    pub todb_id: Option<String>,
    pub video_id: Option<String>,
    // tv/movie
    pub r#type: Option<String>,
    // 0, 1
    pub only_delete: Option<u8>,
    pub with_media: Option<u8>,
    // 1-based
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl EmosApi {
    #[instrument(skip(self, query), fields(
        tmdb_id = query.tmdb_id,
        todb_id = query.todb_id,
        video_id = query.video_id,
        type = query.r#type,
        only_delete = query.only_delete,
        with_media = query.with_media,
        page = query.page,
        page_size = query.page_size,
    ))]
    pub async fn list(&self, query: &QueryParams) -> Result<Root> {
        self.client
            .get(format!("{}/api/video/list", self.base_url))
            .query(&query)
            .send()
            .await?
            .json_ext::<Root>()
            .await
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
            .list(&QueryParams {
                r#type: Some("tv".to_string()),
                with_media: Some(1),
                page: Some(1),
                page_size: Some(100),
                ..Default::default()
            })
            .await?;

        info!("{:#?}", data);
        Ok(())
    }
}
