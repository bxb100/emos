pub mod fs;

use std::fmt::Write;

use anyhow::Result;
use anyhow::bail;
use reqwest::RequestBuilder;
use serde::de::DeserializeOwned;
use tracing::instrument;

pub trait SqlInClause {
    fn to_sql_in_clause(&self) -> Result<String>;
}

impl SqlInClause for Vec<i64> {
    fn to_sql_in_clause(&self) -> Result<String> {
        if self.is_empty() {
            bail!("Cannot create SQL IN clause from empty vector");
        }
        let mut id_str = String::with_capacity(self.len() * 10);
        for (i, id) in self.iter().enumerate() {
            if i > 0 {
                id_str.push(',');
            }
            write!(id_str, "{}", id)?;
        }
        Ok(id_str)
    }
}

pub trait ReqwestExt {
    fn execute<T: DeserializeOwned>(self) -> impl Future<Output = Result<T>>;
}

impl ReqwestExt for RequestBuilder {
    async fn execute<T: DeserializeOwned>(self) -> Result<T> {
        let resp = self.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("API request failed: {} - {}", status, text);
        }
        let result = resp.json_ext().await?;
        Ok(result)
    }
}

trait ResponseExt {
    async fn json_ext<T: serde::de::DeserializeOwned>(self) -> anyhow::Result<T>;
}

impl ResponseExt for reqwest::Response {
    #[cfg(any(test, feature = "test-verbose"))]
    #[instrument(skip(self))]
    async fn json_ext<T: DeserializeOwned>(self) -> Result<T> {
        tracing::info!(status = %self.status(), url = %self.url());

        let text = self.text().await?;
        let jd = &mut serde_json::Deserializer::from_str(&text);
        let mut track = serde_path_to_error::Track::new();

        let pd = serde_path_to_error::Deserializer::new(jd, &mut track);

        match T::deserialize(pd) {
            Ok(data) => Ok(data),
            Err(error) => {
                let path = track.path().to_string();
                let msg = format!("at path '{}': {}", path, error);
                Err(anyhow::anyhow!(msg))
            }
        }
    }

    #[cfg(not(any(test, feature = "test-verbose")))]
    #[instrument(skip(self))]
    async fn json_ext<T: DeserializeOwned>(self) -> Result<T> {
        tracing::debug!(status = %self.status(), url = %self.url());

        self.json::<T>().await.map_err(Into::into)
    }
}
