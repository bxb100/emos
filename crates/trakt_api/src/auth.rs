use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::sync::Weak;
use std::time::Duration;
use std::time::SystemTime;

use anyhow::Context;
use cache::Cache;
use reqwest::Client;
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::header;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::sleep_until;
use tokio::time::timeout_at;
use utils::fs::project_root;

pub const DEFAULT_AUTH_BASE_URL: &str = "https://auth.trakt.tv";

const DEFAULT_REDIRECT_URI: &str = "urn:ietf:wg:oauth:2.0:oob";
const TOKEN_CACHE_KEY_PREFIX: &str = "trakt.oauth.v1:";
const TOKEN_REFRESH_SKEW: Duration = Duration::from_secs(60);

type RefreshLockKey = (PathBuf, String);
type RefreshLockMap = HashMap<RefreshLockKey, Weak<Mutex<()>>>;

static REFRESH_LOCKS: OnceLock<StdMutex<RefreshLockMap>> = OnceLock::new();

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

    pub fn expires_in(&self) -> u64 {
        self.expires_in
    }

    pub fn interval(&self) -> u64 {
        self.interval
    }
}

/// Trakt Device OAuth client with a repository-local, durable token cache.
///
/// This type deliberately has no `Debug` implementation because it owns the client secret and
/// cached OAuth credentials.
pub struct TraktAuth {
    client: Client,
    client_id: String,
    client_secret: String,
    auth_base_url: Url,
    token_cache_path: PathBuf,
    token_cache_key: String,
    refresh_lock: Arc<Mutex<()>>,
}

impl TraktAuth {
    pub fn new() -> anyhow::Result<Self> {
        Self::from_env()
    }

    pub fn from_env() -> anyhow::Result<Self> {
        let client_id = crate::env_or_default("TRAKT_CLIENT_ID", crate::DEFAULT_CLIENT_ID);
        let client_secret =
            crate::env_or_default("TRAKT_CLIENT_SECRET", crate::DEFAULT_CLIENT_SECRET);
        let cache_path = project_root().join("data/cache/trakt_auth");

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
    /// different deployments. Production callers should normally use [`Self::new`].
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

        let headers = crate::trakt_headers(client_id)?;
        let client = Client::builder()
            .user_agent(concat!(
                "emos-trakt-api/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/bxb100/emos)"
            ))
            .default_headers(headers)
            .build()
            .context("failed to build Trakt OAuth HTTP client")?;
        let auth_base_url = crate::normalize_base_url(auth_base_url.as_ref())?;
        let token_cache_path = cache_path.as_ref().to_path_buf();
        Cache::<String, TokenBundle>::with_path(&token_cache_path, Duration::ZERO)
            .context("failed to open Trakt token cache")?;
        let token_cache_key = format!("{TOKEN_CACHE_KEY_PREFIX}{client_id}");
        let refresh_lock = shared_refresh_lock(&token_cache_path, &token_cache_key);

        Ok(Self {
            client,
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            auth_base_url,
            token_cache_path,
            token_cache_key,
            refresh_lock,
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
                    let _refresh_guard = self.refresh_lock.lock().await;
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

    /// Returns a usable access token, refreshing and durably rotating it when necessary.
    pub async fn access_token(&self) -> anyhow::Result<String> {
        let _refresh_guard = self.refresh_lock.lock().await;
        let token = self
            .load_token()
            .await?
            .with_context(reauthorization_message)?;
        if !token.needs_refresh(unix_timestamp()?) {
            return Ok(token.access_token);
        }

        let refreshed = self.refresh_token(&token.refresh_token).await?;
        self.persist_token(&refreshed).await?;
        Ok(refreshed.access_token)
    }

    async fn load_token(&self) -> anyhow::Result<Option<TokenBundle>> {
        self.token_cache()?
            .get(&self.token_cache_key)
            .await
            .context("failed to read the cached Trakt token")
    }

    async fn persist_token(&self, token: &TokenBundle) -> anyhow::Result<()> {
        let token_cache = self.token_cache()?;
        token_cache
            .set(self.token_cache_key.clone(), token)
            .await
            .context("failed to update the cached Trakt token")?;
        token_cache
            .save()
            .await
            .context("failed to persist the cached Trakt token")
    }

    async fn delete_token(&self) -> anyhow::Result<()> {
        let token_cache = self.token_cache()?;
        token_cache
            .delete(&self.token_cache_key)
            .await
            .context("failed to remove the cached Trakt token")?;
        token_cache
            .save()
            .await
            .context("failed to persist removal of the cached Trakt token")
    }

    fn token_cache(&self) -> anyhow::Result<Cache<String, TokenBundle>> {
        Cache::with_path(&self.token_cache_path, Duration::ZERO)
            .context("failed to open Trakt token cache")
    }

    async fn refresh_token(&self, refresh_token: &str) -> anyhow::Result<TokenBundle> {
        let url = self
            .auth_base_url
            .join("oauth/token")
            .context("failed to build Trakt token-refresh URL")?;
        let response = self
            .client
            .post(url)
            .json(&RefreshTokenRequest {
                refresh_token,
                client_id: &self.client_id,
                client_secret: &self.client_secret,
                redirect_uri: DEFAULT_REDIRECT_URI,
                grant_type: "refresh_token",
            })
            .send()
            .await
            .context("failed to refresh the Trakt access token")?;
        let status = response.status();

        if status.is_success() {
            let token = response
                .json::<TokenBundle>()
                .await
                .context("failed to decode the refreshed Trakt OAuth token")?;
            token.validate()?;
            return Ok(token);
        }

        let oauth_error = response.json::<OAuthErrorResponse>().await.ok();
        if oauth_error.as_ref().map(|error| error.error.as_str()) == Some("invalid_grant") {
            if let Some(rotated_token) = self.recover_rotated_refresh_token(refresh_token).await? {
                return Ok(rotated_token);
            }
            self.delete_token().await?;
            anyhow::bail!(
                "cached Trakt authorization is no longer valid; {}",
                reauthorization_message()
            )
        }

        Err(oauth_status_error("token refresh", status, oauth_error))
    }

    // Trakt refresh tokens are single-use. If another process already rotated this token,
    // prefer the replacement now stored in the shared cache instead of deleting it.
    async fn recover_rotated_refresh_token(
        &self,
        attempted_refresh_token: &str,
    ) -> anyhow::Result<Option<TokenBundle>> {
        let Some(cached) = self.load_token().await? else {
            return Ok(None);
        };
        if cached.refresh_token == attempted_refresh_token {
            return Ok(None);
        }

        cached.validate()?;
        Ok(Some(cached))
    }
}

fn shared_refresh_lock(cache_path: &Path, cache_key: &str) -> Arc<Mutex<()>> {
    let cache_path = cache_path.with_extension("mpbr");
    let cache_path = if cache_path.is_absolute() {
        cache_path
    } else {
        std::env::current_dir().map_or(cache_path.clone(), |directory| directory.join(cache_path))
    };
    let key = (cache_path, cache_key.to_string());
    let lock_map = REFRESH_LOCKS.get_or_init(|| StdMutex::new(HashMap::new()));
    let mut lock_map = lock_map.lock().unwrap_or_else(|error| error.into_inner());
    lock_map.retain(|_, lock| lock.strong_count() > 0);

    if let Some(lock) = lock_map.get(&key).and_then(Weak::upgrade) {
        return lock;
    }

    let lock = Arc::new(Mutex::new(()));
    lock_map.insert(key, Arc::downgrade(&lock));
    lock
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

#[derive(Serialize)]
struct RefreshTokenRequest<'a> {
    refresh_token: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
    redirect_uri: &'a str,
    grant_type: &'a str,
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

    fn needs_refresh(&self, now: u64) -> bool {
        now.saturating_add(TOKEN_REFRESH_SKEW.as_secs())
            >= self.created_at.saturating_add(self.expires_in)
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

fn unix_timestamp() -> anyhow::Result<u64> {
    SystemTime::UNIX_EPOCH
        .elapsed()
        .context("system clock is before the Unix epoch")
        .map(|duration| duration.as_secs())
}

fn reauthorization_message() -> &'static str {
    "run the `trakt_auth` task to authorize Trakt again"
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
        assert_eq!(pending.expires_in(), 600);
        assert_eq!(pending.interval(), 1);
        auth.complete_device_authorization(pending).await?;
        drop(auth);

        let auth = test_auth("http://127.0.0.1:9", &cache_path)?;
        assert_eq!(auth.access_token().await?, "first-access");

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

    #[tokio::test]
    async fn expiring_token_is_refreshed_once_and_rotated_durably() -> anyhow::Result<()> {
        let now = unix_timestamp()?;
        let responses = vec![json_response(
            "200 OK",
            token_json("new-access", "new-refresh", now, 3600),
        )];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let directory = tempfile::tempdir()?;
        let cache_path = directory.path().join("trakt_auth");
        let first_auth = test_auth(&base_url, &cache_path)?;
        let second_auth = test_auth(&base_url, &cache_path)?;
        first_auth
            .persist_token(&token("old-access", "old-refresh", now, 30))
            .await?;

        let (first, second) = tokio::join!(first_auth.access_token(), second_auth.access_token());

        assert_eq!(first?, "new-access");
        assert_eq!(second?, "new-access");
        let requests = server.await?;
        assert_eq!(requests.len(), 1);
        assert_json_request(
            &requests[0],
            "/oauth/token",
            serde_json::json!({
                "refresh_token": "old-refresh",
                "client_id": CLIENT_ID,
                "client_secret": CLIENT_SECRET,
                "redirect_uri": DEFAULT_REDIRECT_URI,
                "grant_type": "refresh_token",
            }),
        );
        drop(first_auth);
        drop(second_auth);

        let auth = test_auth("http://127.0.0.1:9", &cache_path)?;
        let cached = auth.load_token().await?.context("token should be cached")?;
        assert_eq!(cached.access_token, "new-access");
        assert_eq!(cached.refresh_token, "new-refresh");
        assert_eq!(cached.token_type, "bearer");
        assert_eq!(cached.scope, "public");
        assert_eq!(cached.created_at, now);
        assert_eq!(cached.expires_in, 3600);
        Ok(())
    }

    #[tokio::test]
    async fn invalid_grant_removes_the_cached_token() -> anyhow::Result<()> {
        let now = unix_timestamp()?;
        let responses = vec![json_response(
            "400 Bad Request",
            r#"{"error":"invalid_grant"}"#,
        )];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let directory = tempfile::tempdir()?;
        let cache_path = directory.path().join("trakt_auth");
        let auth = test_auth(&base_url, &cache_path)?;
        auth.persist_token(&token("old-access", "old-refresh", now, 30))
            .await?;

        let error = auth
            .access_token()
            .await
            .expect_err("invalid grant should fail");

        assert!(error.to_string().contains("trakt_auth"), "{error}");
        assert_eq!(server.await?.len(), 1);
        drop(auth);

        let auth = test_auth("http://127.0.0.1:9", &cache_path)?;
        assert!(auth.load_token().await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn invalid_grant_preserves_a_token_rotated_by_another_process() -> anyhow::Result<()> {
        let now = unix_timestamp()?;
        let responses = vec![json_response(
            "400 Bad Request",
            r#"{"error":"invalid_grant"}"#,
        )];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let directory = tempfile::tempdir()?;
        let cache_path = directory.path().join("trakt_auth");
        let auth = test_auth(&base_url, &cache_path)?;
        auth.persist_token(&token("old-access", "old-refresh", now, 30))
            .await?;

        let other_process = test_auth("http://127.0.0.1:9", &cache_path)?;
        other_process
            .persist_token(&token("new-access", "new-refresh", now, 3600))
            .await?;

        let refreshed = auth.refresh_token("old-refresh").await?;

        assert_eq!(refreshed.access_token, "new-access");
        assert_eq!(refreshed.refresh_token, "new-refresh");
        assert_eq!(server.await?.len(), 1);
        drop(auth);
        drop(other_process);

        let auth = test_auth("http://127.0.0.1:9", &cache_path)?;
        let cached = auth
            .load_token()
            .await?
            .context("token should stay cached")?;
        assert_eq!(cached.access_token, "new-access");
        assert_eq!(cached.refresh_token, "new-refresh");
        Ok(())
    }

    #[tokio::test]
    async fn valid_cached_token_does_not_use_the_network() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let auth = test_auth("http://127.0.0.1:9", &directory.path().join("trakt_auth"))?;
        auth.persist_token(&token("valid-access", "refresh", unix_timestamp()?, 3600))
            .await?;

        assert_eq!(auth.access_token().await?, "valid-access");
        Ok(())
    }

    fn test_auth(base_url: &str, cache_path: &Path) -> anyhow::Result<TraktAuth> {
        TraktAuth::with_client_credentials_and_base_url(
            CLIENT_ID,
            CLIENT_SECRET,
            base_url,
            cache_path,
        )
    }

    fn token(access: &str, refresh: &str, created_at: u64, expires_in: u64) -> TokenBundle {
        TokenBundle {
            access_token: access.to_string(),
            token_type: "bearer".to_string(),
            expires_in,
            refresh_token: refresh.to_string(),
            scope: "public".to_string(),
            created_at,
        }
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
