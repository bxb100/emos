use anyhow::Result;
use emos_api::video;
use emos_utils::SqlInClause;
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
    pub video_type: Option<String>,
    pub video_title: Option<String>,
    pub genres: Option<String>,
}

impl Dao {
    pub async fn find_all_by_genre(
        &self,
        todb_id: i64,
        genre_name: &str,
        limit: u32,
    ) -> Result<Vec<Video>> {
        let data = query_as!(
            Video,
            r"select video.* from video, json_each(genres) where todb_id > ? and json_extract(json_each.value, '$.name') = ? order by todb_id limit ?",
            todb_id,
            genre_name,
            limit
        )
        .fetch_all(&self.0)
        .await?;

        Ok(data)
    }

    pub async fn insert(&self, items: Vec<video::list::Item>) -> Result<u64> {
        if items.is_empty() {
            return Ok(0);
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
        let num = query.execute(&self.0).await?.rows_affected();

        Ok(num)
    }

    pub async fn exist_todb_ids(&self, todb_ids: Vec<i64>) -> Result<Vec<i64>> {
        let id_str = todb_ids.to_sql_in_clause()?;
        // https://github.com/launchbadge/sqlx/blob/main/FAQ.md#how-can-i-do-a-select--where-foo-in--query
        query_scalar(&format!(
            "select todb_id from video where todb_id in ({id_str})"
        ))
        .fetch_all(&self.0)
        .await
        .map_err(Into::into)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Vec<Video>> {
        let name = name.trim().to_owned() + "%";
        let data = query_as!(Video, r"select * from video where video_title like ?", name)
            .fetch_all(&self.0)
            .await?;

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    pub async fn test_find_cartoon() -> Result<()> {
        let dao = Dao::new().await?;
        let videos = dao.find_all_by_genre(-1, "动画", 10).await?;
        for video in videos {
            println!("{:#?}", video);
        }
        Ok(())
    }
}
