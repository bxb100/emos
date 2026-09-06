use std::path::Path;

use anyhow::Result;
use emos_client::video::Video as RemoteVideo;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::SqlitePool;
use sqlx::migrate;
use sqlx::migrate::MigrateDatabase;
use sqlx::query_as;
use sqlx::query_scalar;
use sqlx::types::Json;
use tracing::debug;
use tracing::info;

use crate::Video;

/// Access to the local SQLite video catalog.
pub struct VideoRepository {
    pool: SqlitePool,
}

impl VideoRepository {
    /// Opens the database configured at build time and applies pending migrations.
    pub async fn open() -> Result<Self> {
        let db_url = env!("DATABASE_URL");
        if !Sqlite::database_exists(db_url).await? {
            info!("Creating database {}", db_url);
            Sqlite::create_database(db_url).await?;
        } else {
            info!("Database already exists");
        }
        let pool = SqlitePool::connect(db_url).await?;
        let migrations = Path::new(env!("MIGRATIONS_DIR"));
        debug!("{migrations:?}");

        migrate::Migrator::new(migrations).await?.run(&pool).await?;

        Ok(Self { pool })
    }

    /// Finds videos of a genre after the given ID, ordered by ascending ID.
    pub async fn find_by_genre_after(
        &self,
        after_todb_id: i64,
        genre_name: &str,
        limit: u32,
    ) -> Result<Vec<Video>> {
        let data = query_as!(
            Video,
            r"select video.* from video, json_each(genres) where todb_id > ? and json_extract(json_each.value, '$.name') = ? order by todb_id limit ?",
            after_todb_id,
            genre_name,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(data)
    }

    /// Inserts remote videos and returns the number of affected rows.
    pub async fn insert_videos(&self, videos: Vec<RemoteVideo>) -> Result<u64> {
        if videos.is_empty() {
            return Ok(0);
        }

        let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT INTO video (todb_id, tmdb_id, video_id, video_type, video_title, genres) ",
        );

        query_builder.push_values(videos, |mut row, video| {
            row.push_bind(video.todb_id)
                .push_bind(video.tmdb_id)
                .push_bind(video.video_id)
                .push_bind(video.video_type)
                .push_bind(video.video_title)
                .push_bind(Json(video.genres));
        });

        let query = query_builder.build();
        let num = query.execute(&self.pool).await?.rows_affected();

        Ok(num)
    }

    /// Returns the supplied IDs already in the catalog; the input must be nonempty.
    pub async fn existing_todb_ids(&self, todb_ids: Vec<i64>) -> Result<Vec<i64>> {
        if todb_ids.is_empty() {
            anyhow::bail!("Cannot create SQL IN clause from empty vector");
        }
        let id_str = todb_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        // https://github.com/launchbadge/sqlx/blob/main/FAQ.md#how-can-i-do-a-select--where-foo-in--query
        query_scalar(&format!(
            "select todb_id from video where todb_id in ({id_str})"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// Finds titles matching a trimmed prefix using SQLite's `LIKE` semantics.
    pub async fn find_by_title_prefix(&self, title_prefix: &str) -> Result<Vec<Video>> {
        let title_pattern = title_prefix.trim().to_owned() + "%";
        let data = query_as!(
            Video,
            r"select * from video where video_title like ?",
            title_pattern
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use tracing::Level;

    use super::*;

    #[tokio::test]
    async fn open_video_catalog() -> anyhow::Result<()> {
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .init();

        let _ = VideoRepository::open().await?;
        Ok(())
    }

    #[tokio::test]
    async fn find_cartoon_videos() -> Result<()> {
        let repository = VideoRepository::open().await?;
        let videos = repository.find_by_genre_after(-1, "动画", 10).await?;
        for video in videos {
            println!("{:#?}", video);
        }
        Ok(())
    }
}
