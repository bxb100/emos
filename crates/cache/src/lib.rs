mod blocking;
mod error;
mod kv;

use std::borrow::Borrow;
use std::fmt::Debug;
use std::time::Duration;

use anyhow::Result;
use delegate::delegate;
use emos_utils::fs::project_root;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::kv::TempCache;

pub struct Cache<K, V>
where
    K: Serialize + DeserializeOwned + Clone + Send + Eq + Ord,
    V: Serialize + DeserializeOwned + Clone + Send,
{
    inner: TempCache<V, K>,
}

impl<K, V> Cache<K, V>
where
    K: Serialize + DeserializeOwned + Clone + Send + Eq + Ord,
    V: Serialize + DeserializeOwned + Clone + Send,
{
    pub fn new() -> Result<Self> {
        let path = project_root().join("data/cache/simple_cache.bin");
        let temp_cache = TempCache::<V, K>::new(path, Duration::from_secs(30))?;

        Ok(Self { inner: temp_cache })
    }

    delegate! {
        to self.inner {
            pub fn get<Q>(&self, key: &Q) -> Result<Option<V>> where K: Borrow<Q>, Q: Eq + Ord + Debug + ?Sized;
            pub fn set(&self, key: impl Into<K>, value: impl Borrow<V>) -> Result<()>;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let cache = Cache::<String, String>::new().unwrap();
        cache.set("hello", "world".to_string()).unwrap();

        assert_eq!(cache.get("hello").unwrap().unwrap(), "world");

        drop(cache);

        let cache = Cache::<String, String>::new().unwrap();
        assert_eq!(cache.get("hello").unwrap().unwrap(), "world");
    }
}
