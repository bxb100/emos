use std::env;

use anyhow::Context;
use reqwest::Client as HttpClient;
use reqwest::header;

const BASE_URL: &str = "https://api.themoviedb.org/3";

pub struct Client {
    pub(crate) client: HttpClient,
    pub(crate) base_url: String,
}

impl Client {
    pub fn from_env() -> anyhow::Result<Self> {
        let token = env::var("TMDB_ACCESS_TOKEN")?;
        let mut headers = header::HeaderMap::new();
        let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {}", token))
            .context("Invalid TMDB_ACCESS_TOKEN format")?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        let client = HttpClient::builder()
            .default_headers(headers)
            .build()
            .context("Failed to build reqwest client")?;

        Ok(Self {
            client,
            base_url: BASE_URL.to_string(),
        })
    }
}
