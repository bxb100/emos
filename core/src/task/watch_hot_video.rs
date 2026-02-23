use std::ops::Deref;
use std::sync::Arc;

use anyhow::bail;
use emos_api::watch::BatchType;
use emos_api::watch::UpdateWatchVideoBatchItem;
use emos_cache::Cache;
use emos_douban_api::DoubanApi;
use emos_douban_api::model::top_list::SubjectCollectionItem;
use emos_douban_api::model::top_list::TopList;
use emos_tmdb_api::TmdbApi;
use emos_tmdb_api::model::MediaItem::Movie;
use emos_tmdb_api::model::MediaItem::Tv;
use futures_util::StreamExt;
use futures_util::stream;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use tracing::error;
use tracing::info;

use crate::ArgMatches;
use crate::add_task;

add_task!("watch_hot_video", run, watch_id: String = "watch_id", douban_user_id: String = "douban_user_id");

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
struct CacheData {
    #[serde(rename = "type")]
    pub r#type: BatchType,
    pub value: u64,
}

impl From<&CacheData> for UpdateWatchVideoBatchItem {
    fn from(value: &CacheData) -> Self {
        UpdateWatchVideoBatchItem {
            r#type: value.r#type,
            value: value.value.to_string(),
        }
    }
}

type SimpleCache = Cache<String, Vec<CacheData>>;
struct App {
    tmdb_api: Arc<TmdbApi>,
    title_regex: Arc<Regex>,
    cache: Arc<SimpleCache>,
}

pub async fn run(watch_id: String, douban_user_id: String) -> anyhow::Result<()> {
    let mut data = get_douban_video(Some(douban_user_id)).await?;

    let tmdb_api = TmdbApi::new()?;
    get_tmdb_video(&tmdb_api).await?.into_iter().for_each(|id| {
        data.push(CacheData {
            r#type: BatchType::TmdbTv,
            value: id,
        });
    });

    let emos_api = emos_api::EmosApi::new()?;
    // chunk 200 and send
    for batch in data.chunks(200) {
        if let Err(e) = emos_api
            .batch_update_watch_videos(&watch_id, batch.iter().map(Into::into).collect::<Vec<_>>())
            .await
        {
            error!("Failed to update watch videos: {:?}", batch);
            bail!(e);
        }

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }

    Ok(())
}

#[allow(unused_variables)]
async fn load_douban_collection_data(
    api: &DoubanApi,
) -> anyhow::Result<Vec<SubjectCollectionItem>> {
    let mut res: Vec<SubjectCollectionItem> = vec![];
    macro_rules! load_all {
        ($fun:expr) => {{
            let mut start = 0i64;
            let mut total = 0;
            let mut res = vec![];

            loop {
                let data: TopList = $fun(api, Some(start as i32), Some(50)).await?;
                res.extend(data.subject_collection_items.into_iter());
                total += data.count;
                start += data.count;
                if total >= data.total {
                    break;
                }
            }

            info!("{} load {} items", stringify!($fun), res.len());

            res
        }};
    }
    // tv
    res.extend(load_all!(DoubanApi::tv_hot));
    res.extend(load_all!(DoubanApi::tv_chinese_best_weekly));
    res.extend(load_all!(DoubanApi::tv_global_best_weekly));
    // show
    res.extend(load_all!(DoubanApi::show_hot));
    // movie
    res.extend(load_all!(DoubanApi::movie_top250));
    res.extend(load_all!(DoubanApi::movie_scifi));
    res.extend(load_all!(DoubanApi::movie_hot_gaia));
    res.extend(load_all!(DoubanApi::movie_comedy));
    res.extend(load_all!(DoubanApi::movie_action));
    res.extend(load_all!(DoubanApi::movie_love));

    Ok(res)
}

async fn get_douban_video(douban_user_id: Option<String>) -> anyhow::Result<Vec<CacheData>> {
    let api = DoubanApi::new();
    let res = load_douban_collection_data(&api).await?;

    let app = Arc::new(App {
        tmdb_api: Arc::new(TmdbApi::new()?),
        title_regex: Arc::new(Regex::new(r"[第\s]+([0-9一二三四五六七八九十S\-]+)\s*季")?),
        cache: Arc::new(SimpleCache::new()?),
    });
    let app_clone = app.clone();
    let mut video_res = {
        stream::iter(res)
            .filter_map(move |item| {
                let app_clone = app_clone.clone();
                async move {
                    filter_douban_by_cache(app_clone, &item.id, &item.title)
                        .await
                        .ok()
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    };

    let app_clone = app.clone();
    if let Some(douban_user_id) = douban_user_id {
        let interests = api.wish(&douban_user_id, Some(0), Some(200)).await?;
        stream::iter(interests.interests)
            .filter_map(move |i| {
                let app_clone = app_clone.clone();
                async move {
                    if !i.subject.is_show || !i.subject.is_released {
                        return None;
                    }
                    filter_douban_by_cache(app_clone, &i.subject.id, &i.subject.title)
                        .await
                        .ok()
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .for_each(|item| video_res.push(item));
    }

    Ok(video_res)
}

async fn get_tmdb_video(api: &TmdbApi) -> anyhow::Result<Vec<u64>> {
    let mut res = vec![];

    // on purpose to sequentially fetch
    for _page in 1..=5 {
        if let Ok(data) = api.tv_popular(Some(_page)).await {
            res.extend(data.results.iter().map(|s| s.id))
        };
        if let Ok(data) = api.movie_popular(Some(_page)).await {
            res.extend(data.results.iter().map(|s| s.id))
        };

        info!("Fetched {} items", res.len());
    }
    Ok(res)
}

async fn filter_douban_by_cache(
    app: Arc<App>,
    item_id: &str,
    item_title: &str,
) -> anyhow::Result<Vec<CacheData>> {
    let id = format!("douban_video_{}", item_id);
    let cache = app.cache.deref();

    if let Ok(Some(data)) = cache.get(&id) {
        info!("Cache hit {id}, cached: {:?}", data);
        bail!("cache hit");
    }

    let title = app.title_regex.replace(item_title, "");

    let res = app.tmdb_api.search_multi(&title, None).await?;

    info!("search {} in tmdb got {} results", title, res.total_results);

    let v = res
        .results
        .iter()
        .filter(|e| matches!(e, Tv(_) | Movie(_)))
        .take(3)
        .filter_map(|e| match e {
            Tv(t) => Some(CacheData {
                r#type: BatchType::TmdbTv,
                value: t.id,
            }),
            Movie(m) => Some(CacheData {
                r#type: BatchType::TmdbMovie,
                value: m.id,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    info!("Found {} items", v.len());

    cache.set(id, &v)?;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_douban_foreign_tv() {
        let items = get_douban_video(None).await.unwrap();
        println!("{:?}", items);
    }

    #[tokio::test]
    async fn test_get_tmdb_foreign_tv() {
        let api = TmdbApi::new().unwrap();
        let items = get_tmdb_video(&api).await.unwrap();
        println!("{:?}", items);
    }
}
