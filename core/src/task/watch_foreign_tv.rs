use std::collections::HashSet;

use douban_api::DoubanApi;
use douban_api::model::top_list::TopList;
use emos_api::watch::BatchType;
use emos_api::watch::UpdateWatchVideoBatchItem;
use regex::Regex;
use tmdb_api::TmdbApi;
use tmdb_api::model::MediaItem::Movie;
use tmdb_api::model::MediaItem::Tv;

use crate::ArgMatches;
use crate::add_task;

add_task!("watch_foreign_tv", run, watch_id: String = "watch_id", douban_user_id: String = "douban_user_id");

pub async fn run(watch_id: String, douban_user_id: String) -> anyhow::Result<()> {
    let douban_data = get_douban_foreign_tv(Some(douban_user_id)).await?;

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

    get_tmdb_foreign_tv(&tmdb_api)
        .await?
        .into_iter()
        .for_each(|id| {
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

async fn get_douban_foreign_tv(douban_user_id: Option<String>) -> anyhow::Result<Vec<String>> {
    let api = DoubanApi::new();

    // tv
    let tv_hot: TopList = api.tv_hot(Some(0), Some(200)).await?;
    let american_tv: TopList = api.tv_american(Some(0), Some(200)).await?;
    let korea_tv: TopList = api.tv_korean(Some(0), Some(200)).await?;
    let tv_japanese: TopList = api.tv_japanese(Some(0), Some(200)).await?;
    // movie
    let movie_showing: TopList = api.movie_showing(Some(0), Some(200)).await?;
    let movie_scifi: TopList = api.movie_scifi(None, Some(500)).await?;

    let mut items = tv_hot.subject_collection_items;
    items.extend(american_tv.subject_collection_items);
    items.extend(korea_tv.subject_collection_items);
    items.extend(tv_japanese.subject_collection_items);
    items.extend(movie_showing.subject_collection_items);
    items.extend(movie_scifi.subject_collection_items);

    let mut foreign_tv = items
        .into_iter()
        .map(|item| item.title)
        .collect::<HashSet<_>>();

    if let Some(douban_user_id) = douban_user_id {
        let wish_data = api
            .wish(&douban_user_id, Some(0), Some(100))
            .await?
            .interests
            .into_iter()
            .map(|item| item.subject.title)
            .collect::<HashSet<_>>();
        foreign_tv.extend(wish_data);
    }

    Ok(foreign_tv.into_iter().collect())
}

async fn get_tmdb_foreign_tv(api: &TmdbApi) -> anyhow::Result<Vec<u64>> {
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
        let items = get_douban_foreign_tv(None).await.unwrap();
        println!("{:?}", items);
    }

    #[tokio::test]
    async fn test_get_tmdb_foreign_tv() {
        let api = TmdbApi::new().unwrap();
        let items = get_tmdb_foreign_tv(&api).await.unwrap();
        println!("{:?}", items);
    }
}
