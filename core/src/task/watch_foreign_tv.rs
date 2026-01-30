use dao::Dao;
use douban_api::DoubanApi;
use douban_api::model::top_list::SubjectCollectionItem;
use douban_api::model::top_list::TopList;
use emos_api::watch::BatchType;
use emos_api::watch::UpdateWatchVideoBatchItem;
use regex::Regex;
use tmdb_api::TmdbApi;
use tmdb_api::model::Tv;

use crate::ArgMatches;
use crate::add_task;

add_task!("watch_foreign_tv", run, watch_id: String = "watch_id");

pub async fn run(watch_id: String) -> anyhow::Result<()> {
    let douban_data = get_douban_foreign_tv().await?;

    let dao = Dao::new().await?;
    let mut data = vec![];

    let title_regex = Regex::new(r"[第\s]+([0-9一二三四五六七八九十S\-]+)\s*季")?;
    for datum in douban_data {
        // I know it's idiot
        if let Some(x) = dao
            .find_by_name(&title_regex.replace(&datum.title, ""), true)
            .await?
        {
            data.push(UpdateWatchVideoBatchItem {
                r#type: BatchType::Todb,
                value: x.todb_id.to_string(),
            });
        }
    }

    get_tmdb_foreign_tv().await?.into_iter().for_each(|tv| {
        data.push(UpdateWatchVideoBatchItem {
            r#type: BatchType::TmdbTv,
            value: tv.id.to_string(),
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

async fn get_douban_foreign_tv() -> anyhow::Result<Vec<SubjectCollectionItem>> {
    let api = DoubanApi::new();

    let tv_hot: TopList = api.tv_hot(Some(0), Some(200)).await?;
    let american_tv: TopList = api.tv_american(Some(0), Some(200)).await?;
    let korea_tv: TopList = api.tv_korean(Some(0), Some(200)).await?;
    let tv_japanese: TopList = api.tv_japanese(Some(0), Some(200)).await?;

    let mut items = tv_hot.subject_collection_items;
    items.extend(american_tv.subject_collection_items);
    items.extend(korea_tv.subject_collection_items);
    items.extend(tv_japanese.subject_collection_items);

    let foreign_tv = items
        .into_iter()
        .filter(|item| !item.card_subtitle.contains("中国大陆"))
        .collect::<Vec<_>>();

    Ok(foreign_tv)
}

async fn get_tmdb_foreign_tv() -> anyhow::Result<Vec<Tv>> {
    let api = TmdbApi::new()?;
    let result = api
        .tv_popular(None)
        .await?
        .results
        .into_iter()
        .filter(|tv| tv.origin_country.iter().all(|c| c != "CN"))
        .collect::<Vec<_>>();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_douban_foreign_tv() {
        let items = get_douban_foreign_tv().await.unwrap();
        println!("{:?}", items);
    }

    #[tokio::test]
    async fn test_get_tmdb_foreign_tv() {
        let items = get_tmdb_foreign_tv().await.unwrap();
        println!("{:?}", items);
    }
}
