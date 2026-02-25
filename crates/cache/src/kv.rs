#![allow(unused)]

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Seek;
use std::marker::PhantomData;
#[cfg(target_os = "linux")]
use std::os::linux::fs::MetadataExt;
#[cfg(target_os = "macos")]
use std::os::macos::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::RwLock;
use tokio::sync::RwLockReadGuard;
use tokio::sync::RwLockWriteGuard;
use serde::de::DeserializeOwned;
use tempfile::NamedTempFile;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::error::Error;

#[derive(Serialize, Deserialize, Clone)]
struct RawEntry {
    compressed: Box<[u8]>,
    /// 0 = old data, add default expiry
    expiry_timestamp: u32,
}

struct Inner<K> {
    data: Option<BTreeMap<K, RawEntry>>,
    writes: usize,
    next_autosave: usize,
    next_save_time: Instant,
    expected_size: AtomicU64,
}

pub struct TempCacheJson<
    T: Serialize + DeserializeOwned + Clone + Send,
    K: Serialize + DeserializeOwned + Clone + Send + Sync + Eq + Ord + 'static = Box<str>,
> {
    cache: TempCache<T, K>,
}

pub struct TempCache<
    T: Serialize + DeserializeOwned + Clone + Send,
    K: Serialize + DeserializeOwned + Clone + Send + Sync + Eq + Ord + 'static = Box<str>,
> {
    path: PathBuf,
    inner: Arc<RwLock<Inner<K>>>,
    _ty: PhantomData<T>,
    pub cache_only: bool,
    default_expiry: Duration,
}

impl<
    T: Serialize + DeserializeOwned + Clone + Send,
    K: Serialize + DeserializeOwned + Clone + Send + Sync + Eq + Ord + 'static,
> TempCacheJson<T, K>
{
    pub fn new(path: impl AsRef<Path>, default_expiry: Duration) -> Result<Self> {
        Ok(Self {
            cache: TempCache::new(path, default_expiry)?,
        })
    }
}

impl<
    T: Serialize + DeserializeOwned + Clone + Send,
    K: Serialize + DeserializeOwned + Clone + Send + Sync + Eq + Ord + 'static,
> TempCache<T, K>
{
    pub fn new(path: impl AsRef<Path>, default_expiry: Duration) -> Result<Self> {
        let base_path = path.as_ref();
        let path = base_path.with_extension("mpbr");

        let data = if path.exists() {
            None
        } else {
            Some(BTreeMap::default())
        };

        Ok(Self {
            path,
            inner: Arc::new(RwLock::new(Inner {
                data,
                writes: 0,
                next_autosave: 20,
                expected_size: AtomicU64::new(0),
                next_save_time: Instant::now() + Duration::from_secs(30),
            })),
            _ty: PhantomData,
            cache_only: false,
            default_expiry,
        })
    }

    #[inline]
    pub async fn set(&self, key: impl Into<K>, value: impl Borrow<T>) -> Result<()> {
        self.set_many([(key.into(), value)]).await
    }

    async fn lock_for_write(&self) -> Result<RwLockWriteGuard<'_, Inner<K>>> {
        if let Some(inner) = self.inner.try_write().ok()
            && inner.data.is_some()
        {
            return Ok(inner);
        }

        let mut inner = self.inner.write().await;
        if inner.data.is_none() {
            let (size, data) = self.load_data().await?;
            inner.expected_size = AtomicU64::new(size);
            inner.data = Some(data);
            inner.writes = 0;
            inner.next_autosave = 20;
            inner.next_save_time = Instant::now() + Duration::from_secs(30);
        }
        Ok(inner)
    }

    async fn lock_for_read(&self) -> Result<RwLockReadGuard<'_, Inner<K>>, Error> {
        if let Some(inner) = self.inner.try_read().ok()
            && inner.data.is_some()
        {
            return Ok(inner);
        }

        let inner = self.inner.read().await;
        if inner.data.is_some() {
            return Ok(inner);
        }
        drop(inner);

        let _ = self.lock_for_write().await?;
        Ok(self.inner.read().await)
    }

    async fn load_data(&self) -> Result<(u64, BTreeMap<K, RawEntry>)> {
        let path = self.path.clone();
        let default_expiry = self.default_expiry;

        tokio::task::spawn_blocking(move || {
            Self::load_data_blocking(&path, default_expiry)
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))?
    }

    fn load_data_blocking(path: &Path, default_expiry: Duration) -> Result<(u64, BTreeMap<K, RawEntry>)> {
        let f = File::open(path)?;
        let file_size = f.metadata()?.st_size();
        let mut f = BufReader::new(f);
        let data: BTreeMap<K, RawEntry> = match rmp_serde::from_read(&mut f) {
            Ok(data) => data,
            Err(e) => {
                warn!("Trying to upgrade the file {}: {e}", path.display());
                f.rewind()?;
                let data2: BTreeMap<K, Box<[u8]>> = rmp_serde::from_read(&mut f).map_err(|e| {
                    error!("File {} is broken: {}", path.display(), e);
                    e
                })?;
                let expiry_timestamp = if default_expiry == Duration::ZERO {
                    0
                } else {
                    Self::expiry(default_expiry)
                };
                let mut fuzzy_expiry = 0u32;
                data2
                    .into_iter()
                    .map(|(k, v)| {
                        fuzzy_expiry = fuzzy_expiry.wrapping_add(17291);
                        (
                            k,
                            RawEntry {
                                compressed: v,
                                expiry_timestamp: expiry_timestamp % fuzzy_expiry,
                            },
                        )
                    })
                    .collect()
            }
        };
        Ok((file_size, data))
    }

    #[inline]
    fn expiry(when: Duration) -> u32 {
        let timestamp_base = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_secs();
        ((timestamp_base + when.as_secs()) >> 2) as u32 // that's one way to solve timestamp overflow
    }

    fn serialize(value: &T) -> Result<Vec<u8>> {
        let mut e = brotli::CompressorWriter::new(Vec::new(), 1 << 16, 7, 18);
        rmp_serde::encode::write_named(&mut e, value)?;
        Ok(e.into_inner())
    }

    fn set_one(w: &mut Inner<K>, key: K, value: &T) -> Result<()> {
        let compr = Self::serialize(value)?;
        debug_assert!(Self::unbr(&compr).is_ok());
        match w.data.as_mut().unwrap().entry(key) {
            Entry::Vacant(e) => {
                e.insert(RawEntry {
                    compressed: compr.into_boxed_slice(),
                    expiry_timestamp: 0,
                });
            }
            Entry::Occupied(mut e) => {
                if &*e.get().compressed == compr.as_slice() {
                    return Ok(());
                }
                e.insert(RawEntry {
                    compressed: compr.into_boxed_slice(),
                    expiry_timestamp: 0,
                });
            }
        }
        w.writes += 1;
        Ok(())
    }

    pub async fn set_many(&self, many: impl IntoIterator<Item = (K, impl Borrow<T>)>) -> Result<()> {
        let mut w = self.lock_for_write().await?;

        for (key, value) in many {
            Self::set_one(&mut w, key, value.borrow())?;
        }

        self.save_if_needed(w).await
    }

    async fn save_if_needed(&self, mut w: RwLockWriteGuard<'_, Inner<K>>) -> Result<()> {
        if w.writes < w.next_autosave || w.data.is_none() {
            return Ok(());
        }
        let now = Instant::now();
        if w.next_save_time > now {
            return Ok(());
        }

        w.writes = 0;
        w.next_autosave *= 2;
        w.next_save_time = now + Duration::from_secs(20);
        drop(w); // unlock writes

        let inner = self.inner.clone();
        let path = self.path.clone();

        let success = tokio::task::spawn_blocking(move || {
            let d = inner.blocking_read();
            Self::save_blocking(&d, &path)
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))??;

        if !success {
            let mut w = self.inner.write().await;
            w.writes = 0;
            w.data = None;
        }
        Ok(())
    }

    pub async fn for_each(&self, mut cb: impl FnMut(&K, T)) -> Result<()> {
        let kw = self.lock_for_read().await?;
        kw.data.as_ref().unwrap().iter().try_for_each(|(k, v)| {
            let v = Self::unbr(&v.compressed)?;
            cb(k, v);
            Ok(())
        })
    }

    pub async fn par_for_each(&self, cb: impl Fn(&K, T) + Sync) -> Result<()>
    where
        K: Send + Sync,
        T: Send + Sync,
    {
        let kw = self.lock_for_read().await?;
        // Use spawn_blocking for cpu intensive task if needed, but rayon handles parallelism.
        // But rayon runs in thread pool. We are in async task.
        // We can just run it here.
        use rayon::prelude::*;
        kw.data.as_ref().unwrap().par_iter().try_for_each(|(k, v)| {
            let v = Self::unbr(&v.compressed)?;
            cb(k, v);
            Ok(())
        })
    }

    pub async fn delete<Q>(&self, key: &Q) -> Result<()>
    where
        K: Borrow<Q>,
        Q: Eq + Ord + ?Sized,
    {
        let mut d = self.lock_for_write().await?;
        if d.data.as_mut().unwrap().remove(key).is_some() {
            d.writes += 1;
        }
        Ok(())
    }

    pub async fn get<Q>(&self, key: &Q) -> Result<Option<T>>
    where
        K: Borrow<Q>,
        Q: Eq + Ord + std::fmt::Debug + ?Sized,
    {
        let kw = self.lock_for_read().await?;
        let data_ref = kw.data.as_ref().unwrap().get(key);

        match data_ref {
            Some(br) => {
                match Self::unbr(&br.compressed) {
                    Ok(val) => Ok(Some(val)),
                    Err(e) => {
                         error!("unbr of {:?} failed in {} {e}", key, self.path.display());
                         drop(kw);
                         let _ = self.delete(key).await;
                         Err(e.into())
                    }
                }
            }
            None => Ok(None)
        }
    }

    fn unbr(data: &[u8]) -> Result<T> {
        let unbr = brotli::Decompressor::new(data, (1 << 16).min(1024 + data.len() * 3));
        Ok(rmp_serde::decode::from_read(unbr)?)
    }

    pub async fn save(&self) -> Result<()> {
        let mut data = self.inner.clone().write_owned().await;
        if data.writes > 0 {
             self.expire_old(&mut data);

             let path = self.path.clone();
             let guard = data;

             tokio::task::spawn_blocking(move || {
                 Self::save_blocking(&guard, &path)?;
                 // We need to set writes=0 and data=None on the guard if save successful.
                 // But wait, save_blocking writes to disk.
                 // Original logic cleared memory if save successful.
                 // save_blocking returns bool (success).

                 // However, guard is moved here.
                 // We can mutate it here.
                 // But guard type is OwnedRwLockWriteGuard.
                 // We need mut access.
                 let mut g = guard;
                 g.writes = 0;
                 g.data = None;
                 Ok::<(), Error>(())
             })
             .await
             .map_err(|e| Error::Other(e.to_string()))??;
        } else {
             data.data = None;
        }
        Ok(())
    }

    fn expire_old(&self, d: &mut Inner<K>) {
        if self.default_expiry == Duration::ZERO {
            return;
        }
        if let Some(data) = d.data.as_mut() {
            let mut n = 0;
            let expiry_end = Self::expiry(Duration::ZERO);
            data.retain(|_, entry| {
                if entry.expiry_timestamp == 0 || entry.expiry_timestamp > expiry_end {
                    true
                } else {
                    n += 1;
                    false
                }
            });
            if n > 0 {
                info!("expired {n} entries from {}", self.path.display());
            }
        }
    }

    fn save_blocking(d: &Inner<K>, path: &Path) -> Result<bool> {
        if let Some(data) = d.data.as_ref() {
            debug!(
                "saving {} {} rows [{}/{}] next",
                path.display(),
                data.len(),
                d.writes,
                d.next_autosave
            );
            let tmp_path = NamedTempFile::new_in(path.parent().expect("tmp"))?;
            let mut file = BufWriter::with_capacity(1 << 18, File::create(&tmp_path)?);

            rmp_serde::encode::write(&mut file, data)?;
            // checked after encode to minimize race condition time window
            let expected_size = d.expected_size.load(SeqCst);
            let on_disk_size = std::fs::metadata(path)
                .ok()
                .map_or(0, |m| m.st_size());
            if on_disk_size != expected_size {
                error!(
                    "Data write race; discarding {} (expected {expected_size}; got {on_disk_size})",
                    path.display()
                );
                return Ok(false);
            }
            let new_size = file
                .into_inner()
                .map_err(|e| Error::Other(format!("{} @ {}", e.error(), path.display())))? // uuuuugh
                .metadata()?
                .st_size();
            tmp_path
                .persist(path.with_extension("mpbr"))
                .map_err(|e| e.error)?;
            d.expected_size.store(new_size, SeqCst);
        }
        Ok(true)
    }
}

impl<
    T: Serialize + DeserializeOwned + Clone + Send,
    K: Serialize + DeserializeOwned + Clone + Send + Sync + Eq + Ord + 'static,
> TempCacheJson<T, K>
{
    #[inline(always)]
    pub fn cache_only(&self) -> bool {
        self.cache.cache_only
    }

    #[inline(always)]
    pub fn set_cache_only(&mut self, b: bool) {
        self.cache.cache_only = b;
    }

    #[inline(always)]
    pub async fn get<Q>(&self, key: &Q) -> Result<Option<T>>
    where
        K: Borrow<Q>,
        Q: Eq + Ord + std::fmt::Debug + ?Sized,
    {
        self.cache.get(key).await
    }

    #[inline(always)]
    pub async fn set(&self, key: impl Into<K>, value: impl Borrow<T>) -> Result<()> {
        self.cache.set(key, value).await
    }

    #[inline(always)]
    pub async fn delete<Q>(&self, key: &Q) -> Result<()>
    where
        K: Borrow<Q>,
        Q: Eq + Ord + ?Sized,
    {
        self.cache.delete(key).await
    }

    #[inline(always)]
    pub async fn save(&self) -> Result<()> {
        self.cache.save().await
    }
}

impl<
    T: Serialize + DeserializeOwned + Clone + Send,
    K: Serialize + DeserializeOwned + Clone + Send + Sync + Eq + Ord + 'static,
> Drop for TempCache<T, K>
{
    fn drop(&mut self) {
        // Best effort save on drop
        if let Ok(mut d) = self.inner.clone().try_write_owned() {
            if d.writes > 0 {
                self.expire_old(&mut d);
                let path = self.path.clone();
                // Spawn a thread to save to avoid blocking async runtime or panicking
                // RwLockWriteGuard is Send, so we can move it to another thread.
                let _ = std::thread::spawn(move || {
                    if let Err(err) = Self::save_blocking(&d, &path) {
                        error!("Temp db save failed: {err}");
                    }
                })
                .join();
            }
        }
    }
}

#[tokio::test]
async fn kvtest() {
    let tmp: TempCache<(String, String)> =
        TempCache::new("/tmp/rmptest.bin", Duration::from_secs(1234)).unwrap();
    tmp.set("hello", ("world".to_string(), "etc".to_string()))
        .await
        .unwrap();
    let res = tmp.get("hello").await.unwrap().unwrap();
    drop(tmp);
    assert_eq!(res, ("world".to_string(), "etc".to_string()));

    let tmp2: TempCache<(String, String)> =
        TempCache::new("/tmp/rmptest.bin", Duration::from_secs(1234)).unwrap();
    let res2 = tmp2.get("hello").await.unwrap().unwrap();
    assert_eq!(res2, ("world".to_string(), "etc".to_string()));
}
