pub mod video;

use std::path::Path;

use sqlx::Sqlite;
use sqlx::SqlitePool;
use sqlx::migrate;
use sqlx::migrate::MigrateDatabase;
use tracing::debug;
use tracing::info;

pub struct Dao(SqlitePool);

impl Dao {
    pub async fn new() -> anyhow::Result<Self> {
        let db_url = env!("DATABASE_URL");
        if !Sqlite::database_exists(db_url).await? {
            info!("Creating database {}", db_url);
            Sqlite::create_database(db_url).await?;
        } else {
            info!("Database already exists");
        }
        let db = SqlitePool::connect(db_url).await?;
        let migrations = Path::new(env!("MIGRATIONS_DIR"));
        debug!("{migrations:?}");

        migrate::Migrator::new(migrations).await?.run(&db).await?;

        Ok(Self(db))
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

        let _ = Dao::new().await?;
        Ok(())
    }
}
