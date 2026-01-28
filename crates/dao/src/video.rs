use anyhow::Result;
use async_stream::stream;
use std::fmt::Write;
use emos_api::video;
use emos_api::video::list::Item;
use emos_api::video::list::QueryParams;
use futures_util::Stream;
use serde::Deserialize;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::query_as;
use sqlx::query_scalar;
use sqlx::types::Json;

use crate::Dao;

#[derive(Debug, Deserialize)]
pub struct Video {
    pub todb_id: i64,
    pub tmdb_id: i64,
    pub video_id: i64,
    video_type: Option<String>,
    video_title: Option<String>,
    genres: Option<String>,
}

impl Dao {
    pub async fn find_all_by_genre(&self, todb_id: i64, genre_name: &str) -> Result<Vec<Video>> {
        let data = query_as!(
            Video,
            r"select video.* from video, json_each(genres) where todb_id > ? and json_extract(json_each.value, '$.name') = ? order by todb_id limit ?",
            todb_id,
            genre_name,
            500
        )
        .fetch_all(&self.0)
        .await?;

        Ok(data)
    }

    pub async fn insert(&self, items: Vec<emos_api::video::list::Item>) -> anyhow::Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT INTO video (todb_id, tmdb_id, video_id, video_type, video_title, genres) ",
        );

        query_builder.push_values(items, |mut b, item| {
            b.push_bind(item.todb_id)
                .push_bind(item.tmdb_id)
                .push_bind(item.video_id)
                .push_bind(item.video_type)
                .push_bind(item.video_title)
                .push_bind(Json(item.genres));
        });

        let query = query_builder.build();
        query.execute(&self.0).await?;

        Ok(())
    }

    pub async fn exist_todb_ids(&self, todb_ids: Vec<i64>) -> anyhow::Result<Vec<i64>> {
        let mut id_str = String::with_capacity(todb_ids.len() * 10);
        for (i, id) in todb_ids.iter().enumerate() {
            if i > 0 {
                id_str.push(',');
            }
            write!(id_str, "{}", id)?;
        }

        query_scalar(&format!(
            "select todb_id from video where todb_id in ({})",
            id_str
        ))
        .fetch_all(&self.0)
        .await
        .map_err(Into::into)
    }

    pub async fn fetch_all_videos(&self) -> impl Stream<Item = anyhow::Result<Vec<Item>>> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    pub async fn test_find_cartoon() -> Result<()> {
        let dao = Dao::new().await?;
        let videos = dao.find_all_by_genre(-1, "动画").await?;
        for video in videos {
            println!("{:#?}", video);
        }
        Ok(())
    }

    #[tokio::test]
    pub async fn test_exist_todb_ids() -> Result<()> {
        let dao = Dao::new().await?;
        let test_id = 999999;
        let item = Item {
            todb_id: test_id,
            tmdb_id: 1,
            video_id: 1,
            video_type: "movie".to_string(),
            video_title: "Test Video".to_string(),
            genres: vec![],
            ..Default::default()
        };

        // Ensure clean state or just ignore insert error if exists (for repeated runs)
        // Since we don't have a clear delete/clean, we try to insert.
        // If it fails due to PK, we assume it's there.
        let _ = dao.insert(vec![item]).await;

        let found = dao.exist_todb_ids(vec![test_id]).await?;
        assert!(found.contains(&test_id));

        Ok(())
    }
}
