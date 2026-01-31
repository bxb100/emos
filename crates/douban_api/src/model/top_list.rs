use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopList {
    pub count: i64,
    pub start: i64,
    #[serde(rename = "subject_collection_items")]
    pub subject_collection_items: Vec<SubjectCollectionItem>,
    pub total: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectCollectionItem {
    #[serde(rename = "card_subtitle")]
    pub card_subtitle: String,
    #[serde(rename = "has_linewatch")]
    pub has_linewatch: bool,
    pub id: String,
    pub interest: Value,
    pub title: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub uri: String,
    pub year: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DoubanApi;

    #[tokio::test]
    async fn test_tv_hot() -> anyhow::Result<()> {
        let api = DoubanApi::new();
        let res: TopList = api.tv_chinese_best_weekly(Some(0), Some(50)).await?;
        println!("{}", serde_json::to_string(&res)?);
        Ok(())
    }
}
