use std::collections::HashMap;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::Client;
use crate::subject::SubjectKind;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPage {
    pub count: i64,
    pub start: i64,
    #[serde(rename = "subject_collection_items")]
    pub items: Vec<CollectionItem>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionItem {
    #[serde(rename = "card_subtitle")]
    pub card_subtitle: String,
    #[serde(rename = "has_linewatch")]
    pub has_linewatch: bool,
    pub id: String,
    pub interest: Value,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: SubjectKind,
    pub uri: String,
    pub year: Option<String>,
}

macro_rules! collection_method {
    ($method_name:ident, $path:expr) => {
        pub async fn $method_name<T: DeserializeOwned>(
            &self,
            start: Option<i32>,
            count: Option<i32>,
        ) -> Result<T> {
            let params = HashMap::from([
                ("start".to_string(), start.unwrap_or(0).to_string()),
                ("count".to_string(), count.unwrap_or(100).to_string()),
            ]);
            self.invoke($path, params).await
        }
    };
}

impl Client {
    // 榜单类方法
    collection_method!(movie_top250, "/subject_collection/movie_top250/items");
    collection_method!(movie_scifi, "/subject_collection/movie_scifi/items");
    collection_method!(movie_hot_gaia, "/subject_collection/movie_hot_gaia/items");
    collection_method!(movie_comedy, "/subject_collection/movie_comedy/items");
    collection_method!(movie_action, "/subject_collection/movie_action/items");
    collection_method!(movie_love, "/subject_collection/movie_love/items");

    collection_method!(tv_hot, "/subject_collection/tv_hot/items");
    collection_method!(
        tv_chinese_best_weekly,
        "/subject_collection/tv_chinese_best_weekly/items"
    );
    collection_method!(
        tv_global_best_weekly,
        "/subject_collection/tv_global_best_weekly/items"
    );

    collection_method!(show_hot, "/subject_collection/show_hot/items");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Client;

    #[tokio::test]
    async fn test_movie_scifi() -> Result<()> {
        let api = Client::new();
        let res: serde_json::Value = api.movie_scifi(Some(0), Some(5)).await?;
        println!("{}", serde_json::to_string(&res)?);
        Ok(())
    }

    #[tokio::test]
    async fn test_tv_hot() -> anyhow::Result<()> {
        let api = Client::new();
        let res: CollectionPage = api.tv_hot(Some(0), Some(50)).await?;
        println!("{}", serde_json::to_string(&res)?);
        Ok(())
    }

    #[tokio::test]
    async fn test_show_hot() -> anyhow::Result<()> {
        let api = Client::new();
        let res: CollectionPage = api.show_hot(Some(0), Some(50)).await?;
        println!("{}", serde_json::to_string(&res)?);
        Ok(())
    }
}
