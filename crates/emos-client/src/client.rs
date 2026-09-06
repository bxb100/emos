use std::env;

use anyhow::Result;
use reqwest::Client as HttpClient;
use reqwest::header;

pub struct Client {
    pub client: HttpClient,
    pub base_url: String,
}

impl Client {
    pub fn from_env() -> Result<Self> {
        let mut headers = header::HeaderMap::new();

        let value = format!("Bearer {}", env::var("EMOS_TOKEN")?);
        let mut auth_value = header::HeaderValue::from_str(&value)?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        Ok(Client {
            client: HttpClient::builder()
                .user_agent("emos-rs/api")
                .default_headers(headers)
                .build()?,

            base_url: env::var("EMOS_API_URL")?.to_string(),
        })
    }
}
