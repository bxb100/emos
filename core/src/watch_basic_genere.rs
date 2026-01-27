use anyhow::Result;
use async_stream::stream;
use dao::Dao;
use dao::video::Video;
use emos_api::watch::BatchType;
use emos_api::watch::UpdateWatchVideoBatchItem;
use futures_util::StreamExt;
use futures_util::pin_mut;

pub async fn task(genre: &str, watch_id: &str) -> Result<()> {
    let dao = Dao::new().await?;
    let s = stream! {
        let mut max_todb_id:i64 = -1;
        loop {
            let videos: Vec<Video> = dao.find_all_by_genre(max_todb_id, genre).await?;
            if videos.is_empty() {
                break;
            }
            max_todb_id = videos.iter().map(|video| video.todb_id).max().unwrap();

            yield anyhow::Ok(videos);
        }
    };

    pin_mut!(s);

    let emos_api = emos_api::EmosApi::new()?;
    while let Some(Ok(value)) = s.next().await {
        let req = value
            .into_iter()
            .map(|v| UpdateWatchVideoBatchItem {
                r#type: BatchType::Todb,
                value: v.todb_id.to_string(),
            })
            .collect::<Vec<_>>();

        emos_api.batch_update_watch_videos(watch_id, req).await?;

        tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use tokio::pin;
    use tokio::select;

    use super::*;

    #[tokio::test]
    pub async fn test_add_watch() {
        let watch_id = "1157";
        let genre = "动画";

        let sleep = tokio::time::sleep(std::time::Duration::from_secs(1600));

        pin!(sleep);

        select! {
            _ = task(genre, watch_id) => {},
            _ = sleep => {},
        }
    }
}
