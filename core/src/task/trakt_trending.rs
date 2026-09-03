use anyhow::Result;
use emos_api::watch::dynamic::Media;
use emos_api::watch::dynamic::MediaType;
use emos_api::watch::dynamic::generate_dynamic_binding_file;
use task_macro::add_task;
use trakt_api::TraktApi;
use trakt_api::model::TrendingItem;
use utils::math::normalize_to_1_5000;

const OUTPUT_FILE: &str = "trakt_trending.json";
const COVER_NAMESPACE: &str = "trakt";
const COVER_URL: &str =
    "https://media.githubusercontent.com/media/bxb100/emos/refs/heads/main/data/covers/trakt.png";

/// Builds a standalone dynamic list from <https://app.trakt.tv/discover/trending>.
#[add_task("trakt_trending")]
pub async fn task(download_cover: bool) -> Result<()> {
    let api = TraktApi::new()?;
    let items = api.all_trending().await?;
    let media = trakt_trending_to_media(&items);

    tracing::info!(
        fetched = items.len(),
        converted = media.len(),
        skipped_without_tmdb_id = items.len() - media.len(),
        "Fetched Trakt trending media"
    );

    if download_cover {
        crate::task::tmdb_download_cover::download_media_posters(&media, COVER_NAMESPACE).await?;
    }

    generate_dynamic_binding_file(OUTPUT_FILE, "Trakt 热门趋势", COVER_URL, media)
}

fn trakt_trending_to_media(items: &[TrendingItem]) -> Vec<Media> {
    let ranked_items = items
        .iter()
        .filter_map(|item| {
            let (title, tmdb_id, tmdb_type) = match item {
                TrendingItem::Movie { movie, .. } => {
                    (&movie.title, movie.ids.tmdb, MediaType::Movie)
                }
                TrendingItem::Show { show, .. } => (&show.title, show.ids.tmdb, MediaType::Tv),
            };

            Some((title.clone(), tmdb_id?, tmdb_type))
        })
        .collect::<Vec<_>>();
    let last_rank = ranked_items.len().saturating_sub(1) as i64;

    let last_rank = last_rank + 1;
    ranked_items
        .into_iter()
        .enumerate()
        .map(|(rank, (title, tmdb_id, tmdb_type))| Media {
            tmdb_id,
            tmdb_type,
            title,
            sort: normalize_to_1_5000((rank + 1) as i64, 0, last_rank),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use trakt_api::model::MediaIds;

    use super::*;

    fn trakt_ids(trakt: u64, tmdb: Option<u64>) -> MediaIds {
        MediaIds {
            trakt,
            slug: format!("item-{trakt}"),
            imdb: None,
            tmdb,
        }
    }

    #[test]
    fn maps_mixed_items_and_skips_missing_tmdb_ids() {
        let items = vec![
            TrendingItem::Movie {
                watchers: 30,
                movie: trakt_api::model::Movie {
                    title: "Movie".to_string(),
                    year: Some(2026),
                    ids: trakt_ids(1, Some(101)),
                },
            },
            TrendingItem::Movie {
                watchers: 20,
                movie: trakt_api::model::Movie {
                    title: "Missing TMDB".to_string(),
                    year: Some(2025),
                    ids: trakt_ids(2, None),
                },
            },
            TrendingItem::Show {
                watchers: 10,
                show: trakt_api::model::Show {
                    title: "Show".to_string(),
                    year: Some(2024),
                    ids: trakt_ids(3, Some(303)),
                },
            },
        ];

        let media = trakt_trending_to_media(&items);

        assert_eq!(media.len(), 2);
        assert_eq!(media[0].tmdb_id, 101);
        assert_eq!(media[0].tmdb_type, MediaType::Movie);
        assert_eq!(media[0].title, "Movie");
        assert_eq!(media[0].sort, 1);
        assert_eq!(media[1].tmdb_id, 303);
        assert_eq!(media[1].tmdb_type, MediaType::Tv);
        assert_eq!(media[1].title, "Show");
        assert_eq!(media[1].sort, 100);
    }

    #[test]
    fn handles_empty_and_single_item_rankings() {
        assert!(trakt_trending_to_media(&[]).is_empty());

        let items = vec![TrendingItem::Show {
            watchers: 1,
            show: trakt_api::model::Show {
                title: "Only Show".to_string(),
                year: None,
                ids: trakt_ids(1, Some(404)),
            },
        }];

        let media = trakt_trending_to_media(&items);
        assert_eq!(media.len(), 1);
        assert_eq!(media[0].sort, 1);
    }

    #[test]
    fn reindexes_after_skipping_unmatched_items() {
        let items = vec![
            TrendingItem::Movie {
                watchers: 99,
                movie: trakt_api::model::Movie {
                    title: "Skipped".to_string(),
                    year: Some(2026),
                    ids: trakt_ids(1, None),
                },
            },
            TrendingItem::Show {
                watchers: 88,
                show: trakt_api::model::Show {
                    title: "First visible".to_string(),
                    year: Some(2025),
                    ids: trakt_ids(2, Some(202)),
                },
            },
            TrendingItem::Movie {
                watchers: 77,
                movie: trakt_api::model::Movie {
                    title: "Second visible".to_string(),
                    year: Some(2024),
                    ids: trakt_ids(3, Some(303)),
                },
            },
        ];

        let media = trakt_trending_to_media(&items);

        assert_eq!(media.len(), 2);
        assert_eq!(media[0].title, "First visible");
        assert_eq!(media[0].sort, 1);
        assert_eq!(media[1].title, "Second visible");
        assert_eq!(media[1].sort, 100);
    }
}
