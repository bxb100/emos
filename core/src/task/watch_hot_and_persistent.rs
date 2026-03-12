use emos_api::watch::dynamic::Media;
use emos_api::watch::dynamic::MediaType::Movie;
use emos_api::watch::dynamic::MediaType::Tv;
use emos_api::watch::dynamic::generate_dynamic_binding_file;
use emos_task_macro::add_task;
use emos_tmdb_api::TmdbApi;
use tracing::info;

#[add_task("watch_hot_and_persistent")]
pub async fn run() -> anyhow::Result<()> {
    let filename = "watch_hot_and_persistent.json";
    let videos = watch_tmdb_hot().await?;

    generate_dynamic_binding_file(
        filename,
        "TMDB 热门",
        "https://raw.githubusercontent.com/bxb100/emos/refs/heads/main/data/covers/tmdb_hot.jpg",
        videos,
    )?;

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
