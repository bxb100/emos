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
            r#"
            SELECT
                v.todb_id as "todb_id!",
                v.tmdb_id as "tmdb_id!",
                v.video_id as "video_id!",
                v.video_type,
                v.video_title,
                v.genres
            FROM video v
            JOIN video_genre vg ON v.todb_id = vg.todb_id
            JOIN genre g ON vg.genre_id = g.id
            WHERE v.todb_id > ? AND g.name = ?
            ORDER BY v.todb_id
            LIMIT ?
            "#,
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

        let mut tx = self.0.begin().await?;

        let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT INTO video (todb_id, tmdb_id, video_id, video_type, video_title, genres) ",
        );

        query_builder.push_values(&items, |mut b, item| {
            b.push_bind(item.todb_id)
                .push_bind(item.tmdb_id)
                .push_bind(item.video_id)
                .push_bind(&item.video_type)
                .push_bind(&item.video_title)
                .push_bind(Json(&item.genres));
        });

        let query = query_builder.build();
        let num = query.execute(&mut *tx).await?.rows_affected();

        // Insert genres
        let mut unique_genres = std::collections::HashMap::new();
        for item in &items {
            for genre in &item.genres {
                unique_genres.insert(genre.id, &genre.name);
            }
        }

        if !unique_genres.is_empty() {
            let mut genre_builder: QueryBuilder<Sqlite> =
                QueryBuilder::new("INSERT OR IGNORE INTO genre (id, name) ");
            genre_builder.push_values(unique_genres.iter(), |mut b, (id, name)| {
                b.push_bind(id).push_bind(name);
            });
            genre_builder.build().execute(&mut *tx).await?;
        }

        // Insert video_genre
        let mut video_genres = Vec::new();
        for item in &items {
            for genre in &item.genres {
                video_genres.push((item.todb_id, genre.id));
            }
        }

        if !video_genres.is_empty() {
            let mut vg_builder: QueryBuilder<Sqlite> =
                QueryBuilder::new("INSERT OR IGNORE INTO video_genre (todb_id, genre_id) ");
            vg_builder.push_values(video_genres, |mut b, (todb_id, genre_id)| {
                b.push_bind(todb_id).push_bind(genre_id);
            });
            vg_builder.build().execute(&mut *tx).await?;
        }

        tx.commit().await?;

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
