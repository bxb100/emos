use serde::Deserialize;
use serde::Serialize;

pub mod interests;
pub mod top_list;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TypeField {
    Movie,
    Tv,
    #[serde(untagged)]
    Unknown(String),
}
