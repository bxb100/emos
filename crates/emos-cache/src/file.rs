use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::marker::PhantomData;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tempfile::NamedTempFile;
use tokio::sync::RwLock;
use tokio::sync::RwLockReadGuard;
use tokio::sync::RwLockWriteGuard;
use tracing::debug;
use tracing::error;

/// On-disk entry shape. `expiry_timestamp` is no longer written with a meaningful
/// value, but it stays so previously written cache files remain readable.
#[derive(Serialize, Deserialize)]
struct EncodedEntry {
    compressed: Box<[u8]>,
    expiry_timestamp: u32,
}

struct CacheState {
    data: Option<BTreeMap<String, EncodedEntry>>,
    dirty: bool,
    expected_file_size: u64,
}

/// File-backed KV cache: values are msgpack+brotli encoded and lazily loaded from
/// disk on first access; `save` writes pending changes and keeps the loaded map.
/// Dropping the cache also attempts to persist pending changes.
pub struct FileCache<V: Serialize + DeserializeOwned> {
    path: PathBuf,
    inner: Arc<RwLock<CacheState>>,
    _value: PhantomData<V>,
}

impl<V: Serialize + DeserializeOwned> FileCache<V> {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().with_extension("mpbr");
        // an existing file is loaded lazily on first access
        let data = if path.exists() {
            None
        } else {
            Some(BTreeMap::default())
        };

        Self {
            path,
            inner: Arc::new(RwLock::new(CacheState {
                data,
                dirty: false,
                expected_file_size: 0,
            })),
            _value: PhantomData,
        }
    }

    #[inline]
    pub async fn set(&self, key: impl Into<String>, value: impl Borrow<V>) -> Result<()> {
        let mut w = self.lock_for_write().await?;
        Self::set_entry(&mut w, key.into(), value.borrow())
    }

    async fn lock_for_write(&self) -> Result<RwLockWriteGuard<'_, CacheState>> {
        let mut inner = self.inner.write().await;
        if inner.data.is_none() {
            let (size, data) = self.load_data().await?;
            inner.expected_file_size = size;
            inner.data = Some(data);
        }
        Ok(inner)
    }

    async fn lock_for_read(&self) -> Result<RwLockReadGuard<'_, CacheState>> {
        let inner = self.inner.read().await;
        if inner.data.is_some() {
            return Ok(inner);
        }
        drop(inner);

        Ok(RwLockWriteGuard::downgrade(self.lock_for_write().await?))
    }

    async fn load_data(&self) -> Result<(u64, BTreeMap<String, EncodedEntry>)> {
        let path = self.path.clone();

        tokio::task::spawn_blocking(move || Self::load_data_blocking(&path))
            .await
            .map_err(anyhow::Error::msg)?
    }

    fn load_data_blocking(path: &Path) -> Result<(u64, BTreeMap<String, EncodedEntry>)> {
        let f = File::open(path)?;
        let file_size = f.metadata()?.len();
        let mut f = BufReader::new(f);
        let data = rmp_serde::from_read(&mut f)?;
        Ok((file_size, data))
    }

    fn encode_value(value: &V) -> Result<Vec<u8>> {
        let mut encoder = brotli::CompressorWriter::new(Vec::new(), 1 << 16, 7, 18);
        rmp_serde::encode::write_named(&mut encoder, value)?;
        Ok(encoder.into_inner())
    }

    fn set_entry(w: &mut CacheState, key: String, value: &V) -> Result<()> {
        let compressed = Self::encode_value(value)?;
        match w.data.as_mut().unwrap().entry(key) {
            Entry::Occupied(e) if &*e.get().compressed == compressed.as_slice() => {}
            Entry::Vacant(e) => {
                e.insert(EncodedEntry {
                    compressed: compressed.into_boxed_slice(),
                    expiry_timestamp: 0,
                });
                w.dirty = true;
            }
            Entry::Occupied(mut e) => {
                e.insert(EncodedEntry {
                    compressed: compressed.into_boxed_slice(),
                    expiry_timestamp: 0,
                });
                w.dirty = true;
            }
        }
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut d = self.lock_for_write().await?;
        if d.data.as_mut().unwrap().remove(key).is_some() {
            d.dirty = true;
        }
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<V>> {
        let kw = self.lock_for_read().await?;
        let data_ref = kw.data.as_ref().unwrap().get(key);

        match data_ref {
            Some(br) => match Self::decode_value(&br.compressed) {
                Ok(val) => Ok(Some(val)),
                Err(e) => {
                    error!(
                        "Decoding cache value for {:?} failed in {} {e}",
                        key,
                        self.path.display()
                    );
                    drop(kw);
                    let _ = self.delete(key).await;
                    Err(e)
                }
            },
            None => Ok(None),
        }
    }

    fn decode_value(data: &[u8]) -> Result<V> {
        let decoder = brotli::Decompressor::new(data, (1 << 16).min(1024 + data.len() * 3));
        Ok(rmp_serde::decode::from_read(decoder)?)
    }

    pub async fn save(&self) -> Result<()> {
        let mut data = self.inner.clone().write_owned().await;
        if !data.dirty {
            return Ok(());
        }

        let path = self.path.clone();
        tokio::task::spawn_blocking(move || Self::save_blocking(&mut data, &path)).await?
    }

    fn save_blocking(d: &mut CacheState, path: &Path) -> Result<()> {
        let data = d.data.as_ref().unwrap();
        debug!("saving {} {} rows", path.display(), data.len());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = NamedTempFile::new_in(path.parent().expect("tmp"))?;
        let mut file = BufWriter::with_capacity(1 << 18, File::create(&tmp_path)?);

        rmp_serde::encode::write(&mut file, data)?;
        // Checked after encode to minimize the race window. File length alone
        // does not detect all external writes or make this check atomic.
        let on_disk_size = std::fs::metadata(path).ok().map_or(0, |m| m.len());
        anyhow::ensure!(
            on_disk_size == d.expected_file_size,
            "Cache file changed while saving {} (expected {} bytes; got {on_disk_size})",
            path.display(),
            d.expected_file_size
        );
        let new_size = file
            .into_inner()
            .map_err(|e| e.into_error())?
            .metadata()?
            .len();
        tmp_path.persist(path).map_err(|e| e.error)?;
        d.expected_file_size = new_size;
        d.dirty = false;
        Ok(())
    }
}

impl<V: Serialize + DeserializeOwned> Drop for FileCache<V> {
    fn drop(&mut self) {
        // Best effort save on drop
        if let Ok(mut data) = self.inner.try_write()
            && data.dirty
            && let Err(err) = Self::save_blocking(&mut data, &self.path)
        {
            error!("Cache file save failed: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::Poll;

    use super::*;

    #[tokio::test]
    async fn empty_save_leaves_cache_usable() {
        let directory = tempfile::tempdir().unwrap();
        let cache = FileCache::<String>::new(directory.path().join("cache.bin"));

        cache.save().await.unwrap();
        assert_eq!(cache.get("missing").await.unwrap(), None);
        cache.set("token", "value".to_string()).await.unwrap();
        assert_eq!(cache.get("token").await.unwrap().as_deref(), Some("value"));
    }

    #[tokio::test]
    async fn save_then_update_persists_latest_value() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.bin");
        let cache = FileCache::<String>::new(&path);
        cache.set("token", "first".to_string()).await.unwrap();
        cache.save().await.unwrap();
        cache
            .set("token", "updated value".to_string())
            .await
            .unwrap();
        cache.save().await.unwrap();
        assert_eq!(
            cache.get("token").await.unwrap().as_deref(),
            Some("updated value")
        );

        let reopened = FileCache::<String>::new(&path);
        assert_eq!(
            reopened.get("token").await.unwrap().as_deref(),
            Some("updated value")
        );
    }

    #[tokio::test]
    async fn first_read_can_complete_while_save_is_waiting() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.bin");
        let cache = FileCache::<String>::new(&path);
        cache.set("token", "value".to_string()).await.unwrap();
        cache.save().await.unwrap();
        drop(cache);
        let cache = FileCache::<String>::new(&path);

        // Queue the initial load before save so save is waiting when the load
        // finishes. This exercises the transition to reading the loaded map.
        let guard = cache.inner.read().await;
        let mut read = Box::pin(cache.get("token"));
        let mut save = Box::pin(cache.save());
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(save.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        drop(guard);

        let (value, saved) = tokio::join!(read, save);
        saved.unwrap();
        assert_eq!(value.unwrap().as_deref(), Some("value"));
    }

    #[tokio::test]
    async fn set_save_and_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oauth_token.bin");
        let cache = FileCache::<String>::new(&path);
        cache.set("hello", "world".to_string()).await.unwrap();
        cache.save().await.unwrap();

        assert_eq!(cache.get("hello").await.unwrap().unwrap(), "world");

        drop(cache);

        let cache = FileCache::<String>::new(&path);
        assert_eq!(cache.get("hello").await.unwrap().unwrap(), "world");
    }

    #[tokio::test]
    async fn delete_save_and_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oauth_token.bin");
        let cache = FileCache::<String>::new(&path);
        cache.set("hello", "world".to_string()).await.unwrap();
        cache.save().await.unwrap();
        cache.delete("hello").await.unwrap();
        cache.save().await.unwrap();
        drop(cache);

        let cache = FileCache::<String>::new(&path);
        assert_eq!(cache.get("hello").await.unwrap(), None);
    }

    #[tokio::test]
    async fn reopen_without_explicit_save() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.bin");
        let cache = FileCache::<(String, String)>::new(&path);
        cache
            .set("hello", ("world".to_string(), "etc".to_string()))
            .await
            .unwrap();
        let res = cache.get("hello").await.unwrap().unwrap();
        drop(cache);
        assert_eq!(res, ("world".to_string(), "etc".to_string()));

        let cache = FileCache::<(String, String)>::new(&path);
        let res2 = cache.get("hello").await.unwrap().unwrap();
        assert_eq!(res2, ("world".to_string(), "etc".to_string()));
    }

    #[tokio::test]
    async fn explicit_save_reports_an_external_write_race() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.bin");
        let first = FileCache::<String>::new(&path);
        let second = FileCache::<String>::new(&path);

        first.set("token", "first".to_string()).await.unwrap();
        second.set("token", "second".to_string()).await.unwrap();
        first.save().await.unwrap();

        let error = second.save().await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Cache file changed while saving")
        );

        // Avoid retrying the deliberately stale write in Drop.
        second.inner.write().await.dirty = false;
        let reopened = FileCache::<String>::new(&path);
        assert_eq!(
            reopened.get("token").await.unwrap().as_deref(),
            Some("first")
        );
    }
}
