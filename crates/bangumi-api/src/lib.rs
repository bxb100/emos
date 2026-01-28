use std::collections::HashMap;
use anyhow::bail;
use dotenv_codegen::dotenv;
use reqwest::Client;
use reqwest::header;
use serde::Deserialize;
use serde::Serialize;
use serde_repr::Deserialize_repr;
use serde_repr::Serialize_repr;

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde_with::skip_serializing_none]
pub struct SearchRequest {
    pub keyword: String,
    pub sort: Option<SearchSort>,
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
#[serde_with::skip_serializing_none]
pub struct SearchFilter {
    #[serde(rename = "type")]
    pub subject_type: Option<Vec<SubjectType>>,
    pub tag: Option<Vec<String>>,
    pub air_date: Option<Vec<String>>,
    pub rating: Option<Vec<String>>,
    pub rating_count: Option<Vec<String>>,
    pub rank: Option<Vec<String>>,
    pub nsfw: Option<bool>,
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

pub const DEFAULT_BASE_URL: &str = "https://api.bgm.tv";

impl BangumiApi {
    pub fn new() -> anyhow::Result<Self> {
        Self::with_url(DEFAULT_BASE_URL)
    }

    pub fn with_url(base_url: &str) -> anyhow::Result<Self> {
        let mut headers = header::HeaderMap::new();
        let access_token = format!("Bearer {}", dotenv!("BANGUMI_ACCESS_TOKEN"));
        let mut value = header::HeaderValue::from_str(&access_token)?;
        value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, value);

        // Bangumi API requires User-Agent
        // Using a default one here.
        let client = Client::builder()
            .user_agent("bangumi-api-rs/0.1.0 (contact: github.com/bxb100/emos)")
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.to_string(),
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
            bail!("API request failed: {} - {}", status, text);
        }

        let paged_subject: PagedSubject = resp.json().await?;
        Ok(paged_subject)
    }

    pub async fn search_top_rank_500(
        &self,
        keyword: &str,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> anyhow::Result<PagedSubject> {
        let request = SearchRequest {
            keyword: keyword.to_string(),
            sort: Some(SearchSort::Rank),
            filter: Some(SearchFilter {
                subject_type: Some(vec![SubjectType::Anime]),
                // batch video with 0 rank
                rank: Some(vec![">0".to_string(), "<=500".to_string()]),
                ..Default::default()
            }),
        };

        self.search_subjects(request, limit, offset).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_subjects() -> anyhow::Result<()> {
        let api = BangumiApi::new()?;
        let result = api.search_top_rank_500("", Some(10), Some(0)).await?;
        println!("{:#?}", result);
        Ok(())
    }
}
