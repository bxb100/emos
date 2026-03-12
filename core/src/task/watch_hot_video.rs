use std::ops::Deref;
use std::sync::Arc;

use anyhow::bail;
use emos_api::watch::dynamic::Media;
use emos_api::watch::dynamic::MediaType;
use emos_api::watch::dynamic::generate_dynamic_binding_file;
use emos_cache::Cache;
use emos_douban_api::DoubanApi;
use emos_douban_api::model::TypeField;
use emos_douban_api::model::top_list::SubjectCollectionItem;
use emos_douban_api::model::top_list::TopList;
use emos_task_macro::add_task;
use emos_tmdb_api::TmdbApi;
use emos_tmdb_api::model::MediaItem::Movie;
use emos_tmdb_api::model::MediaItem::Tv;
use futures_util::StreamExt;
use futures_util::stream;
use regex::Regex;
use tracing::debug;
use tracing::info;

type SimpleCache = Cache<String, Vec<Media>>;

struct App {
    tmdb_api: Arc<TmdbApi>,
    cache: Arc<SimpleCache>,
}

#[add_task("watch_hot_video")]
pub async fn run(douban_user_id: Option<String>) -> anyhow::Result<()> {
    let mut data = get_douban_video(douban_user_id).await?;

    let tmdb_api = TmdbApi::new()?;
    data.extend(get_tmdb_video(&tmdb_api).await?);

    generate_dynamic_binding_file(
        "watch_hot_video.json",
        "热门追更",
        "https://media.githubusercontent.com/media/bxb100/emos/refs/heads/main/data/covers/hot.png",
        data,
    )?;

    Ok(())
}

#[allow(unused_variables)]
async fn load_chosen_douban_collection_medias(
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

async fn get_douban_video(douban_user_id: Option<String>) -> anyhow::Result<Vec<Media>> {
    let api = DoubanApi::new();
    let res = load_chosen_douban_collection_medias(&api).await?;

    let app = Arc::new(App {
        tmdb_api: Arc::new(TmdbApi::new()?),
        cache: Arc::new(SimpleCache::new()?),
    });
    let app_clone = app.clone();
    let mut video_res = {
        stream::iter(res)
            .filter_map(move |item| {
                let app_clone = app_clone.clone();
                async move {
                    filter_douban_by_cache(
                        app_clone,
                        item.type_field,
                        &item.id,
                        &item.title,
                        item.year.as_ref(),
                    )
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
                    filter_douban_by_cache(
                        app_clone,
                        i.subject.type_field,
                        &i.subject.id,
                        &i.subject.title,
                        Some(&i.subject.year),
                    )
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

async fn get_tmdb_video(api: &TmdbApi) -> anyhow::Result<Vec<Media>> {
    let mut res = vec![];

    // on purpose to sequentially fetch
    for _page in 1..=5 {
        if let Ok(data) = api.tv_popular(Some(_page)).await {
            res.extend(data.results.iter().map(|s| Media {
                tmdb_id: s.id,
                tmdb_type: MediaType::Tv,
                title: s.name.to_string(),
                sort: 100,
            }))
        };
        if let Ok(data) = api.movie_popular(Some(_page)).await {
            res.extend(data.results.iter().map(|s| Media {
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

async fn filter_douban_by_cache(
    app: Arc<App>,
    type_field: TypeField,
    item_id: &str,
    item_title: &str,
    year: Option<impl AsRef<str>>,
) -> anyhow::Result<Vec<Media>> {
    let id = format!("douban_video_{}", item_id);
    let cache = app.cache.deref();

    // empty data fallback to re-fetch
    if let Ok(Some(data)) = cache.get(&id).await
        && !data.is_empty()
    {
        debug!("{id} Cache hit: {:?} ", data);
        bail!("cache hit");
    }

    let title = regex_replace_season(item_title);

    let v = match type_field {
        TypeField::Movie => {
            let res = app.tmdb_api.search_movie(&title, year, None).await?;
            info!("Movie {id} {title} found {}", res.total_results);
            res.results
                .iter()
                .map(|m| Media {
                    tmdb_id: m.id,
                    tmdb_type: MediaType::Movie,
                    title: m.title.to_owned(),
                    sort: 100,
                })
                .collect::<Vec<_>>()
        }
        TypeField::Tv => {
            let res = app.tmdb_api.search_tv(&title, year, None).await?;
            info!("TV {id} {title} found {}", res.total_results);
            res.results
                .iter()
                .map(|m| Media {
                    tmdb_id: m.id,
                    tmdb_type: MediaType::Tv,
                    title: m.name.to_owned(),
                    sort: 100,
                })
                .collect::<Vec<_>>()
        }
        TypeField::Unknown(s) => {
            let res = app.tmdb_api.search_multi(&title, None).await?;
            info!("Unknown {id} {s} found {}", res.total_results);
            res.results
                .iter()
                .filter(|e| matches!(e, Tv(_) | Movie(_)))
                .take(3)
                .filter_map(|e| match e {
                    Tv(t) => Some(Media {
                        tmdb_id: t.id,
                        tmdb_type: MediaType::Tv,
                        title: t.name.clone(),
                        sort: 100,
                    }),
                    Movie(m) => Some(Media {
                        tmdb_id: m.id,
                        tmdb_type: MediaType::Movie,
                        title: m.title.clone(),
                        sort: 100,
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>()
        }
    };

    cache.set(id, &v).await?;
    Ok(v)
}

#[inline]
fn regex_replace_season(title: &str) -> String {
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
        let items = get_douban_video(None).await.unwrap();
        println!("{:?}", items);
    }

    #[tokio::test]
    async fn test_get_tmdb_foreign_tv() {
        let api = TmdbApi::new().unwrap();
        let items = get_tmdb_video(&api).await.unwrap();
        println!("{:?}", items);
    }

    #[test]
    fn test_regex() {
        assert_eq!(
            regex_replace_season("【我推的孩子】 第三季"),
            "【我推的孩子】"
        );
        assert_eq!(regex_replace_season("辐射 第二季 Fallout Season 2"), "辐射");
        assert_eq!(regex_replace_season("御赐小仵作2"), "御赐小仵作");
        assert_eq!(regex_replace_season("有歌2026"), "有歌2026");
        assert_eq!(regex_replace_season("伟大的导游2.5"), "伟大的导游2.5");
        assert_eq!(regex_replace_season("x1"), "x1");
    }
}
