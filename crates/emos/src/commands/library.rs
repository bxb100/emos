use std::collections::HashSet;

use anyhow::Result;
use emos_client::Client as EmosClient;
use emos_client::video::VideoQuery;
use emos_db::VideoRepository;
use tracing::debug;
use tracing::info;

pub(crate) async fn sync_videos() -> Result<()> {
    let repository = VideoRepository::open().await?;
    let api = EmosClient::from_env()?;
    let page_size = 100;
    let mut page = 1;

    loop {
        let resp = api
            .search_videos(&VideoQuery {
                page: Some(page),
                page_size: Some(page_size),
                ..Default::default()
            })
            .await?;
        let mut items = resp.items;
        if items.is_empty() {
            break;
        }
        let need_filter_ids = repository
            .existing_todb_ids(items.iter().map(|item| item.todb_id).collect())
            .await?;

        debug!(
            "items len {}, need filter: {}",
            items.len(),
            need_filter_ids.len()
        );

        if need_filter_ids.len() == items.len() {
            break;
        }
        let need_filter_ids: HashSet<_> = need_filter_ids.into_iter().collect();
        items.retain(|item| !need_filter_ids.contains(&item.todb_id));

        let inserted_rows = repository.insert_videos(items).await?;

        info!("inserted {} new data", inserted_rows);

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        if page * page_size >= resp.total as u32 {
            break;
        }

        page += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn test_sync() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
        sync_videos().await.unwrap();
    }
}
