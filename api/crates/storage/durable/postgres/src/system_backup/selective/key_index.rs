//! Disk-backed FK membership. Two archive passes, fixed 256 buckets, bounded read cache.
use anyhow::Result;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

const CACHE_LIMIT: usize = 16 * 1024 * 1024;
pub(super) struct KeyIndex {
    path: PathBuf,
    writers: BTreeMap<u8, File>,
    cache: BTreeMap<u8, Vec<[u8; 32]>>,
    cache_bytes: usize,
}
impl KeyIndex {
    pub fn new() -> Result<Self> {
        let path =
            std::env::temp_dir().join(format!("1flowbase-settings-index-{}", uuid::Uuid::now_v7()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path)?;
        Ok(Self {
            path,
            writers: BTreeMap::new(),
            cache: BTreeMap::new(),
            cache_bytes: 0,
        })
    }
    fn digest(table: &str, columns: &[String], values: &[&Value]) -> Result<[u8; 32]> {
        Ok(Sha256::digest(serde_json::to_vec(&(table, columns, values))?).into())
    }
    pub fn insert(&mut self, table: &str, columns: &[String], row: &Value) -> Result<()> {
        let null = Value::Null;
        let values = columns
            .iter()
            .map(|c| row.get(c).unwrap_or(&null))
            .collect::<Vec<_>>();
        if values.iter().any(|v| v.is_null()) {
            return Ok(());
        }
        let digest = Self::digest(table, columns, &values)?;
        let bucket = digest[0];
        if !self.writers.contains_key(&bucket) {
            self.writers
                .insert(bucket, File::create(self.path.join(bucket.to_string()))?);
        }
        self.writers
            .get_mut(&bucket)
            .expect("opened bucket")
            .write_all(&digest)?;
        Ok(())
    }
    pub fn finish(&mut self) -> Result<()> {
        for writer in self.writers.values_mut() {
            writer.flush()?
        }
        self.writers.clear();
        Ok(())
    }
    pub fn contains(
        &mut self,
        table: &str,
        parent_columns: &[String],
        child_columns: &[String],
        row: &Value,
    ) -> Result<bool> {
        let null = Value::Null;
        let values = child_columns
            .iter()
            .map(|c| row.get(c).unwrap_or(&null))
            .collect::<Vec<_>>();
        let digest = Self::digest(table, parent_columns, &values)?;
        let bucket = digest[0];
        if let Some(keys) = self.cache.get(&bucket) {
            return Ok(keys.binary_search(&digest).is_ok());
        }
        let path = self.path.join(bucket.to_string());
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        let bytes = file.metadata()?.len();
        if bytes > CACHE_LIMIT as u64 {
            let mut key = [0u8; 32];
            loop {
                match file.read_exact(&mut key) {
                    Ok(()) if key == digest => return Ok(true),
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
                    Err(e) => return Err(e.into()),
                }
            }
        }
        if self.cache_bytes + bytes as usize > CACHE_LIMIT {
            self.cache.clear();
            self.cache_bytes = 0;
        }
        let mut raw = Vec::with_capacity(bytes as usize);
        file.read_to_end(&mut raw)?;
        let mut keys = raw.as_chunks::<32>().0.to_vec();
        keys.sort_unstable();
        keys.dedup();
        let found = keys.binary_search(&digest).is_ok();
        self.cache_bytes += keys.len() * 32;
        self.cache.insert(bucket, keys);
        Ok(found)
    }
}
impl Drop for KeyIndex {
    fn drop(&mut self) {
        self.writers.clear();
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
