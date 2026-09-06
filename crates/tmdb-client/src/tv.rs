use std::str::FromStr;

use anyhow::Context;
use emos_http::RequestBuilderExt;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::Client;
use crate::media::Page;

#[derive(Debug, Serialize, Deserialize)]
pub struct TvSeries {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub original_name: String,
    #[serde(default)]
    pub overview: String,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub vote_average: f64,
    #[serde(default)]
    pub vote_count: u64,
    #[serde(default)]
    pub popularity: f64,
    #[serde(default)]
    pub genre_ids: Vec<u64>,
    pub original_language: Option<String>,
    #[serde(default)]
    pub origin_country: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TvSeriesDetails {
    pub adult: bool,
    #[serde(rename = "backdrop_path")]
    pub backdrop_path: String,
    #[serde(rename = "created_by")]
    pub created_by: Vec<Value>,
    #[serde(rename = "episode_run_time")]
    pub episode_run_time: Vec<Value>,
    #[serde(rename = "first_air_date")]
    pub first_air_date: String,
    pub genres: Vec<Genre>,
    pub homepage: String,
    pub id: i64,
    #[serde(rename = "in_production")]
    pub in_production: bool,
    pub languages: Vec<String>,
    #[serde(rename = "last_air_date")]
    pub last_air_date: String,
    #[serde(rename = "last_episode_to_air")]
    pub last_episode_to_air: LastEpisodeToAir,
    pub name: String,
    #[serde(rename = "next_episode_to_air")]
    pub next_episode_to_air: NextEpisodeToAir,
    pub networks: Vec<Network>,
    #[serde(rename = "number_of_episodes")]
    pub number_of_episodes: i64,
    #[serde(rename = "number_of_seasons")]
    pub number_of_seasons: i64,
    #[serde(rename = "origin_country")]
    pub origin_country: Vec<String>,
    #[serde(rename = "original_language")]
    pub original_language: String,
    #[serde(rename = "original_name")]
    pub original_name: String,
    pub overview: String,
    pub popularity: f64,
    #[serde(rename = "poster_path")]
    pub poster_path: String,
    #[serde(rename = "production_companies")]
    pub production_companies: Vec<ProductionCompany>,
    #[serde(rename = "production_countries")]
    pub production_countries: Vec<ProductionCountry>,
    pub seasons: Vec<Season>,
    pub softcore: bool,
    #[serde(rename = "spoken_languages")]
    pub spoken_languages: Vec<SpokenLanguage>,
    pub status: String,
    pub tagline: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "vote_average")]
    pub vote_average: f64,
    #[serde(rename = "vote_count")]
    pub vote_count: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    pub id: i64,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastEpisodeToAir {
    pub id: i64,
    pub name: String,
    pub overview: String,
    #[serde(rename = "vote_average")]
    pub vote_average: f64,
    #[serde(rename = "vote_count")]
    pub vote_count: i64,
    #[serde(rename = "air_date")]
    pub air_date: String,
    #[serde(rename = "episode_number")]
    pub episode_number: i64,
    #[serde(rename = "episode_type")]
    pub episode_type: String,
    #[serde(rename = "production_code")]
    pub production_code: String,
    pub runtime: i64,
    #[serde(rename = "season_number")]
    pub season_number: i64,
    #[serde(rename = "show_id")]
    pub show_id: i64,
    #[serde(rename = "still_path")]
    pub still_path: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextEpisodeToAir {
    pub id: i64,
    pub name: String,
    pub overview: String,
    #[serde(rename = "vote_average")]
    pub vote_average: f64,
    #[serde(rename = "vote_count")]
    pub vote_count: i64,
    #[serde(rename = "air_date")]
    pub air_date: String,
    #[serde(rename = "episode_number")]
    pub episode_number: i64,
    #[serde(rename = "episode_type")]
    pub episode_type: String,
    #[serde(rename = "production_code")]
    pub production_code: String,
    pub runtime: i64,
    #[serde(rename = "season_number")]
    pub season_number: i64,
    #[serde(rename = "show_id")]
    pub show_id: i64,
    #[serde(rename = "still_path")]
    pub still_path: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Network {
    pub id: i64,
    #[serde(rename = "logo_path")]
    pub logo_path: String,
    pub name: String,
    #[serde(rename = "origin_country")]
    pub origin_country: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionCompany {
    pub id: i64,
    #[serde(rename = "logo_path")]
    pub logo_path: Option<String>,
    pub name: String,
    #[serde(rename = "origin_country")]
    pub origin_country: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionCountry {
    #[serde(rename = "iso_3166_1")]
    pub iso_3166_1: String,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    #[serde(rename = "air_date")]
    pub air_date: String,
    #[serde(rename = "episode_count")]
    pub episode_count: i64,
    pub id: i64,
    pub name: String,
    pub overview: String,
    #[serde(rename = "poster_path")]
    pub poster_path: String,
    #[serde(rename = "season_number")]
    pub season_number: i64,
    #[serde(rename = "vote_average")]
    pub vote_average: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpokenLanguage {
    #[serde(rename = "english_name")]
    pub english_name: String,
    #[serde(rename = "iso_639_1")]
    pub iso_639_1: String,
    pub name: String,
}

impl Client {
    pub async fn search_tv(
        &self,
        query: &str,
        // we don't use `first_air_date_year` because douban may return season-specific air date
        year: Option<impl AsRef<str>>,
        page: Option<u64>,
    ) -> anyhow::Result<Page<TvSeries>> {
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
            .send_json()
            .await
            .context("Failed to parse search_tv response")?;
        Ok(result)
    }

    pub async fn tv_popular(&self, page: Option<u64>) -> anyhow::Result<Page<TvSeries>> {
        let url = format!("{}/tv/popular", self.base_url);
        let mut request = self.client.get(&url);

        if let Some(p) = page {
            request = request.query(&[("page", p)]);
        }

        let result = request
            .send_json()
            .await
            .context("Failed to parse tv_popular response")?;
        Ok(result)
    }

    pub async fn high_rated_scifi_tv(&self, page: Option<u64>) -> anyhow::Result<Page<TvSeries>> {
        let url = format!("{}/discover/tv", self.base_url);

        let request = self.client.get(&url).query(&[
            ("sort_by", "vote_average.desc"),
            ("vote_count.gte", "500"),
            ("vote_average.gte", "7.5"),
            ("with_genres", "10765"), // sci-fi & fantasy
            ("page", &page.unwrap_or(1).to_string()),
        ]);

        let result = request
            .send_json()
            .await
            .context("Failed to parse high_rated_scifi_tv response")?;
        Ok(result)
    }

    /// Get TV series details by TMDB ID
    /// https://developer.themoviedb.org/reference/tv-series-details
    pub async fn get_tv(&self, tv_id: &str) -> anyhow::Result<TvSeries> {
        let url = format!("{}/tv/{}", self.base_url, tv_id);
        let result = self
            .client
            .get(&url)
            .send_json()
            .await
            .context("Failed to parse get_tv response")?;
        Ok(result)
    }

    pub async fn tv_details(&self, series_id: u64) -> anyhow::Result<TvSeriesDetails> {
        let url = format!("{}/tv/{series_id}", self.base_url);

        let result = self
            .client
            .get(&url)
            .send_json()
            .await
            .context("Failed to parse tv_details response")?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_tv_popular() -> anyhow::Result<()> {
        let api = Client::from_env()?;
        let result = api.tv_popular(None).await?;
        println!("Found {} popular TV shows", result.results.len());
        assert!(!result.results.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_high_rated_scifi_tv() -> anyhow::Result<()> {
        let api = Client::from_env()?;
        let result = api.high_rated_scifi_tv(Some(1)).await?;
        println!("Found {} high-rated sci-fi TV shows", result.total_results);
        assert!(!result.results.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_tv() -> anyhow::Result<()> {
        let api = Client::from_env()?;
        let tv = api.get_tv("1399").await?; // Breaking Bad
        println!("TV: {} (id: {})", tv.name, tv.id);
        assert_eq!(tv.id, 1399);
        Ok(())
    }

    #[tokio::test]
    async fn test_tv_details() -> anyhow::Result<()> {
        let api = Client::from_env()?;
        println!("{:?}", api.tv_details(254498).await?);
        Ok(())
    }
}
