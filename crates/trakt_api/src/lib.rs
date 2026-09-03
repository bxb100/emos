pub mod auth;
pub mod model;

use std::collections::HashSet;
use std::env;

use anyhow::Context;
use model::Pagination;
use model::TrendingItem;
use model::TrendingPage;
use reqwest::Client;
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

const TRENDING_PAGE_LIMIT: u64 = 250;
const TRAKT_API_KEY: &str = "trakt-api-key";
const TRAKT_API_VERSION: &str = "trakt-api-version";

pub struct TraktApi {
    client: Client,
    base_url: Url,
}

impl TraktApi {
    pub fn new() -> anyhow::Result<Self> {
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

        let client = Client::builder()
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

    /// Fetches one page from Trakt's mixed movie/show trending feed.
    ///
    /// See <https://docs.trakt.tv/reference/getmediatrending>.
    pub async fn trending(&self, page: u64, limit: u64) -> anyhow::Result<TrendingPage> {
        anyhow::ensure!(page >= 1, "Trakt page must be at least 1");
        anyhow::ensure!(
            (1..=TRENDING_PAGE_LIMIT).contains(&limit),
            "Trakt limit must be between 1 and {TRENDING_PAGE_LIMIT}"
        );

        let url = self
            .base_url
            .join("media/trending")
            .context("failed to build Trakt trending URL")?;
        let response = self
            .client
            .get(url)
            .query(&[("page", page), ("limit", limit)])
            .send()
            .await
            .context("failed to request Trakt trending media")?;

        let status = response.status();
        if !status.is_success() {
            return Err(response_error(status, response).await);
        }

        let pagination = parse_pagination(response.headers())?;
        let items = response
            .json::<Vec<TrendingItem>>()
            .await
            .context("failed to decode Trakt trending response")?;

        Ok(TrendingPage { items, pagination })
    }

    /// Fetches every page while preserving Trakt's mixed-media order.
    ///
    /// Completion follows `X-Pagination-Page-Count` because this mixed feed can return a short
    /// intermediate page. Items are de-duplicated because the live ranking can move while its
    /// pages are being fetched.
    pub async fn all_trending(&self) -> anyhow::Result<Vec<TrendingItem>> {
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        let mut page = 1;

        loop {
            let page_data = self.trending(page, TRENDING_PAGE_LIMIT).await?;
            let reached_last_page = page >= page_data.pagination.page_count;

            items.extend(
                page_data
                    .items
                    .into_iter()
                    .filter(|item| seen.insert(item_key(item))),
            );

            if reached_last_page {
                break;
            }

            page += 1;
        }

        Ok(items)
    }
}

fn env_or_default(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map_or_else(|| default.to_string(), |value| value.trim().to_string())
}

fn trakt_headers(client_id: &str) -> anyhow::Result<header::HeaderMap> {
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

fn normalize_base_url(base_url: &str) -> anyhow::Result<Url> {
    let normalized = format!("{}/", base_url.trim_end_matches('/'));
    Url::parse(&normalized).context("invalid Trakt base URL")
}

fn item_key(item: &TrendingItem) -> (u8, u64) {
    match item {
        TrendingItem::Movie { movie, .. } => (0, movie.ids.trakt),
        TrendingItem::Show { show, .. } => (1, show.ids.trakt),
    }
}

fn parse_pagination(headers: &header::HeaderMap) -> anyhow::Result<Pagination> {
    Ok(Pagination {
        page: pagination_header(headers, "x-pagination-page")?,
        limit: pagination_header(headers, "x-pagination-limit")?,
        page_count: pagination_header(headers, "x-pagination-page-count")?,
        item_count: pagination_header(headers, "x-pagination-item-count")?,
    })
}

fn pagination_header(headers: &header::HeaderMap, name: &'static str) -> anyhow::Result<u64> {
    let value = headers
        .get(name)
        .with_context(|| format!("Trakt response is missing {name}"))?;
    let value = value
        .to_str()
        .with_context(|| format!("Trakt response has invalid {name}"))?;
    value
        .parse()
        .with_context(|| format!("Trakt response has non-numeric {name}: {value}"))
}

async fn response_error(status: StatusCode, response: reqwest::Response) -> anyhow::Error {
    match response.text().await {
        Ok(body) => anyhow::anyhow!("Trakt API request failed with HTTP {status}: {body}"),
        Err(error) => anyhow::anyhow!(
            "Trakt API request failed with HTTP {status}; failed to read response body: {error}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    use super::*;

    struct MockResponse {
        status: &'static str,
        headers: Vec<(&'static str, String)>,
        body: String,
    }

    struct RecordedRequest {
        target: String,
        headers: HashMap<String, String>,
    }

    #[tokio::test]
    async fn all_trending_fetches_the_last_page_and_decodes_mixed_media() -> anyhow::Result<()> {
        let responses = vec![
            paginated_response(
                1,
                3,
                r#"[{"watchers":42,"movie":{"title":"Arrival","year":2016,"ids":{"trakt":1,"slug":"arrival-2016","imdb":"tt2543164","tmdb":329865}}}]"#,
            ),
            paginated_response(
                2,
                3,
                r#"[{"watchers":99,"movie":{"title":"Duplicate Arrival","year":2016,"ids":{"trakt":1,"slug":"arrival-2016","imdb":"tt2543164","tmdb":329865}}}]"#,
            ),
            paginated_response(
                3,
                3,
                r#"[{"watchers":17,"show":{"title":"Dark","year":2017,"ids":{"trakt":1,"slug":"dark","imdb":"tt5753856","tmdb":null}}}]"#,
            ),
        ];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let api = TraktApi::with_client_id_and_base_url("test-client-id", base_url)?;

        let items = api.all_trending().await?;

        assert_eq!(items.len(), 2);
        assert!(matches!(
            &items[0],
            TrendingItem::Movie { watchers: 42, movie }
                if movie.title == "Arrival" && movie.ids.tmdb == Some(329_865)
        ));
        assert!(matches!(
            &items[1],
            TrendingItem::Show { watchers: 17, show }
                if show.title == "Dark" && show.ids.tmdb.is_none()
        ));

        let requests = server.await?;
        assert_eq!(requests.len(), 3);
        assert_trending_request(&requests[0], 1, 250);
        assert_trending_request(&requests[1], 2, 250);
        assert_trending_request(&requests[2], 3, 250);
        Ok(())
    }

    #[tokio::test]
    async fn all_trending_keeps_fetching_when_page_count_grows() -> anyhow::Result<()> {
        let responses = vec![
            paginated_response(1, 2, trending_movies_body(1, 250)),
            paginated_response(2, 3, trending_movies_body(251, 250)),
            paginated_response(3, 3, trending_movies_body(501, 1)),
        ];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let api = TraktApi::with_client_id_and_base_url("test-client-id", base_url)?;

        let items = api.all_trending().await?;

        assert_eq!(items.len(), 501);
        let requests = server.await?;
        assert_eq!(requests.len(), 3);
        assert_trending_request(&requests[0], 1, 250);
        assert_trending_request(&requests[1], 2, 250);
        assert_trending_request(&requests[2], 3, 250);
        Ok(())
    }

    #[tokio::test]
    async fn trending_returns_pagination_and_sends_requested_query() -> anyhow::Result<()> {
        let responses = vec![MockResponse {
            status: "200 OK",
            headers: pagination_headers(3, 20, 7, 123),
            body: "[]".to_string(),
        }];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let api = TraktApi::with_client_id_and_base_url("query-client-id", base_url)?;

        let result = api.trending(3, 20).await?;

        assert_eq!(
            result.pagination,
            Pagination {
                page: 3,
                limit: 20,
                page_count: 7,
                item_count: 123,
            }
        );
        let requests = server.await?;
        assert_trending_request(&requests[0], 3, 20);
        assert_eq!(requests[0].headers[TRAKT_API_KEY], "query-client-id");
        Ok(())
    }

    #[tokio::test]
    async fn non_success_error_contains_status_and_body() -> anyhow::Result<()> {
        let responses = vec![MockResponse {
            status: "401 Unauthorized",
            headers: Vec::new(),
            body: r#"{"error":"invalid client id"}"#.to_string(),
        }];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let api = TraktApi::with_client_id_and_base_url("bad-client-id", base_url)?;

        let error = api.trending(1, 10).await.expect_err("request should fail");
        let message = error.to_string();
        assert!(message.contains("401 Unauthorized"), "{message}");
        assert!(
            message.contains(r#"{"error":"invalid client id"}"#),
            "{message}"
        );

        server.await?;
        Ok(())
    }

    #[tokio::test]
    async fn rejects_invalid_paging_before_sending_a_request() -> anyhow::Result<()> {
        let api = TraktApi::with_client_id_and_base_url("test-client-id", "http://127.0.0.1:9")?;

        let page_error = api.trending(0, 1).await.expect_err("page zero should fail");
        assert!(page_error.to_string().contains("page must be at least 1"));

        for limit in [0, 251] {
            let limit_error = api
                .trending(1, limit)
                .await
                .expect_err("out-of-range limit should fail");
            assert!(
                limit_error
                    .to_string()
                    .contains("limit must be between 1 and 250")
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn missing_pagination_header_is_an_error() -> anyhow::Result<()> {
        let responses = vec![MockResponse {
            status: "200 OK",
            headers: vec![
                ("X-Pagination-Page", "1".to_string()),
                ("X-Pagination-Limit", "10".to_string()),
                ("X-Pagination-Item-Count", "0".to_string()),
            ],
            body: "[]".to_string(),
        }];
        let (base_url, server) = spawn_mock_server(responses).await?;
        let api = TraktApi::with_client_id_and_base_url("test-client-id", base_url)?;

        let error = api
            .trending(1, 10)
            .await
            .expect_err("missing page count should fail");
        assert!(
            error
                .to_string()
                .contains("missing x-pagination-page-count")
        );

        server.await?;
        Ok(())
    }

    fn paginated_response(page: u64, page_count: u64, body: impl Into<String>) -> MockResponse {
        MockResponse {
            status: "200 OK",
            headers: pagination_headers(page, 250, page_count, 2),
            body: body.into(),
        }
    }

    fn trending_movies_body(start_trakt_id: u64, count: u64) -> String {
        let items = (0..count)
            .map(|offset| {
                let trakt_id = start_trakt_id + offset;
                format!(
                    r#"{{"watchers":1,"movie":{{"title":"Movie {trakt_id}","year":2026,"ids":{{"trakt":{trakt_id},"slug":"movie-{trakt_id}","imdb":null,"tmdb":{}}}}}}}"#,
                    100_000 + trakt_id
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{items}]")
    }

    fn pagination_headers(
        page: u64,
        limit: u64,
        page_count: u64,
        item_count: u64,
    ) -> Vec<(&'static str, String)> {
        vec![
            ("X-Pagination-Page", page.to_string()),
            ("X-Pagination-Limit", limit.to_string()),
            ("X-Pagination-Page-Count", page_count.to_string()),
            ("X-Pagination-Item-Count", item_count.to_string()),
        ]
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
        loop {
            let mut buffer = [0; 1024];
            let read = stream.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        Ok(String::from_utf8(bytes).expect("request should be valid UTF-8"))
    }

    fn parse_request(request: &str) -> RecordedRequest {
        let mut lines = request.split("\r\n");
        let request_line = lines.next().expect("request line should be present");
        let target = request_line
            .split_whitespace()
            .nth(1)
            .expect("request target should be present")
            .to_string();
        let headers = lines
            .take_while(|line| !line.is_empty())
            .map(|line| {
                let (name, value) = line.split_once(':').expect("header should contain colon");
                (name.to_ascii_lowercase(), value.trim().to_string())
            })
            .collect();
        RecordedRequest { target, headers }
    }

    fn assert_trending_request(request: &RecordedRequest, page: u64, limit: u64) {
        let (path, query) = request
            .target
            .split_once('?')
            .expect("request should include query parameters");
        assert_eq!(path, "/media/trending");
        let query: HashMap<_, _> = query
            .split('&')
            .map(|part| {
                part.split_once('=')
                    .expect("query pair should contain equals")
            })
            .collect();
        let expected_page = page.to_string();
        let expected_limit = limit.to_string();
        assert_eq!(query.get("page"), Some(&expected_page.as_str()));
        assert_eq!(query.get("limit"), Some(&expected_limit.as_str()));
        assert_eq!(request.headers[TRAKT_API_VERSION], "2");
        assert_eq!(request.headers["content-type"], "application/json");
        assert_eq!(
            request.headers["user-agent"],
            concat!(
                "emos-trakt-api/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/bxb100/emos)"
            )
        );
        assert!(request.headers.contains_key(TRAKT_API_KEY));
    }
}
