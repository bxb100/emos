use douban_client::Client as DoubanClient;
use douban_client::collections::CollectionItem;
use douban_client::collections::CollectionPage;
use douban_client::subject::SubjectKind;
use emos_cache::FileCache;
use regex::Regex;
use tmdb_client::Client as TmdbClient;
use tmdb_client::media::MediaItem::Movie;
use tmdb_client::media::MediaItem::Tv;
use tracing::debug;
use tracing::info;

use crate::files::workspace_root;
use crate::ranking::recency_score;
use crate::watchlist::MediaType;
use crate::watchlist::WatchlistVideo;
use crate::watchlist::write_dynamic_watchlist;

type VideoCache = FileCache<Vec<WatchlistVideo>>;
const DOUBAN_VIDEO_CACHE_KEY_PREFIX: &str = "douban_video_last_air_date_v1";

pub(crate) async fn generate_hot_watchlist(douban_user_id: Option<String>) -> anyhow::Result<()> {
    let mut data = fetch_douban_videos(douban_user_id).await?;

    let tmdb = TmdbClient::from_env()?;
    let tmdb_data = fetch_tmdb_videos(&tmdb).await?;
    debug!("tmdb_data: {:?}", tmdb_data);
    data.extend(tmdb_data);

    {
        let download_cover = |data: &Vec<WatchlistVideo>, t: MediaType| {
            let m = data
                .iter()
                .filter(|m| m.tmdb_type == t)
                .take(5)
                .map(|m| m.tmdb_id.to_string())
                .collect::<Vec<_>>();
            crate::commands::covers::download_covers(t == MediaType::Movie, m, "hot".to_string())
        };

        download_cover(&data, MediaType::Movie).await?;
        download_cover(&data, MediaType::Tv).await?;
    }

    write_dynamic_watchlist(
        "watch_hot_video.json",
        "热门追更",
        "https://media.githubusercontent.com/media/bxb100/emos/refs/heads/main/data/covers/hot.png",
        data,
    )?;

    Ok(())
}

async fn fetch_douban_collections(api: &DoubanClient) -> anyhow::Result<Vec<CollectionItem>> {
    let mut res: Vec<CollectionItem> = vec![];
    macro_rules! load_all {
        ($fun:expr) => {{
            let mut start = 0i64;
            let mut res = vec![];

            loop {
                let data: CollectionPage = $fun(api, Some(start as i32), Some(50)).await?;
                res.extend(data.items.into_iter());
                start += data.count;
                if start >= data.total {
                    break;
                }
            }

            info!("{} load {} items", stringify!($fun), res.len());

            res
        }};
    }
    // tv
    res.extend(load_all!(DoubanClient::tv_hot));
    res.extend(load_all!(DoubanClient::tv_chinese_best_weekly));
    res.extend(load_all!(DoubanClient::tv_global_best_weekly));
    // show
    res.extend(load_all!(DoubanClient::show_hot));
    // movie
    res.extend(load_all!(DoubanClient::movie_top250));
    res.extend(load_all!(DoubanClient::movie_scifi));
    res.extend(load_all!(DoubanClient::movie_hot_gaia));
    res.extend(load_all!(DoubanClient::movie_comedy));
    res.extend(load_all!(DoubanClient::movie_action));
    res.extend(load_all!(DoubanClient::movie_love));

    Ok(res)
}

async fn fetch_douban_videos(
    douban_user_id: Option<String>,
) -> anyhow::Result<Vec<WatchlistVideo>> {
    let api = DoubanClient::new();
    let res = fetch_douban_collections(&api).await?;

    let tmdb = TmdbClient::from_env()?;
    let cache = VideoCache::new(workspace_root().join("data/cache/simple_cache.bin"));
    let mut video_res = Vec::new();
    for item in res {
        if let Ok(videos) = match_douban_video(
            &tmdb,
            &cache,
            item.kind,
            &item.id,
            &item.title,
            item.year.as_ref(),
        )
        .await
        {
            video_res.extend(videos);
        }
    }

    if let Some(douban_user_id) = douban_user_id {
        let interests = api.wish(&douban_user_id, Some(0), Some(200)).await?;
        for interest in interests.interests {
            let subject = interest.subject;
            if !subject.is_show || !subject.is_released {
                continue;
            }
            if let Ok(videos) = match_douban_video(
                &tmdb,
                &cache,
                subject.kind,
                &subject.id,
                &subject.title,
                Some(&subject.year),
            )
            .await
            {
                video_res.extend(videos);
            }
        }
    }

    Ok(video_res)
}

async fn fetch_tmdb_videos(api: &TmdbClient) -> anyhow::Result<Vec<WatchlistVideo>> {
    let mut res = vec![];

    // on purpose to sequentially fetch
    for _page in 1..=5 {
        if let Ok(data) = api.tv_popular(Some(_page)).await {
            res.extend(data.results.iter().map(|s| WatchlistVideo {
                tmdb_id: s.id,
                tmdb_type: MediaType::Tv,
                title: s.name.to_string(),
                sort: 100,
            }))
        };
        if let Ok(data) = api.movie_popular(Some(_page)).await {
            res.extend(data.results.iter().map(|s| WatchlistVideo {
                tmdb_id: s.id,
                tmdb_type: MediaType::Movie,
                title: s.title.to_string(),
                sort: 100,
            }))
        };

        info!("Fetched {} items", res.len());
    }
    Ok(res)
}

async fn match_douban_video(
    tmdb: &TmdbClient,
    cache: &VideoCache,
    kind: SubjectKind,
    item_id: &str,
    item_title: &str,
    year: Option<impl AsRef<str>>,
) -> anyhow::Result<Vec<WatchlistVideo>> {
    let id = format!("{DOUBAN_VIDEO_CACHE_KEY_PREFIX}_{item_id}");

    // empty data fallback to re-fetch
    let videos = if let Ok(Some(data)) = cache.get(&id).await
        && !data.is_empty()
    {
        debug!("Cache hit for {}: {:?}", id, data);
        data
    } else {
        let title = strip_season_suffix(item_title);

        let v = match kind {
            SubjectKind::Movie => {
                let res = tmdb.search_movie(&title, year, None).await?;
                info!("Movie {id} {title} found {}", res.total_results);
                res.results
                    .iter()
                    .map(|m| WatchlistVideo {
                        tmdb_id: m.id,
                        tmdb_type: MediaType::Movie,
                        title: m.title.to_owned(),
                        sort: recency_score(m.release_date.as_ref()),
                    })
                    .collect::<Vec<_>>()
            }
            SubjectKind::Tv => {
                let res = tmdb.search_tv(&title, year, None).await?;
                info!("TV {id} {title} found {}", res.total_results);
                let mut videos = Vec::with_capacity(res.results.len());
                for m in &res.results {
                    videos.push(WatchlistVideo {
                        tmdb_id: m.id,
                        tmdb_type: MediaType::Tv,
                        title: m.name.to_owned(),
                        sort: tv_recency_score(tmdb, m.id, m.first_air_date.as_ref()).await,
                    });
                }
                videos
            }
            SubjectKind::Unknown(s) => {
                let res = tmdb.search_multi(&title, None).await?;
                info!("Unknown {id} {s} found {}", res.total_results);
                let mut videos = Vec::with_capacity(res.results.len());
                for item in &res.results {
                    match item {
                        Tv(t) => {
                            videos.push(WatchlistVideo {
                                tmdb_id: t.id,
                                tmdb_type: MediaType::Tv,
                                title: t.name.clone(),
                                sort: tv_recency_score(tmdb, t.id, t.first_air_date.as_ref()).await,
                            });
                        }
                        Movie(m) => videos.push(WatchlistVideo {
                            tmdb_id: m.id,
                            tmdb_type: MediaType::Movie,
                            title: m.title.clone(),
                            sort: recency_score(m.release_date.as_ref()),
                        }),
                        _ => {}
                    }
                }
                videos
            }
        };

        debug!("{id} found {v:?}");
        cache.set(id, &v).await?;
        v
    };
    Ok(videos.into_iter().take(2).collect())
}

async fn tv_recency_score(
    tmdb: &TmdbClient,
    tmdb_id: u64,
    fallback_first_air_date: Option<&String>,
) -> i64 {
    match tmdb.tv_details(tmdb_id).await {
        Ok(details) => recency_score(Some(details.last_air_date)),
        Err(_) => recency_score(fallback_first_air_date),
    }
}

#[inline]
fn strip_season_suffix(title: &str) -> String {
    let re = Regex::new(r"[第\s]+[0-9一二三四五六七八九十S\-]+\s*季[\s\w]*").unwrap();
    let res = re.replace(title, "").to_string();
    // min 2 chinese chars
    if res.len() <= 6 {
        return res;
    }

    let mut chars = res.chars().rev();
    if match (chars.next(), chars.next()) {
        (Some(last), Some(sec_last)) => {
            last.is_ascii_digit() && !(sec_last.is_ascii_digit() || sec_last == '.')
        }
        _ => false,
    } {
        res.trim_end_matches(char::is_numeric).to_string()
    } else {
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_douban_foreign_tv() {
        let items = fetch_douban_videos(None).await.unwrap();
        println!("{:?}", items);
    }

    #[tokio::test]
    async fn test_get_tmdb_foreign_tv() {
        let api = TmdbClient::from_env().unwrap();
        let items = fetch_tmdb_videos(&api).await.unwrap();
        println!("{:?}", items);
    }

    #[test]
    fn test_regex() {
        assert_eq!(
            strip_season_suffix("【我推的孩子】 第三季"),
            "【我推的孩子】"
        );
        assert_eq!(strip_season_suffix("辐射 第二季 Fallout Season 2"), "辐射");
        assert_eq!(strip_season_suffix("御赐小仵作2"), "御赐小仵作");
        assert_eq!(strip_season_suffix("有歌2026"), "有歌2026");
        assert_eq!(strip_season_suffix("伟大的导游2.5"), "伟大的导游2.5");
        assert_eq!(strip_season_suffix("x1"), "x1");
    }
}
