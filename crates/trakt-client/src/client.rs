use std::env;

use anyhow::Context;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::header;

pub const DEFAULT_BASE_URL: &str = "https://api.trakt.tv";

// Default OAuth application credentials published by DuckieTV:
// https://github.com/SchizoDuckie/DuckieTV/blob/8825daba0960939fe199352c4fe8965d0bfb5be0/js/services/TraktTVv2.js#L7-L9
pub const DEFAULT_CLIENT_ID: &str =
    "e65088ee83478f54ffd9d5775dc63d0c64312eabd72b6b2e5623194675959bac"; // gitleaks:allow
pub const DEFAULT_CLIENT_SECRET: &str =
    "3e97816f32ac913e51a96d2b0296b8f2172e7dee4b01e62df381ad7f62560c96"; // gitleaks:allow

pub(crate) const TRAKT_API_KEY: &str = "trakt-api-key";
pub(crate) const TRAKT_API_VERSION: &str = "trakt-api-version";

pub struct Client {
    pub(crate) client: HttpClient,
    pub(crate) base_url: Url,
}

impl Client {
    /// Creates a client using `TRAKT_CLIENT_ID`, falling back to [`DEFAULT_CLIENT_ID`]
    /// when the variable is unset or blank.
    pub fn from_env() -> anyhow::Result<Self> {
        let client_id = env_or_default("TRAKT_CLIENT_ID", DEFAULT_CLIENT_ID);
        Self::with_client_id_and_base_url(client_id, DEFAULT_BASE_URL)
    }

    pub fn with_client_id_and_base_url(
        client_id: impl AsRef<str>,
        base_url: impl AsRef<str>,
    ) -> anyhow::Result<Self> {
        let client_id = client_id.as_ref().trim();
        anyhow::ensure!(!client_id.is_empty(), "TRAKT_CLIENT_ID must not be empty");

        let headers = trakt_headers(client_id)?;

        let client = HttpClient::builder()
            .user_agent(concat!(
                "emos-trakt-api/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/bxb100/emos)"
            ))
            .default_headers(headers)
            .build()
            .context("failed to build Trakt HTTP client")?;
        let base_url = normalize_base_url(base_url.as_ref())?;

        Ok(Self { client, base_url })
    }
}

pub(crate) fn env_or_default(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map_or_else(|| default.to_string(), |value| value.trim().to_string())
}

pub(crate) fn trakt_headers(client_id: &str) -> anyhow::Result<header::HeaderMap> {
    let mut headers = header::HeaderMap::new();
    let mut api_key = header::HeaderValue::from_str(client_id)
        .context("TRAKT_CLIENT_ID is not a valid HTTP header value")?;
    api_key.set_sensitive(true);
    headers.insert(header::HeaderName::from_static(TRAKT_API_KEY), api_key);
    headers.insert(
        header::HeaderName::from_static(TRAKT_API_VERSION),
        header::HeaderValue::from_static("2"),
    );
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    Ok(headers)
}

pub(crate) fn normalize_base_url(base_url: &str) -> anyhow::Result<Url> {
    let normalized = format!("{}/", base_url.trim_end_matches('/'));
    Url::parse(&normalized).context("invalid Trakt base URL")
}

pub(crate) async fn response_error(
    status: StatusCode,
    response: reqwest::Response,
) -> anyhow::Error {
    match response.text().await {
        Ok(body) => anyhow::anyhow!("Trakt API request failed with HTTP {status}: {body}"),
        Err(error) => anyhow::anyhow!(
            "Trakt API request failed with HTTP {status}; failed to read response body: {error}"
        ),
    }
}
