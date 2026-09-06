use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use emos_cache::FileCache;
use reqwest::Client as HttpClient;
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::header;
use serde::Deserialize;
use serde::Serialize;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::sleep_until;
use tokio::time::timeout_at;

pub const DEFAULT_AUTH_BASE_URL: &str = "https://auth.trakt.tv";

const TOKEN_CACHE_KEY_PREFIX: &str = "trakt.oauth.v1:";

/// A pending Trakt device authorization.
///
/// Only the values intended for display are exposed. The device code remains private so callers
/// cannot accidentally print it alongside the user-facing code.
pub struct DeviceAuthorization {
    device_code: String,
    user_code: String,
    verification_url: String,
    expires_in: u64,
    interval: u64,
    requested_at: Instant,
}

impl DeviceAuthorization {
    pub fn user_code(&self) -> &str {
        &self.user_code
    }

    pub fn verification_url(&self) -> &str {
        &self.verification_url
    }
}

/// Trakt Device OAuth client with a durable token cache at the caller-provided path.
///
/// This type deliberately has no `Debug` implementation because it owns the client secret and
/// cached OAuth credentials.
pub struct AuthClient {
    client: HttpClient,
    client_id: String,
    client_secret: String,
    auth_base_url: Url,
    token_cache_path: PathBuf,
    token_cache_key: String,
}

impl AuthClient {
    /// Creates an OAuth client with a token cache at `cache_path`.
    ///
    /// Credentials come from `TRAKT_CLIENT_ID` and `TRAKT_CLIENT_SECRET`; unset or blank
    /// variables fall back to the published application credentials.
    pub fn from_env(cache_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let client_id = crate::client::env_or_default("TRAKT_CLIENT_ID", crate::DEFAULT_CLIENT_ID);
        let client_secret =
            crate::client::env_or_default("TRAKT_CLIENT_SECRET", crate::DEFAULT_CLIENT_SECRET);

        Self::with_client_credentials_and_base_url(
            client_id,
            client_secret,
            DEFAULT_AUTH_BASE_URL,
            cache_path,
        )
    }

    /// Creates an OAuth client with injectable network and cache locations.
    ///
    /// The custom locations keep tests fully offline and allow callers to isolate credentials for
    /// different deployments. Production callers should normally use [`Self::from_env`].
    pub fn with_client_credentials_and_base_url(
        client_id: impl AsRef<str>,
        client_secret: impl AsRef<str>,
        auth_base_url: impl AsRef<str>,
        cache_path: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        let client_id = client_id.as_ref().trim();
        let client_secret = client_secret.as_ref().trim();
        anyhow::ensure!(!client_id.is_empty(), "TRAKT_CLIENT_ID must not be empty");
        anyhow::ensure!(
            !client_secret.is_empty(),
            "TRAKT_CLIENT_SECRET must not be empty"
        );

        let headers = crate::client::trakt_headers(client_id)?;
        let client = HttpClient::builder()
            .user_agent(concat!(
                "emos-trakt-api/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/bxb100/emos)"
            ))
            .default_headers(headers)
            .build()
            .context("failed to build Trakt OAuth HTTP client")?;
        let auth_base_url = crate::client::normalize_base_url(auth_base_url.as_ref())?;
        let token_cache_path = cache_path.as_ref().to_path_buf();
        let token_cache_key = format!("{TOKEN_CACHE_KEY_PREFIX}{client_id}");

        Ok(Self {
            client,
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            auth_base_url,
            token_cache_path,
            token_cache_key,
        })
    }

    // Device OAuth flow based on DuckieTV, with bounded polling and safe token rotation:
    // https://github.com/SchizoDuckie/DuckieTV/blob/8825daba0960939fe199352c4fe8965d0bfb5be0/js/services/TraktTVv2.js#L423-L536
    pub async fn request_device_authorization(&self) -> anyhow::Result<DeviceAuthorization> {
        let url = self
            .auth_base_url
            .join("oauth/device/code")
            .context("failed to build Trakt device-code URL")?;
        let response = self
            .client
            .post(url)
            .json(&DeviceCodeRequest {
                client_id: &self.client_id,
            })
            .send()
            .await
            .context("failed to request Trakt device authorization")?;
        let status = response.status();
        if !status.is_success() {
            return Err(oauth_response_error("device authorization", status, response).await);
        }

        let response = response
            .json::<DeviceCodeResponse>()
            .await
            .context("failed to decode Trakt device authorization")?;
        anyhow::ensure!(
            !response.device_code.trim().is_empty(),
            "Trakt returned an empty device code"
        );
        anyhow::ensure!(
            !response.user_code.trim().is_empty(),
            "Trakt returned an empty user code"
        );
        anyhow::ensure!(
            !response.verification_url.trim().is_empty(),
            "Trakt returned an empty verification URL"
        );
        anyhow::ensure!(
            response.expires_in > 0,
            "Trakt returned a zero-second device authorization lifetime"
        );
        anyhow::ensure!(
            response.interval > 0,
            "Trakt returned a zero-second polling interval"
        );

        Ok(DeviceAuthorization {
            device_code: response.device_code,
            user_code: response.user_code,
            verification_url: response.verification_url,
            expires_in: response.expires_in,
            interval: response.interval,
            requested_at: Instant::now(),
        })
    }

    /// Polls Trakt sequentially until the user authorizes, rejects, or the device code expires.
    ///
    /// On success the complete token response is persisted before this method returns. Tokens are
    /// intentionally not returned, which makes it harder for CLI callers to print them.
    pub async fn complete_device_authorization(
        &self,
        authorization: DeviceAuthorization,
    ) -> anyhow::Result<()> {
        let deadline = authorization
            .requested_at
            .checked_add(Duration::from_secs(authorization.expires_in))
            .context("Trakt device authorization lifetime is too large")?;
        let url = self
            .auth_base_url
            .join("oauth/device/token")
            .context("failed to build Trakt device-token URL")?;

        loop {
            anyhow::ensure!(
                Instant::now() < deadline,
                "Trakt device authorization expired; start authorization again"
            );
            let response = timeout_at(
                deadline,
                self.client
                    .post(url.clone())
                    .json(&DeviceTokenRequest {
                        code: &authorization.device_code,
                        client_id: &self.client_id,
                        client_secret: &self.client_secret,
                    })
                    .send(),
            )
            .await
            .context("Trakt device authorization expired while polling")?
            .context("failed to poll Trakt device authorization")?;
            let status = response.status();

            match status {
                StatusCode::OK => {
                    let token = timeout_at(deadline, response.json::<TokenBundle>())
                        .await
                        .context("Trakt device authorization expired while reading its token")?
                        .context("failed to decode Trakt OAuth token")?;
                    token.validate()?;
                    self.persist_token(&token).await?;
                    return Ok(());
                }
                StatusCode::BAD_REQUEST => {
                    wait_for_retry(deadline, authorization.interval).await?;
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    let retry_after =
                        retry_after_seconds(response.headers()).unwrap_or(authorization.interval);
                    wait_for_retry(deadline, retry_after).await?;
                }
                StatusCode::NOT_FOUND => {
                    anyhow::bail!(
                        "Trakt rejected the device code as invalid; start authorization again"
                    )
                }
                StatusCode::CONFLICT => {
                    anyhow::bail!(
                        "Trakt device code has already been used; start authorization again"
                    )
                }
                StatusCode::GONE => {
                    anyhow::bail!("Trakt device authorization expired; start authorization again")
                }
                StatusCode::IM_A_TEAPOT => {
                    anyhow::bail!("Trakt device authorization was denied by the user")
                }
                _ => {
                    return Err(
                        oauth_response_error("device-token polling", status, response).await,
                    );
                }
            }
        }
    }

    async fn persist_token(&self, token: &TokenBundle) -> anyhow::Result<()> {
        let token_cache = self.token_cache();
        token_cache
            .set(self.token_cache_key.clone(), token)
            .await
            .context("failed to update the cached Trakt token")?;
        token_cache
            .save()
            .await
            .context("failed to persist the cached Trakt token")
    }

    fn token_cache(&self) -> FileCache<TokenBundle> {
        FileCache::new(&self.token_cache_path)
    }
}

#[derive(Serialize)]
struct DeviceCodeRequest<'a> {
    client_id: &'a str,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_url: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Serialize)]
struct DeviceTokenRequest<'a> {
    code: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
}

#[derive(Clone, Serialize, Deserialize)]
struct TokenBundle {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: String,
    scope: String,
    created_at: u64,
}

impl TokenBundle {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.access_token.trim().is_empty(),
            "Trakt returned an empty access token"
        );
        anyhow::ensure!(
            !self.refresh_token.trim().is_empty(),
            "Trakt returned an empty refresh token"
        );
        anyhow::ensure!(
            !self.token_type.trim().is_empty(),
            "Trakt returned an empty token type"
        );
        anyhow::ensure!(
            self.expires_in > 0,
            "Trakt returned a zero-second access-token lifetime"
        );
        anyhow::ensure!(
            self.created_at > 0,
            "Trakt returned an invalid creation time"
        );
        Ok(())
    }
}

#[derive(Deserialize)]
struct OAuthErrorResponse {
    error: String,
}

fn retry_after_seconds(headers: &header::HeaderMap) -> Option<u64> {
    headers
        .get(header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|seconds| *seconds > 0)
}

async fn wait_for_retry(deadline: Instant, seconds: u64) -> anyhow::Result<()> {
    let now = Instant::now();
    anyhow::ensure!(
        now < deadline,
        "Trakt device authorization expired; start authorization again"
    );
    let delay = Duration::from_secs(seconds);
    if delay >= deadline.duration_since(now) {
        sleep_until(deadline).await;
        anyhow::bail!("Trakt device authorization expired; start authorization again");
    }

    sleep(delay).await;
    Ok(())
}

async fn oauth_response_error(
    operation: &'static str,
    status: StatusCode,
    response: reqwest::Response,
) -> anyhow::Error {
    let oauth_error = response.json::<OAuthErrorResponse>().await.ok();
    oauth_status_error(operation, status, oauth_error)
}

fn oauth_status_error(
    operation: &'static str,
    status: StatusCode,
    oauth_error: Option<OAuthErrorResponse>,
) -> anyhow::Error {
    match oauth_error {
        Some(error) => anyhow::anyhow!(
            "Trakt OAuth {operation} failed with HTTP {status} ({})",
            error.error
        ),
        None => anyhow::anyhow!("Trakt OAuth {operation} failed with HTTP {status}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::Value;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    use super::*;

    const CLIENT_ID: &str = "test-client";
    const CLIENT_SECRET: &str = "test-secret";

    struct MockResponse {
        status: &'static str,
        headers: Vec<(&'static str, &'static str)>,
        body: String,
    }

    struct RecordedRequest {
        method: String,
        target: String,
        headers: HashMap<String, String>,
        body: String,
    }

    #[tokio::test]
    async fn device_flow_polls_and_persists_the_complete_token() -> anyhow::Result<()> {
        let created_at = unix_timestamp()?;
        let responses = vec![
            json_response(
                "200 OK",
                r#"{"device_code":"private-device","user_code":"ABCD1234","verification_url":"https://trakt.tv/activate","expires_in":600,"interval":1}"#,
            ),
            json_response("400 Bad Request", r#"{"error":"authorization_pending"}"#),
            json_response(
                "200 OK",
                token_json("first-access", "first-refresh", created_at, 3600),
            ),
        ];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let directory = tempfile::tempdir()?;
        let cache_path = directory.path().join("trakt_auth");
        let auth = test_auth(&base_url, &cache_path)?;

        let pending = auth.request_device_authorization().await?;
        assert_eq!(pending.verification_url(), "https://trakt.tv/activate");
        assert_eq!(pending.user_code(), "ABCD1234");
        assert_eq!(pending.expires_in, 600);
        assert_eq!(pending.interval, 1);
        auth.complete_device_authorization(pending).await?;
        drop(auth);

        let auth = test_auth("http://127.0.0.1:9", &cache_path)?;
        let cached = auth
            .token_cache()
            .get(&auth.token_cache_key)
            .await?
            .context("token should be persisted across instances")?;
        assert_eq!(cached.access_token, "first-access");

        let requests = server.await?;
        assert_eq!(requests.len(), 3);
        assert_json_request(
            &requests[0],
            "/oauth/device/code",
            serde_json::json!({"client_id": CLIENT_ID}),
        );
        assert_json_request(
            &requests[1],
            "/oauth/device/token",
            serde_json::json!({
                "code": "private-device",
                "client_id": CLIENT_ID,
                "client_secret": CLIENT_SECRET,
            }),
        );
        assert_json_request(
            &requests[2],
            "/oauth/device/token",
            serde_json::json!({
                "code": "private-device",
                "client_id": CLIENT_ID,
                "client_secret": CLIENT_SECRET,
            }),
        );
        Ok(())
    }

    #[tokio::test]
    async fn rate_limit_honors_retry_after() -> anyhow::Result<()> {
        let created_at = unix_timestamp()?;
        let responses = vec![
            json_response(
                "200 OK",
                r#"{"device_code":"device","user_code":"CODE","verification_url":"https://trakt.tv/activate","expires_in":10,"interval":4}"#,
            ),
            MockResponse {
                status: "429 Too Many Requests",
                headers: vec![("Retry-After", "1")],
                body: String::new(),
            },
            json_response("200 OK", token_json("access", "refresh", created_at, 3600)),
        ];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let directory = tempfile::tempdir()?;
        let auth = test_auth(&base_url, &directory.path().join("trakt_auth"))?;
        let pending = auth.request_device_authorization().await?;
        let started_at = Instant::now();

        auth.complete_device_authorization(pending).await?;

        let elapsed = Instant::now().duration_since(started_at);
        assert!(elapsed >= Duration::from_secs(1));
        assert!(elapsed < Duration::from_secs(3));
        assert_eq!(server.await?.len(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn polling_stops_at_the_local_expiry_deadline() -> anyhow::Result<()> {
        let responses = vec![
            json_response(
                "200 OK",
                r#"{"device_code":"device","user_code":"CODE","verification_url":"https://trakt.tv/activate","expires_in":1,"interval":5}"#,
            ),
            json_response("400 Bad Request", r#"{"error":"authorization_pending"}"#),
        ];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let directory = tempfile::tempdir()?;
        let auth = test_auth(&base_url, &directory.path().join("trakt_auth"))?;
        let pending = auth.request_device_authorization().await?;

        let error = auth
            .complete_device_authorization(pending)
            .await
            .expect_err("polling should expire locally");

        assert!(error.to_string().contains("expired"));
        assert_eq!(server.await?.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn terminal_device_statuses_do_not_retry() -> anyhow::Result<()> {
        for (status, expected) in [
            ("404 Not Found", "invalid"),
            ("409 Conflict", "already been used"),
            ("410 Gone", "expired"),
            ("418 I'm a teapot", "denied"),
        ] {
            let responses = vec![
                json_response(
                    "200 OK",
                    r#"{"device_code":"device","user_code":"CODE","verification_url":"https://trakt.tv/activate","expires_in":60,"interval":1}"#,
                ),
                json_response(status, "{}"),
            ];
            let (base_url, server) = spawn_mock_server(responses).await?;
            let directory = tempfile::tempdir()?;
            let auth = test_auth(&base_url, &directory.path().join("trakt_auth"))?;
            let pending = auth.request_device_authorization().await?;

            let error = auth
                .complete_device_authorization(pending)
                .await
                .expect_err("terminal status should fail");

            assert!(error.to_string().contains(expected), "{error}");
            assert_eq!(server.await?.len(), 2);
        }
        Ok(())
    }

    fn test_auth(base_url: &str, cache_path: &Path) -> anyhow::Result<AuthClient> {
        AuthClient::with_client_credentials_and_base_url(
            CLIENT_ID,
            CLIENT_SECRET,
            base_url,
            cache_path,
        )
    }

    fn unix_timestamp() -> anyhow::Result<u64> {
        Ok(std::time::SystemTime::UNIX_EPOCH.elapsed()?.as_secs())
    }

    fn token_json(access: &str, refresh: &str, created_at: u64, expires_in: u64) -> String {
        serde_json::json!({
            "access_token": access,
            "token_type": "bearer",
            "expires_in": expires_in,
            "refresh_token": refresh,
            "scope": "public",
            "created_at": created_at,
        })
        .to_string()
    }

    fn json_response(status: &'static str, body: impl Into<String>) -> MockResponse {
        MockResponse {
            status,
            headers: vec![("Content-Type", "application/json")],
            body: body.into(),
        }
    }

    async fn spawn_mock_server(
        responses: Vec<MockResponse>,
    ) -> anyhow::Result<(String, JoinHandle<Vec<RecordedRequest>>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move {
            let mut requests = Vec::with_capacity(responses.len());
            for response in responses {
                let (mut stream, _) = listener.accept().await.expect("mock server accept failed");
                let raw_request = read_request(&mut stream)
                    .await
                    .expect("mock server failed to read request");
                requests.push(parse_request(&raw_request));

                let mut raw_response = format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                    response.status,
                    response.body.len()
                );
                for (name, value) in response.headers {
                    raw_response.push_str(&format!("{name}: {value}\r\n"));
                }
                raw_response.push_str("\r\n");
                raw_response.push_str(&response.body);
                stream
                    .write_all(raw_response.as_bytes())
                    .await
                    .expect("mock server failed to write response");
            }
            requests
        });

        Ok((format!("http://{address}"), server))
    }

    async fn read_request(stream: &mut tokio::net::TcpStream) -> std::io::Result<String> {
        let mut bytes = Vec::new();
        let mut expected_length = None;
        loop {
            let mut buffer = [0; 1024];
            let read = stream.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);

            if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let body_start = header_end + 4;
                let content_length = *expected_length.get_or_insert_with(|| {
                    let headers = String::from_utf8_lossy(&bytes[..header_end]);
                    headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0)
                });
                if bytes.len() >= body_start + content_length {
                    break;
                }
            }
        }
        Ok(String::from_utf8(bytes).expect("request should be valid UTF-8"))
    }

    fn parse_request(request: &str) -> RecordedRequest {
        let (head, body) = request
            .split_once("\r\n\r\n")
            .expect("request should contain header terminator");
        let mut lines = head.lines();
        let request_line = lines.next().expect("request line should be present");
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .expect("method should be present")
            .to_string();
        let target = request_parts
            .next()
            .expect("target should be present")
            .to_string();
        let headers = lines
            .map(|line| {
                let (name, value) = line.split_once(':').expect("header should contain colon");
                (name.to_ascii_lowercase(), value.trim().to_string())
            })
            .collect();

        RecordedRequest {
            method,
            target,
            headers,
            body: body.to_string(),
        }
    }

    fn assert_json_request(request: &RecordedRequest, path: &str, expected_body: Value) {
        assert_eq!(request.method, "POST");
        assert_eq!(request.target, path);
        assert_eq!(request.headers["content-type"], "application/json");
        assert_eq!(
            request.headers["user-agent"],
            concat!(
                "emos-trakt-api/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/bxb100/emos)"
            )
        );
        assert_eq!(request.headers["trakt-api-key"], CLIENT_ID);
        assert_eq!(request.headers["trakt-api-version"], "2");
        assert_eq!(
            serde_json::from_str::<Value>(&request.body).expect("request body should be JSON"),
            expected_body
        );
    }
}
