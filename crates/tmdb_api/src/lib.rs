pub mod model;

use std::env;
use std::str::FromStr;

use anyhow::Context;
use emos_utils::ReqwestExt;
use model::MediaItem;
use model::Movie;
use model::PagedResult;
use model::Tv;
use reqwest::Client;
use reqwest::header;

const BASE_URL: &str = "https://api.themoviedb.org/3";
pub const IMAGE_BASE_URL: &str = "https://image.tmdb.org/t/p/original";

pub struct TmdbApi {
    client: Client,
    base_url: String,
}

impl Default for TmdbApi {
    fn default() -> Self {
        Self::new().expect(
            "Failed to create default TmdbApi client. Ensure TMDB_ACCESS_TOKEN is set and valid.",
        )
    }
}

impl TmdbApi {
    pub fn new() -> anyhow::Result<Self> {
        let token = env::var("TMDB_ACCESS_TOKEN")?;
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

    pub async fn search_multi(
        &self,
        query: &str,
        page: Option<u64>,
    ) -> anyhow::Result<PagedResult<MediaItem>> {
        let url = format!("{}/search/multi", self.base_url);
        let mut request = self.client.get(&url).query(&[("query", query)]);

        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let result = request
            .execute()
            .await
            .context("Failed to parse search_multi response")?;
        Ok(result)
    }

    pub async fn search_movie(
        &self,
        query: &str,
        year: Option<impl AsRef<str>>,
        page: Option<u64>,
    ) -> anyhow::Result<PagedResult<Movie>> {
        let url = format!("{}/search/movie", self.base_url);
        let mut request = self.client.get(&url).query(&[("query", query)]);

        if let Some(y) = year {
            request = request.query(&[("year", y.as_ref())]);
        }
        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let result = request
            .execute()
            .await
            .context("Failed to parse search_movie response")?;
        Ok(result)
    }

    pub async fn search_tv(
        &self,
        query: &str,
        // we don't use `first_air_date_year` because douban may return season-specific air date
        year: Option<impl AsRef<str>>,
        page: Option<u64>,
    ) -> anyhow::Result<PagedResult<Tv>> {
        let url = format!("{}/search/tv", self.base_url);
        let mut request = self.client.get(&url).query(&[("query", query)]);

        if let Some(y) = year
            && let Ok(_year) = i32::from_str(y.as_ref())
        {
            request = request.query(&[("year", y.as_ref())]);
        }
        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let result = request
            .execute()
            .await
            .context("Failed to parse search_tv response")?;
        Ok(result)
    }

    pub async fn movie_popular(&self, page: Option<u64>) -> anyhow::Result<PagedResult<Movie>> {
        let url = format!("{}/movie/popular", self.base_url);
        let mut request = self.client.get(&url);

        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let result = request
            .execute()
            .await
            .context("Failed to parse movie_popular response")?;
        Ok(result)
    }

    pub async fn tv_popular(&self, page: Option<u64>) -> anyhow::Result<PagedResult<Tv>> {
        let url = format!("{}/tv/popular", self.base_url);
        let mut request = self.client.get(&url);

        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let result = request
            .execute()
            .await
            .context("Failed to parse tv_popular response")?;
        Ok(result)
    }

    pub async fn high_rated_scifi_movie(
        &self,
        page: Option<u64>,
    ) -> anyhow::Result<PagedResult<Movie>> {
        let url = format!("{}/discover/movie", self.base_url);

        let request = self.client.get(&url).query(&[
            ("sort_by", "vote_average.desc"),
            ("vote_count.gte", "500"),
            ("vote_average.gte", "7.5"),
            ("with_genres", "878"), // sci-fi
            ("page", &page.unwrap_or(1).to_string()),
        ]);

        let result = request
            .execute()
            .await
            .context("Failed to parse high_rated_scifi_movie response")?;
        Ok(result)
    }

    pub async fn high_rated_scifi_tv(&self, page: Option<u64>) -> anyhow::Result<PagedResult<Tv>> {
        let url = format!("{}/discover/tv", self.base_url);

        let request = self.client.get(&url).query(&[
            ("sort_by", "vote_average.desc"),
            ("vote_count.gte", "500"),
            ("vote_average.gte", "7.5"),
            ("with_genres", "10765"), // sci-fi & fantasy
            ("page", &page.unwrap_or(1).to_string()),
        ]);

        let result = request
            .execute()
            .await
            .context("Failed to parse high_rated_scifi_tv response")?;
        Ok(result)
    }

    /// Get movie details by TMDB ID
    /// https://developer.themoviedb.org/reference/movie-details
    pub async fn get_movie(&self, movie_id: &str) -> anyhow::Result<Movie> {
        let url = format!("{}/movie/{}", self.base_url, movie_id);
        let result = self
            .client
            .get(&url)
            .execute()
            .await
            .context("Failed to parse get_movie response")?;
        Ok(result)
    }

    /// Get TV series details by TMDB ID
    /// https://developer.themoviedb.org/reference/tv-series-details
    pub async fn get_tv(&self, tv_id: &str) -> anyhow::Result<Tv> {
        let url = format!("{}/tv/{}", self.base_url, tv_id);
        let result = self
            .client
            .get(&url)
            .execute()
            .await
            .context("Failed to parse get_tv response")?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_search_multi() -> anyhow::Result<()> {
        let api = TmdbApi::new()?;
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

    #[tokio::test]
    #[ignore]
    async fn test_movie_popular() -> anyhow::Result<()> {
        let api = TmdbApi::new()?;
        let result = api.movie_popular(Some(1)).await?;
        println!("Found {} popular movies", result.results.len());
        assert!(!result.results.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_tv_popular() -> anyhow::Result<()> {
        let api = TmdbApi::new()?;
        let result = api.tv_popular(None).await?;
        println!("Found {} popular TV shows", result.results.len());
        assert!(!result.results.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_high_rated_scifi_movie() -> anyhow::Result<()> {
        let api = TmdbApi::new()?;
        let result = api.high_rated_scifi_movie(Some(1)).await?;
        println!("Found {} high-rated sci-fi movies", result.total_results);
        assert!(!result.results.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_high_rated_scifi_tv() -> anyhow::Result<()> {
        let api = TmdbApi::new()?;
        let result = api.high_rated_scifi_tv(Some(1)).await?;
        println!("Found {} high-rated sci-fi TV shows", result.total_results);
        assert!(!result.results.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_movie() -> anyhow::Result<()> {
        let api = TmdbApi::new()?;
        let movie = api.get_movie("550").await?; // Fight Club
        println!("Movie: {} (id: {})", movie.title, movie.id);
        assert_eq!(movie.id, 550);
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_tv() -> anyhow::Result<()> {
        let api = TmdbApi::new()?;
        let tv = api.get_tv("1399").await?; // Breaking Bad
        println!("TV: {} (id: {})", tv.name, tv.id);
        assert_eq!(tv.id, 1399);
        Ok(())
    }
}
