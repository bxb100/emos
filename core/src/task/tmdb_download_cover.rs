use std::sync::Arc;

use anyhow::Result;
use emos_api::watch::dynamic::Media;
use emos_api::watch::dynamic::MediaType;
use futures_util::future::join_all;
use task_macro::add_task;
use tmdb_api::TmdbApi;
use tracing::info;
use utils::fs::batch_download_imgs;
use utils::fs::project_root;

const MAX_POSTERS: usize = 10;

#[derive(Debug, PartialEq, Eq)]
struct CoverCandidate {
    is_movie: bool,
    tmdb_id: String,
}

#[add_task("tmdb_download_cover", rename(tmdb_id = "id"))]
pub async fn task(video: bool, tmdb_id: Vec<String>, namespace: String) -> Result<()> {
    let candidates = tmdb_id
        .into_iter()
        .map(|tmdb_id| CoverCandidate {
            is_movie: video,
            tmdb_id,
        })
        .collect();

    download_candidates(candidates, &namespace).await
}

pub(crate) async fn download_media_posters(media: &[Media], namespace: &str) -> Result<()> {
    download_candidates(media_cover_candidates(media), namespace).await
}

fn media_cover_candidates(media: &[Media]) -> Vec<CoverCandidate> {
    media
        .iter()
        .take(MAX_POSTERS)
        .map(|media| CoverCandidate {
            is_movie: media.tmdb_type == MediaType::Movie,
            tmdb_id: media.tmdb_id.to_string(),
        })
        .collect()
}

async fn download_candidates(candidates: Vec<CoverCandidate>, namespace: &str) -> Result<()> {
    let api = Arc::new(TmdbApi::new()?);

    let dest_dir = project_root().join("data/covers").join(namespace);
    std::fs::create_dir_all(&dest_dir)?;

    let data = candidates
        .into_iter()
        .map(|candidate| {
            let api = api.clone();
            async move {
                if candidate.is_movie {
                    let movie = api.get_movie(&candidate.tmdb_id).await?;
                    info!("Found movie: {} (id: {})", movie.title, movie.id);
                    Ok::<Option<String>, anyhow::Error>(movie.poster_path)
                } else {
                    let tv = api.get_tv(&candidate.tmdb_id).await?;
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
        .map(|p| format!("{}{}", tmdb_api::IMAGE_BASE_URL, p))
        .collect::<Vec<_>>();
    let poster_count = posters.len();

    batch_download_imgs(posters, &dest_dir, true).await?;

    info!("Downloaded {poster_count} posters to {dest_dir:?}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trakt_cover_candidates_preserve_mixed_rank_and_cap_at_ten() {
        let media = (0..12)
            .map(|rank| Media {
                tmdb_id: 100 + rank,
                tmdb_type: if rank % 2 == 0 {
                    MediaType::Movie
                } else {
                    MediaType::Tv
                },
                title: format!("Item {rank}"),
                sort: rank as i64,
            })
            .collect::<Vec<_>>();

        let candidates = media_cover_candidates(&media);

        assert_eq!(candidates.len(), MAX_POSTERS);
        assert_eq!(
            candidates,
            (0..10)
                .map(|rank| CoverCandidate {
                    is_movie: rank % 2 == 0,
                    tmdb_id: (100 + rank).to_string(),
                })
                .collect::<Vec<_>>()
        );
    }
}
