use std::sync::Arc;

use anyhow::Result;
use emos_task_macro::add_task;
use emos_tmdb_api::TmdbApi;
use emos_utils::fs::batch_download_imgs;
use emos_utils::fs::project_root;
use futures_util::future::join_all;
use tracing::info;

#[add_task("tmdb_download_cover", rename(tmdb_id = "id"))]
pub async fn task(video: bool, tmdb_id: Vec<String>, namespace: String) -> Result<()> {
    let api = Arc::new(TmdbApi::new()?);

    let dest_dir = project_root().join("data/covers").join(namespace);
    std::fs::create_dir_all(&dest_dir)?;

    let data = tmdb_id
        .iter()
        .map(|id| {
            let api = api.clone();
            async move {
                if video {
                    let movie = api.get_movie(id).await?;
                    info!("Found movie: {} (id: {})", movie.title, movie.id);
                    Ok::<Option<String>, anyhow::Error>(movie.poster_path)
                } else {
                    let tv = api.get_tv(id).await?;
                    info!("Found TV: {} (id: {})", tv.name, tv.id);
                    Ok::<Option<String>, anyhow::Error>(tv.poster_path)
                }
            }
        })
        .collect::<Vec<_>>();

    let posters = join_all(data)
        .await
        .into_iter()
        .filter_map(Result::ok)
        .flatten()
        .map(|p| format!("{}{}", emos_tmdb_api::IMAGE_BASE_URL, p))
        .collect::<Vec<_>>();

    batch_download_imgs(posters, &dest_dir).await?;

    info!(
        "Downloaded poster for {} {:?} to {:?}",
        video, tmdb_id, dest_dir
    );

    Ok(())
}
