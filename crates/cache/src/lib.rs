mod error;
mod kv;

use std::borrow::Borrow;
use std::fmt::Debug;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use delegate::delegate;
use serde::Serialize;
use serde::de::DeserializeOwned;
use utils::fs::project_root;

use crate::kv::TempCache;

pub struct Cache<K, V>
where
    K: Serialize + DeserializeOwned + Clone + Send + Sync + Eq + Ord + 'static,
    V: Serialize + DeserializeOwned + Clone + Send,
{
    inner: TempCache<V, K>,
}

impl<K, V> Cache<K, V>
where
    K: Serialize + DeserializeOwned + Clone + Send + Sync + Eq + Ord + 'static,
    V: Serialize + DeserializeOwned + Clone + Send,
{
    pub fn new() -> Result<Self> {
        let path = project_root().join("data/cache/simple_cache.bin");
        Self::with_path(path, Duration::from_hours(100))
    }

    /// Creates a cache backed by a caller-selected file.
    ///
    /// A dedicated path keeps values with different serialized types from sharing the
    /// default cache file. The supplied extension is replaced with `.mpbr` by the
    /// underlying store.
    pub fn with_path(path: impl AsRef<Path>, default_expiry: Duration) -> Result<Self> {
        let temp_cache = TempCache::<V, K>::new(path, default_expiry)?;

        Ok(Self { inner: temp_cache })
    }

    delegate! {
        to self.inner {
            pub async fn get<Q>(&self, key: &Q) -> Result<Option<V>> where K: Borrow<Q>, Q: Eq + Ord + Debug + ?Sized;
            pub async fn set(&self, key: impl Into<K>, value: impl Borrow<V>) -> Result<()>;
            pub async fn delete<Q>(&self, key: &Q) -> Result<()> where K: Borrow<Q>, Q: Eq + Ord + ?Sized;
            pub async fn save(&self) -> Result<()>;
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn set_save_and_reopen() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("oauth_token.bin");
        let cache = Cache::<String, String>::with_path(&path, Duration::ZERO).unwrap();
        cache.set("hello", "world".to_string()).await.unwrap();
        cache.save().await.unwrap();

        assert_eq!(cache.get("hello").await.unwrap().unwrap(), "world");

        drop(cache);

        let cache = Cache::<String, String>::with_path(&path, Duration::ZERO).unwrap();
        assert_eq!(cache.get("hello").await.unwrap().unwrap(), "world");
    }

    #[tokio::test]
    async fn delete_save_and_reopen() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("oauth_token.bin");
        let cache = Cache::<String, String>::with_path(&path, Duration::ZERO).unwrap();
        cache.set("hello", "world".to_string()).await.unwrap();
        cache.save().await.unwrap();
        cache.delete("hello").await.unwrap();
        cache.save().await.unwrap();
        drop(cache);

        let cache = Cache::<String, String>::with_path(&path, Duration::ZERO).unwrap();
        assert_eq!(cache.get("hello").await.unwrap(), None);
    }
}
