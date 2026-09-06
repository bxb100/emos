use anyhow::Result;
use rand::rng;
use rand::seq::IteratorRandom;
use tmdb_client::Client as TmdbClient;
use tmdb_client::media::Page;
use tmdb_client::movie::Movie;
use tmdb_client::tv::TvSeries;
use tracing::debug;

use crate::files::download_cover_images;
use crate::files::workspace_root;
use crate::ranking::normalize_score;
use crate::watchlist::MediaType;
use crate::watchlist::WatchlistVideo;
use crate::watchlist::write_dynamic_watchlist;

macro_rules! load_all {
    ($api:expr, $fun:expr, $type:ty) => {{
        let mut page = 1;
        let mut result: Vec<$type> = vec![];

        loop {
            let res: Page<$type> = $fun($api, Some(page)).await?;
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

pub(crate) async fn generate_scifi_watchlist(should_download_posters: bool) -> Result<()> {
    let api = TmdbClient::from_env()?;

    let tv = load_all!(&api, TmdbClient::high_rated_scifi_tv, TvSeries);
    let movie = load_all!(&api, TmdbClient::high_rated_scifi_movie, Movie);

    debug!("tv: {:?}, movie: {:?}", tv, movie);

    if should_download_posters {
        download_posters(&tv, &movie).await?;
    }

    write_watchlist(&tv, &movie).await?;

    Ok(())
}

async fn download_posters(tv: &[TvSeries], movie: &[Movie]) -> Result<()> {
    let posters = tv
        .iter()
        .filter_map(|t| t.poster_path.as_ref())
        .chain(movie.iter().filter_map(|m| m.poster_path.as_ref()))
        .sample(&mut rng(), 10);

    // https://developer.themoviedb.org/docs/image-basics
    let imgs = posters
        .iter()
        // poster_path like `/gajva2L0rPYkEWjzgFlBXCAVBE5.jpg`
        .map(|p| format!("{}{}", tmdb_client::IMAGE_BASE_URL, p))
        .collect::<Vec<_>>();

    download_cover_images(imgs, &workspace_root().join("data/covers/scifi"), true).await?;
    Ok(())
}

async fn write_watchlist(tv: &[TvSeries], movie: &[Movie]) -> Result<()> {
    let filename = "tmdb_scifi.json";

    let mut videos = tv
        .iter()
        .enumerate()
        .map(|(i, s)| WatchlistVideo {
            tmdb_id: s.id,
            tmdb_type: MediaType::Tv,
            title: s.name.to_string(),
            sort: normalize_score(i as i64, 0, tv.len() as i64),
        })
        .chain(movie.iter().enumerate().map(|(i, s)| WatchlistVideo {
            tmdb_id: s.id,
            tmdb_type: MediaType::Movie,
            title: s.title.to_string(),
            sort: normalize_score(i as i64, 0, movie.len() as i64),
        }))
        .collect::<Vec<_>>();

    videos.sort_by_key(|m| m.sort);

    write_dynamic_watchlist(
        filename,
        "TMDB 科幻",
        "https://media.githubusercontent.com/media/bxb100/emos/refs/heads/main/data/covers/scifi.png",
        videos,
    )?;

    Ok(())
}

#[tokio::test]
async fn test_load_all() {
    generate_scifi_watchlist(false).await.unwrap();
}
