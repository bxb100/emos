use std::collections::HashMap;

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::Local;
use emos_http::RequestBuilderExt;
use hmac::Hmac;
use hmac::Mac;
use rand::prelude::*;
use rand::rng;
use reqwest::Client as HttpClient;
use reqwest::Url;
use serde::de::DeserializeOwned;
use sha1::Sha1;

// --- Constants ---
const API_SECRET_KEY: &str = "bf7dddc7c9cfe6f7";
const API_KEY: &str = "0dad551ec0f84ed02907ff5c42e8ec70";
const BASE_URL: &str = "https://frodo.douban.com/api/v2";

const USER_AGENTS: &[&str] = &[
    "api-client/1 com.douban.frodo/7.22.0.beta9(231) Android/23 product/Mate 40 vendor/HUAWEI model/Mate 40 brand/HUAWEI  rom/android  network/wifi  platform/AndroidPad",
    "api-client/1 com.douban.frodo/7.18.0(230) Android/22 product/MI 9 vendor/Xiaomi model/MI 9 brand/Android  rom/miui6  network/wifi  platform/mobile nd/1",
    "api-client/1 com.douban.frodo/7.1.0(205) Android/29 product/perseus vendor/Xiaomi model/Mi MIX 3  rom/miui6  network/wifi  platform/mobile nd/1",
    "api-client/1 com.douban.frodo/7.3.0(207) Android/22 product/MI 9 vendor/Xiaomi model/MI 9 brand/Android  rom/miui6  network/wifi  platform/mobile nd/1",
];

type HmacSha1 = Hmac<Sha1>;

#[derive(Default, Debug)]
pub struct Client {
    client: HttpClient,
}

impl Client {
    pub fn new() -> Self {
        Self {
            client: HttpClient::builder()
                .user_agent(USER_AGENTS[0])
                .cookie_store(true)
                .build()
                .unwrap_or_default(),
        }
    }

    /// 签名逻辑
    fn sign(url: &str, ts: &str, method: &str) -> Result<String> {
        let parsed_url = Url::parse(url)?;
        let url_path = parsed_url.path();

        // Python: parse.quote(url_path, safe='') -> 意味着把 '/' 也编码
        let encoded_path = urlencoding::encode(url_path);

        let raw_sign = format!("{}&{}&{}", method.to_uppercase(), encoded_path, ts);

        let mut mac = HmacSha1::new_from_slice(API_SECRET_KEY.as_bytes())?;
        mac.update(raw_sign.as_bytes());
        let result = mac.finalize().into_bytes();

        Ok(STANDARD.encode(result))
    }

    /// 核心 GET 请求处理
    pub(crate) async fn invoke<T: DeserializeOwned>(
        &self,
        path: &str,
        mut params: HashMap<String, String>,
    ) -> Result<T> {
        let req_url = format!("{BASE_URL}{path}");

        let ts = Local::now().format("%Y%m%d").to_string();

        // 构建签名参数
        let sig = Self::sign(&req_url, &ts, "GET")?;

        params.insert("apiKey".to_string(), API_KEY.to_string());
        params.insert("os_rom".to_string(), "android".to_string());
        params.insert("_ts".to_string(), ts);
        params.insert("_sig".to_string(), sig);

        let ua = USER_AGENTS.choose(&mut rng()).unwrap();

        let req = self
            .client
            .get(&req_url)
            .header("User-Agent", *ua)
            .query(&params);

        req.send_json().await
    }
}
