use anyhow::Result;
use dao::Dao;
use futures_util::StreamExt;
use futures_util::pin_mut;
use tracing::debug;

pub async fn task() -> Result<()> {
    let v = Dao::new().await?;

    // fixme: it's not stable sort
    let stream = v.fetch_all_videos().await;
    pin_mut!(stream);

    while let Some(items) = stream.next().await {
        let items = items?;
        // advance-mod
        let need_filter_ids = v
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

        let items = items
            .into_iter()
            .filter(|item| !need_filter_ids.contains(&item.todb_id))
            .collect::<Vec<_>>();

        v.insert(items).await?;
    }

    Ok(())
}
