pub mod model;
use std::collections::HashMap;

use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::Local;
use hmac::Hmac;
use hmac::Mac;
use once_cell::sync::Lazy;
use rand::prelude::*;
use rand::rng;
use reqwest::Client;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha1::Sha1;
use utils::ReqwestExt;

use crate::model::interests::Interests;

// --- Constants ---
const API_SECRET_KEY: &str = "bf7dddc7c9cfe6f7";
const API_KEY: &str = "0dad551ec0f84ed02907ff5c42e8ec70";
const API_KEY2: &str = "0ab215a8b1977939201640fa14c66bab";
const BASE_URL: &str = "https://frodo.douban.com/api/v2";
const API_URL: &str = "https://api.douban.com/v2";

const USER_AGENTS: &[&str] = &[
    "api-client/1 com.douban.frodo/7.22.0.beta9(231) Android/23 product/Mate 40 vendor/HUAWEI model/Mate 40 brand/HUAWEI  rom/android  network/wifi  platform/AndroidPad",
    "api-client/1 com.douban.frodo/7.18.0(230) Android/22 product/MI 9 vendor/Xiaomi model/MI 9 brand/Android  rom/miui6  network/wifi  platform/mobile nd/1",
    "api-client/1 com.douban.frodo/7.1.0(205) Android/29 product/perseus vendor/Xiaomi model/Mi MIX 3  rom/miui6  network/wifi  platform/mobile nd/1",
    "api-client/1 com.douban.frodo/7.3.0(207) Android/22 product/MI 9 vendor/Xiaomi model/MI 9 brand/Android  rom/miui6  network/wifi platform/mobile nd/1",
];

// URL 映射，使用 Lazy Static 初始化
static URLS: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    // 搜索类
    m.insert("search", "/search/weixin");
    m.insert("search_agg", "/search");
    m.insert("search_subject", "/search/subjects");
    m.insert("imdbid", "/movie/imdb/%s");
    // 电影/电视
    // 电影探索
    // sort=U:综合排序 T:近期热度 S:高分优先 R:首播时间
    m.insert("movie_recommend", "/movie/recommend");
    // 电视剧探索
    m.insert("tv_recommend", "/tv/recommend");
    m.insert("movie_search", "/search/movie");
    m.insert("tv_search", "/search/movie");
    m.insert("book_search", "/search/book");
    m.insert("group_search", "/search/group");
    // 榜单
    m.extend([
        // 正在上映
        ("movie_showing", "/subject_collection/movie_showing/items"),
        // 热门电影
        ("movie_hot_gaia", "/subject_collection/movie_hot_gaia/items"),
        // 即将上映
        ("movie_soon", "/subject_collection/movie_soon/items"),
        // TOP250
        ("movie_top250", "/subject_collection/movie_top250/items"),
        // 高分经典科幻片榜
        ("movie_scifi", "/subject_collection/movie_scifi/items"),
        // 高分经典喜剧片榜
        ("movie_comedy", "/subject_collection/movie_comedy/items"),
        // 高分经典动作片榜
        ("movie_action", "/subject_collection/movie_action/items"),
        // 高分经典爱情片榜
        ("movie_love", "/subject_collection/movie_love/items"),
        // 热门剧集
        ("tv_hot", "/subject_collection/tv_hot/items"),
        // 国产剧
        ("tv_domestic", "/subject_collection/tv_domestic/items"),
        // 美剧
        ("tv_american", "/subject_collection/tv_american/items"),
        // 本剧
        ("tv_japanese", "/subject_collection/tv_japanese/items"),
        // 韩剧
        ("tv_korean", "/subject_collection/tv_korean/items"),
        // 动画
        ("tv_animation", "/subject_collection/tv_animation/items"),
        // 综艺
        (
            "tv_variety_show",
            "/subject_collection/tv_variety_show/items",
        ),
        // 华语口碑周榜
        (
            "tv_chinese_best_weekly",
            "/subject_collection/tv_chinese_best_weekly/items",
        ),
        // 全球口碑周榜
        (
            "tv_global_best_weekly",
            "/subject_collection/tv_global_best_weekly/items",
        ),
        // 执门综艺
        ("show_hot", "/subject_collection/show_hot/items"),
        // 国内综艺
        ("show_domestic", "/subject_collection/show_domestic/items"),
        // 国外综艺
        ("show_foreign", "/subject_collection/show_foreign/items"),
        (
            "book_bestseller",
            "/subject_collection/book_bestseller/items",
        ),
        ("book_top250", "/subject_collection/book_top250/items"),
        // 虚构类热门榜
        (
            "book_fiction_hot_weekly",
            "/subject_collection/book_fiction_hot_weekly/items",
        ),
        // 非虚构类热门
        (
            "book_nonfiction_hot_weekly",
            "/subject_collection/book_nonfiction_hot_weekly/items",
        ),
    ]);
    m
});

type HmacSha1 = Hmac<Sha1>;

#[derive(Default, Debug)]
pub struct DoubanApi {
    client: Client,
}

impl DoubanApi {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
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
    async fn invoke<T: DeserializeOwned>(
        &self,
        path: &str,
        mut params: HashMap<String, String>,
    ) -> Result<T> {
        let req_url = format!("{}{}", BASE_URL, path);

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

        req.execute().await
    }

    /// 核心 POST 请求处理
    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        mut params: HashMap<String, String>,
    ) -> Result<T> {
        let req_url = format!("{}{}", API_URL, path); // 注意 Base URL 不同

        params.insert("apikey".to_string(), API_KEY2.to_string());

        let ua = USER_AGENTS.choose(&mut rng()).unwrap();

        let resp = self
            .client
            .post(&req_url)
            .header("User-Agent", *ua)
            .form(&params) // application/x-www-form-urlencoded
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!("Post Request failed: {}", resp.status()))
        }
    }
}

macro_rules! impl_search_method {
    ($method_name:ident, $url_key:expr) => {
        pub async fn $method_name<T: DeserializeOwned>(
            &self,
            keyword: &str,
            start: Option<i32>,
            count: Option<i32>,
        ) -> Result<T> {
            let path = URLS.get($url_key).context("URL key not found")?;
            let params = HashMap::from([
                ("q".to_string(), keyword.to_string()),
                ("start".to_string(), start.unwrap_or(0).to_string()),
                ("count".to_string(), count.unwrap_or(100).to_string()),
            ]);
            self.invoke(path, params).await
        }
    };
}

macro_rules! impl_recommend_method {
    ($method_name:ident, $url_key:expr) => {
        pub async fn $method_name<T: DeserializeOwned>(
            &self,
            start: Option<i32>,
            count: Option<i32>,
        ) -> Result<T> {
            let path = URLS.get($url_key).context("URL key not found")?;
            let params = HashMap::from([
                ("start".to_string(), start.unwrap_or(0).to_string()),
                ("count".to_string(), count.unwrap_or(100).to_string()),
            ]);
            self.invoke(path, params).await
        }
    };
}

// --- 具体方法实现 ---

impl DoubanApi {
    // 1. 搜索类方法
    impl_search_method!(search, "search");
    impl_search_method!(movie_search, "movie_search");
    impl_search_method!(tv_search, "tv_search");
    impl_search_method!(book_search, "book_search");
    impl_search_method!(group_search, "group_search");

    // 2. 推荐/榜单类方法
    impl_recommend_method!(movie_showing, "movie_showing");
    impl_recommend_method!(movie_top250, "movie_top250");
    impl_recommend_method!(movie_scifi, "movie_scifi");
    impl_recommend_method!(tv_hot, "tv_hot");
    impl_recommend_method!(tv_american, "tv_american");
    impl_recommend_method!(tv_korean, "tv_korean");
    impl_recommend_method!(tv_japanese, "tv_japanese");
    impl_recommend_method!(tv_domestic, "tv_domestic");

    impl_recommend_method!(tv_global_best_weekly, "tv_global_best_weekly");

    // 3. 特殊方法：IMDB ID (POST)
    pub async fn imdbid(&self, imdbid: &str) -> Result<Value> {
        let path_tmpl = URLS.get("imdbid").context("URL key not found")?;
        // Rust 中简单的字符串格式化替换
        let path = path_tmpl.replace("%s", imdbid);

        // POST 请求参数，ts 已经在内部处理或者不需要
        let params = HashMap::new();
        self.post(&path, params).await
    }

    // 4. 详情类 (Detail)
    pub async fn movie_detail(&self, subject_id: &str) -> Result<Value> {
        // Python: "/movie/" + subject_id
        // 这里直接硬编码路径前缀，或者从 map 获取 "/movie/"
        let path = format!("/movie/{}", subject_id);
        self.invoke(&path, HashMap::new()).await
    }

    pub async fn tv_detail(&self, subject_id: &str) -> Result<Value> {
        let path = format!("/tv/{}", subject_id);
        self.invoke(&path, HashMap::new()).await
    }

    // 5. 探索 (Recommend with tags)
    pub async fn movie_recommend(
        &self,
        tags: &str,
        sort: &str,
        start: Option<i32>,
        count: Option<i32>,
    ) -> Result<Value> {
        let path = URLS.get("movie_recommend").context("URL key not found")?;
        let mut params = HashMap::new();
        params.insert("tags".to_string(), tags.to_string());
        params.insert("sort".to_string(), sort.to_string());
        if let Some(s) = start {
            params.insert("start".to_string(), s.to_string());
        }
        if let Some(c) = count {
            params.insert("count".to_string(), c.to_string());
        }

        self.invoke(path, params).await
    }

    // https://github.com/chengzhongxue/plugin-douban/blob/main/src/main/java/la/moony/douban/service/impl/DoubanServiceImpl.java
    pub async fn wish(
        &self,
        user_id: &str,
        start: Option<u32>,
        count: Option<u32>,
    ) -> anyhow::Result<Interests> {
        let path = format!("/user/{user_id}/interests");
        let params = HashMap::from([
            ("type".to_string(), "movie".to_string()),
            ("status".to_string(), "mark".to_string()),
            ("count".to_string(), count.unwrap_or(20).to_string()),
            ("start".to_string(), start.unwrap_or(0).to_string()),
        ]);
        self.invoke(&path, params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_movie_scifi() -> Result<()> {
        let api = DoubanApi::new();
        let res: Value = api.movie_scifi(Some(0), Some(5)).await?;
        println!("{}", serde_json::to_string(&res)?);
        Ok(())
    }
}
