use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SubjectKind {
    Movie,
    Tv,
    #[serde(untagged)]
    Unknown(String),
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
    pub kind: SubjectKind,
    pub year: String,
}
