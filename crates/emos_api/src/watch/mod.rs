pub mod dynamic;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::EmosApi;

#[derive(Debug, Serialize)]
pub struct UpdateWatchVideoBatchItem {
    #[serde(rename = "type")]
    pub r#type: BatchType,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum BatchType {
    TmdbTv,
    TmdbMovie,
    Todb,
    VideoId,
}
impl EmosApi {
    pub async fn batch_update_watch_videos(
        &self,
        watch_id: &str,
        items: Vec<UpdateWatchVideoBatchItem>,
    ) -> Result<()> {
        let url = format!("{}/api/watch/{}/video/update", self.base_url, watch_id);

        self.client
            .post(url)
            .json(&items)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}
