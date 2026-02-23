use chrono::DateTime;
use chrono_tz::Tz;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize)]
pub struct Dynamic {
    pub name: String,
    pub cover: String,
    #[serde(with = "my_date_format")]
    pub updated_at: DateTime<Tz>,
    pub videos: Vec<Media>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Media {
    pub tmdb_id: u64,
    pub tmdb_type: MediaType,
    pub title: String,
    pub sort: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Tv,
    Movie,
}

mod my_date_format {
    use chrono::DateTime;
    use chrono::NaiveDateTime;
    use chrono_tz::Asia::Shanghai;
    use chrono_tz::Tz;
    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serializer;
    use serde::{self};

    const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

    pub fn serialize<S>(date: &DateTime<Tz>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", date.format(FORMAT));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Tz>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let dt = NaiveDateTime::parse_from_str(&s, FORMAT).map_err(serde::de::Error::custom)?;
        Ok(dt.and_local_timezone(Shanghai).unwrap())
    }
}

#[test]
fn test_dynamic_struct_is_suit() {
    let json = r#"
{
    "name": "热门电影",
    "cover": "https://emos.local/image.png",
    "updated_at": "2026-01-28 11:22:33",
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

    let a = serde_json::from_str::<Dynamic>(json).unwrap();
    println!("{:#?}", a);

    let a = serde_json::to_string(&a).unwrap();
    println!("{}", a);
}
