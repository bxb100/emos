use std::sync::Arc;

use anyhow::Result;
use emos_api::watch::dynamic::Media;
use emos_api::watch::dynamic::MediaType;
use emos_api::watch::dynamic::generate_dynamic_binding_file;
use emos_tmdb_api::TmdbApi;
use emos_tmdb_api::model::Movie;
use emos_tmdb_api::model::PagedResult;
use emos_tmdb_api::model::Tv;
use emos_utils::fs::batch_download_imgs;
use emos_utils::fs::project_root;
use emos_utils::math::normalize_to_1_100;
use tracing::debug;

use crate::add_task;

add_task!("tmdb_scifi_media", task);

macro_rules! load_all {
    ($api:expr, $fun:expr, $type:ty) => {{
        let mut page = 1;
        let mut result: Vec<$type> = vec![];

        loop {
            let res: PagedResult<$type> = $fun($api, Some(page)).await?;
            if page >= res.total_pages {
                break;
            }
            if res.results.is_empty() {
                break;
            }
            result.extend(res.results);
            page += 1;
        }

        tracing::info!("{} load {} items", stringify!($fun), result.len());

        result
    }};
}

pub async fn task() -> Result<()> {
    let api = TmdbApi::new()?;

    let tv = load_all!(&api, TmdbApi::high_rated_scifi_tv, Tv);
    let movie = load_all!(&api, TmdbApi::high_rated_scifi_movie, Movie);

    debug!("tv: {:?}, movie: {:?}", tv, movie);

    let tv = Arc::new(tv);
    let movie = Arc::new(movie);

    {
        let tv = Arc::clone(&tv);
        let movie = Arc::clone(&movie);
        tokio::spawn(async move { download_posters(tv, movie).await }).await??;
    };

    {
        let tv = Arc::clone(&tv);
        let movie = Arc::clone(&movie);
        tokio::spawn(async move { to_json(tv, movie).await }).await??;
    };

    Ok(())
}

async fn download_posters(tv: Arc<Vec<Tv>>, movie: Arc<Vec<Movie>>) -> Result<()> {
    let posters = tv
        .iter()
        .filter(|m| m.poster_path.is_some())
        .take(5)
        .filter_map(|m| m.poster_path.as_ref())
        .chain(
            movie
                .iter()
                .filter(|m| m.poster_path.is_some())
                .take(5)
                .filter_map(|m| m.poster_path.as_ref()),
        )
        .collect::<Vec<_>>();

    // https://developer.themoviedb.org/docs/image-basics
    let base_url = "https://image.tmdb.org/t/p/original";
    let imgs = posters
        .iter()
        // poster_path like `/gajva2L0rPYkEWjzgFlBXCAVBE5.jpg`
        .map(|p| format!("{}{}", base_url, p))
        .collect::<Vec<_>>();

    batch_download_imgs(imgs, &project_root().join("data/covers/scifi")).await?;
    Ok(())
}

async fn to_json(tv: Arc<Vec<Tv>>, movie: Arc<Vec<Movie>>) -> Result<()> {
    let filename = "tmdb_scifi.json";

    let mut videos = tv
        .iter()
        .enumerate()
        .map(|(i, s)| Media {
            tmdb_id: s.id,
            tmdb_type: MediaType::Tv,
            title: s.name.to_string(),
            sort: normalize_to_1_100(i, 0, tv.len()),
        })
        .chain(movie.iter().enumerate().map(|(i, s)| Media {
            tmdb_id: s.id,
            tmdb_type: MediaType::Movie,
            title: s.title.to_string(),
            sort: normalize_to_1_100(i, 0, movie.len()),
        }))
        .collect::<Vec<_>>();

    videos.sort_by_key(|m| m.sort);

    generate_dynamic_binding_file(
        filename,
        "TMDB 科幻",
        "https://media.githubusercontent.com/media/bxb100/emos/refs/heads/main/data/covers/scifi.png",
        videos,
    )?;

    Ok(())
}

#[tokio::test]
async fn test_load_all() {
    task().await.unwrap();
}
