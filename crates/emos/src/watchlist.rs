use jiff::Timestamp;
use serde::Deserialize;
use serde::Serialize;

use crate::files::workspace_root;

#[derive(Serialize)]
struct DynamicWatchlist {
    name: String,
    cover: String,
    updated_at: String,
    videos: Vec<WatchlistVideo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct WatchlistVideo {
    pub(crate) tmdb_id: u64,
    pub(crate) tmdb_type: MediaType,
    pub(crate) title: String,
    pub(crate) sort: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum MediaType {
    Tv,
    Movie,
}

fn format_updated_at(timestamp: Timestamp) -> String {
    timestamp
        .to_zoned(jiff::tz::get!("Asia/Shanghai"))
        .strftime("%Y-%m-%d %H:%M:%S")
        .to_string()
}

pub(crate) fn write_dynamic_watchlist(
    filename: &str,
    name: &str,
    cover: &str,
    videos: Vec<WatchlistVideo>,
) -> anyhow::Result<()> {
    let data = DynamicWatchlist {
        name: name.to_string(),
        cover: cover.to_string(),
        updated_at: format_updated_at(Timestamp::now()),
        videos,
    };

    let path = workspace_root().join("data").join(filename);
    std::fs::write(path, serde_json::to_vec(&data)?)?;
    Ok(())
}

#[test]
fn dynamic_watchlist_preserves_json_format_and_shanghai_timezone() {
    let json = r#"
{
    "name": "热门电影",
    "cover": "https://emos.local/image.png",
    "updated_at": "2026-01-28 00:22:33",
    "videos": [
        {
            "tmdb_id": 1024,
            "tmdb_type": "tv",
            "title": "电视标题",
            "sort": 100
        },
        {
            "tmdb_id": 2048,
            "tmdb_type": "movie",
            "title": "电影标题",
            "sort": 100
        }
    ]
}
    "#;

    let watchlist = DynamicWatchlist {
        name: "热门电影".to_string(),
        cover: "https://emos.local/image.png".to_string(),
        updated_at: format_updated_at("2026-01-27T16:22:33Z".parse().unwrap()),
        videos: vec![
            WatchlistVideo {
                tmdb_id: 1024,
                tmdb_type: MediaType::Tv,
                title: "电视标题".to_string(),
                sort: 100,
            },
            WatchlistVideo {
                tmdb_id: 2048,
                tmdb_type: MediaType::Movie,
                title: "电影标题".to_string(),
                sort: 100,
            },
        ],
    };
    assert_eq!(
        serde_json::to_value(&watchlist).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}
