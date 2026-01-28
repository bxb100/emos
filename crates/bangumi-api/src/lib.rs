use std::collections::HashMap;

use reqwest::Client;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub keyword: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<SearchSort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<SearchFilter>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSort {
    Match,
    Heat,
    Rank,
    Score,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SearchFilter {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub subject_type: Option<Vec<SubjectType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub air_date: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating_count: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nsfw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PagedSubject {
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
    pub data: Vec<Subject>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Subject {
    pub id: u64,
    #[serde(rename = "type")]
    pub subject_type: SubjectType,
    pub name: String,
    pub name_cn: String,
    pub summary: String,
    pub nsfw: bool,
    pub locked: bool,
    pub date: Option<String>,
    pub platform: String,
    pub images: Images,
    pub infobox: Option<Vec<InfoboxItem>>,
    pub volumes: Option<u64>,
    pub eps: Option<u64>,
    pub total_episodes: Option<u64>,
    pub rating: Rating,
    pub collection: Collection,
    pub meta_tags: Vec<String>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum SubjectType {
    Book = 1,
    Anime = 2,
    Music = 3,
    Game = 4,
    Real = 6,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Images {
    pub large: String,
    pub common: String,
    pub medium: String,
    pub small: String,
    pub grid: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InfoboxItem {
    pub key: String,
    pub value: InfoboxValue,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum InfoboxValue {
    String(String),
    List(Vec<InfoboxItemValue>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum InfoboxItemValue {
    KV { k: String, v: String },
    V { v: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rating {
    pub rank: u64,
    pub total: u64,
    // The API returns a map of "1".."10" to count.
    pub count: HashMap<String, u64>,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Collection {
    pub wish: u64,
    pub collect: u64,
    pub doing: u64,
    pub on_hold: u64,
    pub dropped: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub count: u64,
}

// --- API Client ---

pub struct BangumiApi {
    client: Client,
    base_url: String,
}

impl BangumiApi {
    pub fn new() -> anyhow::Result<Self> {
        let headers = header::HeaderMap::new();
        // Bangumi API requires User-Agent
        // Using a default one here.
        let client = Client::builder()
            .user_agent("bangumi-api-rs/0.1.0 (contact: github.com/bxb100/emos)")
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            base_url: "https://api.bgm.tv".to_string(),
        })
    }

    pub async fn search_subjects(
        &self,
        request: SearchRequest,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> anyhow::Result<PagedSubject> {
        let url = format!("{}/v0/search/subjects", self.base_url);

        let query_params: Vec<(&str, String)> = [
            limit.map(|v| ("limit", v.to_string())),
            offset.map(|v| ("offset", v.to_string())),
        ]
        .into_iter()
        .flatten()
        .collect();

        let resp = self
            .client
            .post(&url)
            .query(&query_params)
            .json(&request)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("API request failed: {} - {}", status, text));
        }

        let paged_subject: PagedSubject = resp.json().await?;
        Ok(paged_subject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_subjects() -> anyhow::Result<()> {
        let api = BangumiApi::new()?;
        let request = SearchRequest {
            keyword: "Cowboy Bebop".to_string(),
            sort: None,
            filter: Some(SearchFilter {
                subject_type: Some(vec![SubjectType::Anime]),
                ..Default::default()
            }),
        };

        let result = api.search_subjects(request, Some(10), None).await?;
        println!("Found {} results", result.total);
        if let Some(subject) = result.data.first() {
            println!("First result: {} ({})", subject.name, subject.name_cn);
            println!("Summary: {}", subject.summary);
            println!("Rating: {}", subject.rating.score);
        }

        assert!(result.total > 0);
        Ok(())
    }
}
