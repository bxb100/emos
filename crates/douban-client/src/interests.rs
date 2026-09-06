use std::collections::HashMap;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::Client;
use crate::subject::Subject;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterestPage {
    pub count: i64,
    pub interests: Vec<Interest>,
    pub start: i64,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interest {
    pub id: i64,
    pub status: String,
    pub subject: Subject,
}

impl Client {
    // https://github.com/chengzhongxue/plugin-douban/blob/main/src/main/java/la/moony/douban/service/impl/DoubanServiceImpl.java
    pub async fn wish(
        &self,
        user_id: &str,
        start: Option<u32>,
        count: Option<u32>,
    ) -> Result<InterestPage> {
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
    use crate::Client;
    #[tokio::test]
    async fn test_interests() {
        let api = Client::new();
        let c = api.wish("1321428", Some(0), Some(50)).await.unwrap();

        println!("{:?}", c);
    }
}
