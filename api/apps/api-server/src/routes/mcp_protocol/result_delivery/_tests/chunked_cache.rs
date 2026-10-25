use super::*;
use std::sync::Mutex;

struct RecordingCache {
    inner: storage_ephemeral::MokaCacheStore,
    fail_key: Option<String>,
    writes: Mutex<Vec<(String, usize, Option<Duration>)>>,
}

impl RecordingCache {
    fn new(fail_key: Option<String>) -> Self {
        Self {
            inner: storage_ephemeral::MokaCacheStore::new("mcp-chunk-protocol", 128),
            fail_key,
            writes: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl CacheStore for RecordingCache {
    async fn get_json(&self, key: &str) -> anyhow::Result<Option<Value>> {
        self.inner.get_json(key).await
    }

    async fn set_json(&self, key: &str, value: Value, ttl: Option<Duration>) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(&value)?.len();
        assert!(bytes <= DETAIL_CHUNK_MAX_BYTES);
        assert_eq!(ttl, Some(Duration::seconds(600)));
        if let Some((manifest_key, _)) = key.rsplit_once(":chunk:") {
            assert!(self.inner.get_json(manifest_key).await?.is_none());
        } else if value["format"] == "chunked" {
            let count = value["chunk_count"].as_u64().unwrap();
            for index in 0..count {
                assert!(self
                    .inner
                    .get_json(&format!("{key}:chunk:{index}"))
                    .await?
                    .is_some());
            }
        }
        if self.fail_key.as_deref() == Some(key) {
            anyhow::bail!("injected cache write failure");
        }
        self.inner.set_json(key, value, ttl).await?;
        self.writes
            .lock()
            .unwrap()
            .push((key.to_owned(), bytes, ttl));
        Ok(())
    }

    async fn set_if_absent_json(
        &self,
        key: &str,
        value: Value,
        ttl: Option<Duration>,
    ) -> anyhow::Result<bool> {
        self.inner.set_if_absent_json(key, value, ttl).await
    }

    async fn increment_counter(
        &self,
        key: &str,
        amount: i64,
        ttl: Option<Duration>,
    ) -> anyhow::Result<i64> {
        self.inner.increment_counter(key, amount, ttl).await
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.inner.delete(key).await
    }

    async fn touch(&self, key: &str, ttl: Duration) -> anyhow::Result<bool> {
        self.inner.touch(key, ttl).await
    }
}

fn dependencies(cache: Arc<RecordingCache>) -> McpResultDeliveryDependencies {
    // These tests call only the cache owner, never a durable receipt method.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:17601/postgres")
        .unwrap();
    McpResultDeliveryDependencies::new(
        storage_durable_postgres::PgControlPlaneStore::new(pool),
        cache,
    )
}

fn mixed_detail() -> Value {
    let unit = "界😀\u{0000}\n\r\t\"\\/é";
    // Bounded serialized JSON above one cache value, with several chunk writes.
    json!({"nested": [{"text": unit.repeat(DETAIL_CHUNK_MAX_BYTES / unit.len())}], "tail": [null, true, 42]})
}

#[tokio::test]
async fn chunked_cache_roundtrip_preserves_full_json_and_rejects_missing_or_corrupt_chunks() {
    let cache = Arc::new(RecordingCache::new(None));
    let dependencies = dependencies(cache.clone());
    let workspace_id = Uuid::now_v7();
    let result_ref = Uuid::now_v7();
    let detail = mixed_detail();
    let defaults = json!({"max_inline_chars": 1600, "response_fields": ["/nested/0/text"]});
    assert!(serialized(&detail).len() > DETAIL_CHUNK_MAX_BYTES);
    assert!(matches!(
        cache_detail(
            &dependencies,
            workspace_id,
            result_ref,
            &detail,
            Some(&defaults)
        )
        .await,
        DetailCacheStatus::Available
    ));
    let manifest_key = cache_key(workspace_id, result_ref);
    let manifest = cache.get_json(&manifest_key).await.unwrap().unwrap();
    assert_eq!(manifest["format"], "chunked");
    assert_eq!(manifest["return_defaults"], defaults);
    let count = manifest["chunk_count"].as_u64().unwrap() as usize;
    assert!(count > 1);
    let mut stored_json = String::new();
    for index in 0..count {
        let value = cache
            .get_json(&chunk_cache_key(workspace_id, result_ref, index))
            .await
            .unwrap()
            .unwrap();
        assert!(serialized(&value).len() <= DETAIL_CHUNK_MAX_BYTES);
        stored_json.push_str(value.as_str().unwrap());
    }
    assert_eq!(stored_json, serialized(&detail));
    {
        let writes = cache.writes.lock().unwrap();
        assert_eq!(writes.len(), count + 1);
        for (index, (key, _, ttl)) in writes[..count].iter().enumerate() {
            assert_eq!(*key, chunk_cache_key(workspace_id, result_ref, index));
            assert_eq!(*ttl, Some(Duration::seconds(600)));
        }
        assert_eq!(writes.last().unwrap().0, manifest_key);
    }
    assert_eq!(
        resolve_cached_detail(&dependencies, workspace_id, result_ref, manifest.clone()).await,
        Some(detail.clone())
    );

    let first_key = chunk_cache_key(workspace_id, result_ref, 0);
    let first = cache.get_json(&first_key).await.unwrap().unwrap();
    cache.delete(&first_key).await.unwrap();
    assert!(
        resolve_cached_detail(&dependencies, workspace_id, result_ref, manifest.clone())
            .await
            .is_none()
    );
    // A string with invalid JSON and a non-string are distinct corruption paths.
    for corrupt in [json!("!"), json!({"invalid": "chunk"})] {
        cache
            .inner
            .set_json(&first_key, corrupt, None)
            .await
            .unwrap();
        assert!(
            resolve_cached_detail(&dependencies, workspace_id, result_ref, manifest.clone())
                .await
                .is_none()
        );
    }
    cache.inner.set_json(&first_key, first, None).await.unwrap();
    assert_eq!(
        resolve_cached_detail(&dependencies, workspace_id, result_ref, manifest).await,
        Some(detail)
    );
}

#[tokio::test]
async fn failed_chunk_or_manifest_write_never_publishes_partial_detail() {
    let detail = mixed_detail();
    for fail_manifest in [false, true] {
        let workspace_id = Uuid::now_v7();
        let result_ref = Uuid::now_v7();
        let manifest_key = cache_key(workspace_id, result_ref);
        let fail_key = if fail_manifest {
            manifest_key.clone()
        } else {
            chunk_cache_key(workspace_id, result_ref, 1)
        };
        let cache = Arc::new(RecordingCache::new(Some(fail_key)));
        let dependencies = dependencies(cache.clone());
        assert!(matches!(
            cache_detail(&dependencies, workspace_id, result_ref, &detail, None).await,
            DetailCacheStatus::Unavailable("cache_store_unavailable")
        ));
        assert!(cache.get_json(&manifest_key).await.unwrap().is_none());
        assert!(cache
            .get_json(&chunk_cache_key(workspace_id, result_ref, 0))
            .await
            .unwrap()
            .is_some());
        if !fail_manifest {
            assert!(cache
                .get_json(&chunk_cache_key(workspace_id, result_ref, 1))
                .await
                .unwrap()
                .is_none());
        }
    }
}
