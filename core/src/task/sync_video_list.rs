use std::collections::HashSet;

use anyhow::Result;
use dao::Dao;
use futures_util::StreamExt;
use futures_util::pin_mut;
use tracing::debug;

pub async fn task() -> Result<()> {
    let dao = Dao::new().await?;

    let stream = dao.fetch_all_videos().await;
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

        dao.insert(items).await?;
    }

    Ok(())
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
