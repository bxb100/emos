use anyhow::Result;
use emos_client::Client as EmosClient;
use emos_client::watch::VideoIdKind;
use emos_client::watch::WatchVideo;
use emos_db::VideoRepository;
use tokio::time::Duration;
use tokio::time::sleep;
use tracing::info;

pub(crate) async fn update_by_genre(genre: String, watch_id: String) -> Result<()> {
    let repository = VideoRepository::open().await?;
    let client = EmosClient::from_env()?;
    let mut max_todb_id = -1;

    loop {
        let videos = repository
            .find_by_genre_after(max_todb_id, &genre, 500)
            .await?;
        let Some(last) = videos.last() else {
            break;
        };
        max_todb_id = last.todb_id;

        info!("fetch {} videos", videos.len());
        let params = videos
            .into_iter()
            .map(|v| WatchVideo {
                kind: VideoIdKind::Todb,
                value: v.todb_id.to_string(),
            })
            .collect::<Vec<_>>();

        client.batch_update_watch_videos(&watch_id, params).await?;

        sleep(Duration::from_secs(10)).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use tokio::select;

    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn test_add_watch() {
        let watch_id = "1157".to_owned();
        let genre = "动画".to_owned();

        select! {
            _ = update_by_genre(genre, watch_id) => {},
            _ = sleep(Duration::from_millis(1600)) => {},
        }
    }
}
