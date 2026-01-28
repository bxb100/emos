pub mod model;

use anyhow::Context;
use dotenv_codegen::dotenv;
use reqwest::{Client, header};
use model::{PagedResult, MediaItem, Movie, Tv};

const BASE_URL: &str = "https://api.themoviedb.org/3";

pub struct TmdbApi {
    client: Client,
    base_url: String,
}

impl Default for TmdbApi {
    fn default() -> Self {
        Self::new()
    }
}

impl TmdbApi {
    pub fn new() -> anyhow::Result<Self> {
        let token = dotenv!("TMDB_ACCESS_TOKEN");
        let mut headers = header::HeaderMap::new();
        let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {}", token))
            .context("Invalid TMDB_ACCESS_TOKEN format")?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .context("Failed to build reqwest client")?;

        Ok(Self {
            client,
            base_url: BASE_URL.to_string(),
        })
    }

    pub async fn search_multi(&self, query: &str, page: Option<u64>) -> anyhow::Result<PagedResult<MediaItem>> {
        let url = format!("{}/search/multi", self.base_url);
        let mut request = self.client.get(&url).query(&[("query", query)]);

        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let resp = request.send().await.context("Failed to send search_multi request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("TMDB API error: {} - {}", status, text);
        }

        let result = resp.json().await.context("Failed to parse search_multi response")?;
        Ok(result)
    }

    pub async fn movie_popular(&self, page: Option<u64>) -> anyhow::Result<PagedResult<Movie>> {
        let url = format!("{}/movie/popular", self.base_url);
        let mut request = self.client.get(&url);

        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let resp = request.send().await.context("Failed to send movie_popular request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("TMDB API error: {} - {}", status, text);
        }

        let result = resp.json().await.context("Failed to parse movie_popular response")?;
        Ok(result)
    }

    pub async fn tv_popular(&self, page: Option<u64>) -> anyhow::Result<PagedResult<Tv>> {
        let url = format!("{}/tv/popular", self.base_url);
        let mut request = self.client.get(&url);

        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let resp = request.send().await.context("Failed to send tv_popular request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("TMDB API error: {} - {}", status, text);
        }

        let result = resp.json().await.context("Failed to parse tv_popular response")?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_search_multi() -> anyhow::Result<()> {
        let api = TmdbApi::new();
        let result = api.search_multi("Inception", None).await?;
        println!("Found {} results", result.total_results);
        for item in result.results {
            match item {
                MediaItem::Movie(m) => println!("Movie: {}", m.title),
                MediaItem::Tv(t) => println!("TV: {}", t.name),
                MediaItem::Person(p) => println!("Person: {}", p.name),
            }
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_movie_popular() -> anyhow::Result<()> {
        let api = TmdbApi::new();
        let result = api.movie_popular(None).await?;
        println!("Found {} popular movies", result.results.len());
        assert!(!result.results.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_tv_popular() -> anyhow::Result<()> {
        let api = TmdbApi::new();
        let result = api.tv_popular(None).await?;
        println!("Found {} popular TV shows", result.results.len());
        assert!(!result.results.is_empty());
        Ok(())
    }
}
