use std::collections::HashSet;

use anyhow::Context;
use anyhow::Result;
use async_stream::stream;
use clap::ArgMatches;
use dao::Dao;
use emos_api::video;
use emos_api::video::list::QueryParams;
use futures_util::Stream;
use futures_util::StreamExt;
use futures_util::pin_mut;
use tracing::debug;
use tracing::info;

use crate::Task;
use crate::add_task;

add_task!("sync_video_list", task);

pub async fn task() -> Result<()> {
    let dao = Dao::new().await?;

    let stream = fetch_all_videos().await;
    pin_mut!(stream);

    while let Some(items) = stream.next().await {
        let mut items = items?;
        // advance-mod
        let need_filter_ids = dao
            .exist_todb_ids(items.iter().map(|item| item.todb_id).collect())
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

        let inserted_rows = dao.insert(items).await?;

        info!("inserted {} new data", inserted_rows);

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    Ok(())
}
async fn fetch_all_videos() -> impl Stream<Item = Result<Vec<video::list::Item>>> {
    let page_size = 100;

    stream! {
        let api = video::list::EmosApi::new()?;
        let mut page = 1;

        loop {
            let resp = api.search(&QueryParams {
                page: Some(page),
                page_size: Some(page_size),
                ..Default::default()
            }).await?;

            let total = resp.total;
            let items = resp.items;
            if items.is_empty() {
                break;
            }

            yield Ok(items);

            if page * page_size >= total as u32 {
                break;
            }

            page += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    pub async fn test_sync() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
        task().await.unwrap();
    }
}
