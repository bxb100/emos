use chrono::Local;
use chrono_tz::Asia::Shanghai;
use emos_api::watch::dynamic::Dynamic;
use emos_api::watch::dynamic::Media;
use emos_api::watch::dynamic::MediaType::Movie;
use emos_api::watch::dynamic::MediaType::Tv;
use emos_tmdb_api::TmdbApi;
use emos_utils::fs::project_root;
use tracing::info;

use crate::add_task;

add_task!("watch_hot_and_persistent", run);

pub async fn run() -> anyhow::Result<()> {
    let file_name = "watch_hot_and_persistent.json";
    let videos = watch_tmdb_hot().await?;

    let data = Dynamic {
        name: "热门追更".to_string(),
        cover: "https://raw.githubusercontent.com/bxb100/emos/refs/heads/main/data/cover.png"
            .to_string(),
        updated_at: Local::now().with_timezone(&Shanghai),
        videos,
    };

    let path = project_root().join("data").join(file_name);
    tokio::fs::write(&path, serde_json::to_string(&data)?).await?;

    Ok(())
}

async fn watch_tmdb_hot() -> anyhow::Result<Vec<Media>> {
    let api = TmdbApi::new()?;
    let mut res = vec![];

    // on purpose to sequentially fetch
    for _page in 1..=5 {
        if let Ok(data) = api.tv_popular(Some(_page)).await {
            res.extend(data.results.iter().map(|s| Media {
                tmdb_id: s.id,
                tmdb_type: Tv,
                title: s.name.to_string(),
                sort: 100,
            }))
        };
        if let Ok(data) = api.movie_popular(Some(_page)).await {
            res.extend(data.results.iter().map(|s| Media {
                tmdb_id: s.id,
                tmdb_type: Movie,
                title: s.title.to_string(),
                sort: 100,
            }))
        };

        info!("Fetched {} items", res.len());
    }
    Ok(res)
}
