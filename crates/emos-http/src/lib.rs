//! Shared HTTP response handling for service clients.

use anyhow::Result;
use anyhow::bail;
use reqwest::RequestBuilder;
use serde::de::DeserializeOwned;
use tracing::instrument;

/// Sends a request and decodes its successful JSON response.
pub trait RequestBuilderExt {
    /// Returns an error for a non-success status or invalid JSON, including the
    /// failing field path when deserialization fails.
    fn send_json<T: DeserializeOwned>(self) -> impl Future<Output = Result<T>>;
}

impl RequestBuilderExt for RequestBuilder {
    async fn send_json<T: DeserializeOwned>(self) -> Result<T> {
        let resp = self.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("API request failed: {} - {}", status, text);
        }
        parse_json(resp).await
    }
}

#[instrument(skip(resp))]
async fn parse_json<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
    tracing::debug!(status = %resp.status(), url = %resp.url());

    let text = resp.text().await?;
    let jd = &mut serde_json::Deserializer::from_str(&text);
    let mut track = serde_path_to_error::Track::new();
    let pd = serde_path_to_error::Deserializer::new(jd, &mut track);

    T::deserialize(pd).map_err(|error| anyhow::anyhow!("at path '{}': {}", track.path(), error))
}
