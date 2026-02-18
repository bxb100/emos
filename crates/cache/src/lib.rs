use std::fmt::Debug;
use std::hash::BuildHasherDefault;

use anyhow::Result;
use emos_utils::fs::project_root;
use foyer::BlockEngineConfig;
use foyer::DeviceBuilder;
use foyer::FsDeviceBuilder;
use foyer::HybridCache;
use foyer::HybridCacheBuilder;
use foyer::HybridCachePolicy;
use foyer::PsyncIoEngineConfig;
use foyer::RecoverMode;
use foyer::StorageKey;
use foyer::StorageValue;

pub struct HyperCache<K: StorageKey, V: StorageValue> {
    pub cache: HybridCache<K, V>,
}

impl<K: StorageKey + Debug, V: StorageValue + Debug> HyperCache<K, V> {
    pub async fn new() -> Result<Self> {
        let data_path = project_root().join("data/cache");

        let device = FsDeviceBuilder::new(data_path)
            .with_capacity(16 * 1024 * 1024)
            .build()?;

        let hybrid: HybridCache<K, V> = HybridCacheBuilder::new()
            .with_policy(HybridCachePolicy::WriteOnInsertion)
            .with_flush_on_close(true)
            .memory(1024)
            .with_shards(4)
            .with_hash_builder(BuildHasherDefault::default())
            .storage()
            .with_io_engine_config(PsyncIoEngineConfig::new())
            .with_engine_config(BlockEngineConfig::new(device))
            .with_recover_mode(RecoverMode::Quiet)
            .with_compression(foyer::Compression::Lz4)
            .build()
            .await?;

        Ok(Self { cache: hybrid })
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde::Serialize;

    use crate::HyperCache;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
    struct TestSerde {
        id: u64,
        name: String,
    }

    #[tokio::test]
    async fn test_it_work() -> anyhow::Result<()> {
        let c = HyperCache::<String, String>::new().await?;
        c.cache.insert("key".to_string(), "value".to_string());
        let v = c.cache.get("key").await?.unwrap();

        assert_eq!(v.value(), "value");

        c.cache.close().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_it_serde() -> anyhow::Result<()> {
        let c = HyperCache::<TestSerde, u64>::new().await?;
        let key = TestSerde {
            id: 1,
            name: "test".to_string(),
        };

        c.cache.insert(key.clone(), 1);
        let v = c.cache.get(&key).await?.unwrap();

        assert_eq!(v.value(), &1);
        Ok(())
    }
}
