use anyhow::Result;
use emos_utils::fs::project_root;
use foyer::BlockEngineConfig;
use foyer::DeviceBuilder;
use foyer::FsDeviceBuilder;
use foyer::HybridCache;
use foyer::HybridCacheBuilder;
use foyer::StorageKey;
use foyer::StorageValue;

pub struct HyperCache<K: StorageKey, V: StorageValue> {
    cache: HybridCache<K, V>,
}

impl<K: StorageKey, V: StorageValue> HyperCache<K, V> {
    pub async fn new() -> Result<Self> {
        let data_path = project_root().join("data/foyer_cache");

        let device = FsDeviceBuilder::new(data_path)
            .with_capacity(256 * 1024 * 1024)
            .build()?;

        let hybrid: HybridCache<K, V> = HybridCacheBuilder::new()
            .memory(64 * 1024 * 1024)
            .storage()
            .with_engine_config(BlockEngineConfig::new(device))
            .build()
            .await?;

        Ok(Self { cache: hybrid })
    }
}

#[cfg(test)]
mod tests {
    use crate::HyperCache;

    #[tokio::test]
    async fn test_it_work() {
        let c = HyperCache::<String, String>::new().await.unwrap();
        c.cache.insert("key".to_string(), "value".to_string());
        let v = c.cache.get(&"key".to_string()).await.unwrap().unwrap();

        assert_eq!(v.value(), &"value".to_string());

        c.cache.clear().await.unwrap();

        c.cache.close().await.unwrap();
    }
}
