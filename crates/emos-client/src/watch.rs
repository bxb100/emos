use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::Client;

#[derive(Debug, Serialize)]
pub struct WatchVideo {
    #[serde(rename = "type")]
    pub kind: VideoIdKind,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum VideoIdKind {
    TmdbTv,
    TmdbMovie,
    Todb,
    VideoId,
}
impl Client {
    pub async fn batch_update_watch_videos(
        &self,
        watch_id: &str,
        items: Vec<WatchVideo>,
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
