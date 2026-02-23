use serde::Deserialize;
use serde::Serialize;

use crate::model::TypeField;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interests {
    pub count: i64,
    pub interests: Vec<Interest>,
    pub start: i64,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interest {
    pub id: i64,
    pub status: String,
    pub subject: Subject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
    #[serde(rename = "card_subtitle")]
    pub card_subtitle: String,
    pub genres: Vec<String>,
    pub id: String,
    #[serde(rename = "is_released")]
    pub is_released: bool,
    #[serde(rename = "is_show")]
    pub is_show: bool,
    pub subtype: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_field: TypeField,
    pub year: String,
}

#[cfg(test)]
mod tests {
    use crate::DoubanApi;
    #[tokio::test]
    async fn test_interests() {
        let api = DoubanApi::new();
        let c = api.wish("1321428", Some(0), Some(50)).await.unwrap();

        println!("{:?}", c);
    }
}
