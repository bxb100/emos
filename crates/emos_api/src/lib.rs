use anyhow::Result;
use dotenv_codegen::dotenv;
use reqwest::Client;
use reqwest::header;
use serde::de::DeserializeOwned;
use tracing::instrument;

pub mod video;

pub struct EmosApi {
    pub client: Client,
    pub base_url: String,
}

impl EmosApi {
    pub fn new() -> Result<Self> {
        let mut headers = header::HeaderMap::new();

        let value = format!("Bearer {}", dotenv!("EMOS_TOKEN"));
        let mut auth_value = header::HeaderValue::from_str(&value)?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        Ok(EmosApi {
            client: Client::builder()
                .user_agent("emos-rs/api")
                .default_headers(headers)
                .build()?,

            base_url: dotenv!("EMOS_API_URL").to_string(),
        })
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
