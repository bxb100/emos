use anyhow::Context;
use emos_http::RequestBuilderExt;
use serde::Deserialize;
use serde::Serialize;

use crate::Client;
use crate::media::Page;

#[derive(Debug, Serialize, Deserialize)]
pub struct Movie {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub original_title: String,
    #[serde(default)]
    pub overview: String,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub release_date: Option<String>,
    #[serde(default)]
    pub vote_average: f64,
    #[serde(default)]
    pub vote_count: u64,
    #[serde(default)]
    pub popularity: f64,
    #[serde(default)]
    pub genre_ids: Vec<u64>,
    pub adult: Option<bool>,
    pub original_language: Option<String>,
    pub video: Option<bool>,
}

impl Client {
    pub async fn search_movie(
        &self,
        query: &str,
        year: Option<impl AsRef<str>>,
        page: Option<u64>,
    ) -> anyhow::Result<Page<Movie>> {
        let url = format!("{}/search/movie", self.base_url);
        let mut request = self.client.get(&url).query(&[("query", query)]);

        if let Some(y) = year {
            request = request.query(&[("year", y.as_ref())]);
        }
        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let result = request
            .send_json()
            .await
            .context("Failed to parse search_movie response")?;
        Ok(result)
    }

    pub async fn movie_popular(&self, page: Option<u64>) -> anyhow::Result<Page<Movie>> {
        let url = format!("{}/movie/popular", self.base_url);
        let mut request = self.client.get(&url);

        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let result = request
            .send_json()
            .await
            .context("Failed to parse movie_popular response")?;
        Ok(result)
    }

    pub async fn high_rated_scifi_movie(&self, page: Option<u64>) -> anyhow::Result<Page<Movie>> {
        let url = format!("{}/discover/movie", self.base_url);

        let request = self.client.get(&url).query(&[
            ("sort_by", "vote_average.desc"),
            ("vote_count.gte", "500"),
            ("vote_average.gte", "7.5"),
            ("with_genres", "878"), // sci-fi
            ("page", &page.unwrap_or(1).to_string()),
        ]);

        let result = request
            .send_json()
            .await
            .context("Failed to parse high_rated_scifi_movie response")?;
        Ok(result)
    }

    /// Get movie details by TMDB ID
    /// https://developer.themoviedb.org/reference/movie-details
    pub async fn get_movie(&self, movie_id: &str) -> anyhow::Result<Movie> {
        let url = format!("{}/movie/{}", self.base_url, movie_id);
        let result = self
            .client
            .get(&url)
            .send_json()
            .await
            .context("Failed to parse get_movie response")?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_movie_popular() -> anyhow::Result<()> {
        let api = Client::from_env()?;
        let result = api.movie_popular(Some(1)).await?;
        println!("Found {} popular movies", result.results.len());
        assert!(!result.results.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_high_rated_scifi_movie() -> anyhow::Result<()> {
        let api = Client::from_env()?;
        let result = api.high_rated_scifi_movie(Some(1)).await?;
        println!("Found {} high-rated sci-fi movies", result.total_results);
        assert!(!result.results.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_movie() -> anyhow::Result<()> {
        let api = Client::from_env()?;
        let movie = api.get_movie("550").await?; // Fight Club
        println!("Movie: {} (id: {})", movie.title, movie.id);
        assert_eq!(movie.id, 550);
        Ok(())
    }
}
