use std::collections::HashSet;

use douban_api::DoubanApi;
use douban_api::model::top_list::SubjectCollectionItem;
use douban_api::model::top_list::TopList;
use emos_api::watch::BatchType;
use emos_api::watch::UpdateWatchVideoBatchItem;
use regex::Regex;
use tmdb_api::TmdbApi;
use tmdb_api::model::MediaItem::Movie;
use tmdb_api::model::MediaItem::Tv;

use crate::ArgMatches;
use crate::add_task;

add_task!("watch_hot_video", run, watch_id: String = "watch_id", douban_user_id: String = "douban_user_id");

pub async fn run(watch_id: String, douban_user_id: String) -> anyhow::Result<()> {
    let douban_data = get_douban_video(Some(douban_user_id)).await?;

    let mut data = vec![];

    // really need this?
    let title_regex = Regex::new(r"[第\s]+([0-9一二三四五六七八九十S\-]+)\s*季")?;

    let tmdb_api = TmdbApi::new()?;
    for datum in douban_data {
        let title = title_regex.replace(&datum, "");

        let res = tmdb_api.search_multi(&title, None).await?;
        res.results.iter().for_each(|e| match e {
            Tv(t) => data.push(UpdateWatchVideoBatchItem {
                r#type: BatchType::TmdbTv,
                value: t.id.to_string(),
            }),
            Movie(m) => data.push(UpdateWatchVideoBatchItem {
                r#type: BatchType::TmdbTv,
                value: m.id.to_string(),
            }),
            _ => {}
        });
    }

    get_tmdb_video(&tmdb_api).await?.into_iter().for_each(|id| {
        data.push(UpdateWatchVideoBatchItem {
            r#type: BatchType::TmdbTv,
            value: id.to_string(),
        });
    });

    let emos_api = emos_api::EmosApi::new()?;
    // chunk 200 and send
    for batch in data.chunks(200) {
        emos_api.batch_update_watch_videos(&watch_id, batch).await?;

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }

    Ok(())
}

async fn get_douban_video(douban_user_id: Option<String>) -> anyhow::Result<Vec<String>> {
    let api = DoubanApi::new();

    let mut res: Vec<SubjectCollectionItem> = vec![];
    macro_rules! load_all {
        ($fun:expr) => {{
            let start = 0;
            let mut total = 0;
            let mut res = vec![];

            loop {
                let data: TopList = $fun(&api, Some(start), Some(50)).await?;
                res.extend(data.subject_collection_items.into_iter());
                total += data.count;
                if total >= data.total {
                    break;
                }
            }

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

    let mut video_res = res
        .into_iter()
        .map(|item| item.title)
        .collect::<HashSet<String>>();

    if let Some(douban_user_id) = douban_user_id {
        let wish_data = api
            .wish(&douban_user_id, Some(0), Some(100))
            .await?
            .interests
            .into_iter()
            .map(|item| item.subject.title)
            .collect::<HashSet<_>>();
        video_res.extend(wish_data);
    }

    Ok(video_res.into_iter().collect())
}

async fn get_tmdb_video(api: &TmdbApi) -> anyhow::Result<Vec<u64>> {
    let mut res = vec![];

    for _page in 1..=5 {
        if let Ok(data) = api.tv_popular(Some(_page)).await {
            res.extend(data.results.iter().map(|s| s.id).collect::<Vec<_>>())
        };
        if let Ok(data) = api.movie_popular(Some(_page)).await {
            res.extend(data.results.iter().map(|s| s.id).collect::<Vec<_>>())
        };
    }
    Ok(res)
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
