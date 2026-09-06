use anyhow::Context;
use emos_http::RequestBuilderExt;
use serde::Deserialize;
use serde::Serialize;

use crate::Client;
use crate::movie::Movie;
use crate::tv::TvSeries;

#[derive(Debug, Serialize, Deserialize)]
pub struct Page<T> {
    pub page: u64,
    pub results: Vec<T>,
    pub total_pages: u64,
    pub total_results: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "media_type", rename_all = "lowercase")]
pub enum MediaItem {
    Movie(Movie),
    Tv(TvSeries),
    Person(Person),
    Collection,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Person {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub original_name: String,
    pub profile_path: Option<String>,
    #[serde(default)]
    pub known_for: Vec<MediaItem>,
    pub gender: Option<u8>,
    pub known_for_department: Option<String>,
    #[serde(default)]
    pub popularity: f64,
    pub adult: Option<bool>,
}

impl Client {
    pub async fn search_multi(
        &self,
        query: &str,
        page: Option<u64>,
    ) -> anyhow::Result<Page<MediaItem>> {
        let url = format!("{}/search/multi", self.base_url);
        let mut request = self.client.get(&url).query(&[("query", query)]);

        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let result = request
            .send_json()
            .await
            .context("Failed to parse search_multi response")?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_search_multi() -> anyhow::Result<()> {
        let api = Client::from_env()?;
        let result = api.search_multi("辛德勒的名单", None).await?;
        println!("Found {} results", result.total_results);
        for item in result.results {
            match item {
                MediaItem::Movie(m) => println!("Movie: {}, id: {}", m.title, m.id),
                MediaItem::Tv(t) => println!("TV: {}", t.name),
                MediaItem::Person(p) => println!("Person: {}", p.name),
                _ => {}
            }
        }
        Ok(())
    }
}
