use std::path::Path;

use anyhow::Result;
use async_stream::stream;
use emos_api::video;
use emos_api::video::list::Item;
use emos_api::video::list::QueryParams;
use futures_util::Stream;
use futures_util::StreamExt;
use futures_util::pin_mut;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::SqlitePool;
use sqlx::migrate::MigrateDatabase;
use sqlx::query_scalar;
use sqlx::types::Json;
use tracing::debug;
use tracing::info;

pub struct VideoTable(sqlx::SqlitePool);

impl VideoTable {
    pub async fn new() -> anyhow::Result<Self> {
        let db_url = env!("DATABASE_URL");
        if !Sqlite::database_exists(db_url).await? {
            info!("Creating database {}", db_url);
            Sqlite::create_database(db_url).await?;
        } else {
            info!("Database already exists");
        }
        let db = SqlitePool::connect(db_url).await?;
        let migrations =
            Path::new(env!("CARGO_WORKSPACE_DIR")).join("./crates/video_db/migrations");
        debug!("{migrations:?}");

        sqlx::migrate::Migrator::new(migrations)
            .await?
            .run(&db)
            .await?;

        Ok(Self(db))
    }

    async fn insert(&self, items: Vec<emos_api::video::list::Item>) -> anyhow::Result<()> {
        let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT INTO video (todb_id, tmdb_id, video_id, video_type, video_title, genres) ",
        );

        let need_filter_ids = self
            .exist_todb_ids(items.iter().map(|item| item.todb_id).collect())
            .await?;

        let items = items
            .into_iter()
            .filter(|item| !need_filter_ids.contains(&item.todb_id))
            .collect::<Vec<_>>();

        debug!("exist {}, filtered: {}", need_filter_ids.len(), items.len());

        if items.is_empty() {
            return Ok(());
        }

        query_builder.push_values(items, |mut b, item| {
            b.push_bind(item.todb_id)
                .push_bind(item.tmdb_id)
                .push_bind(item.video_id)
                .push_bind(item.video_type)
                .push_bind(item.video_title)
                .push_bind(Json(item.genres)); // 序列化 JSON 存入 TEXT 字段
        });

        let query = query_builder.build();
        query.execute(&self.0).await?;

        Ok(())
    }

    async fn exist_todb_ids(&self, todb_ids: Vec<i64>) -> anyhow::Result<Vec<i64>> {
        let id_str = todb_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");

        query_scalar(&format!(
            "select todb_id from video where todb_id in ({})",
            id_str
        ))
        .fetch_all(&self.0)
        .await
        .map_err(Into::into)
    }

    async fn fetch_all_videos(&self) -> impl Stream<Item = Result<Vec<Item>>> {
        let page_size = 100;

        stream! {
            let api = video::list::EmosApi::new()?;
            let mut page = 1;

            loop {
                let resp = api.list(&QueryParams {
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

                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }

    pub async fn task(&self) -> Result<()> {
        let stream = self.fetch_all_videos().await;
        pin_mut!(stream);

        while let Some(items) = stream.next().await {
            let items = items?;
            self.insert(items).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tracing::Level;

    use super::*;

    #[tokio::test]
    async fn test_video_table() -> anyhow::Result<()> {
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .init();

        let v = VideoTable::new().await?;
        v.task().await?;
        Ok(())
    }
}
