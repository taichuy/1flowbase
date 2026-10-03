use super::*;
use async_trait::async_trait;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Default)]
struct MemoryCache {
    values: Mutex<HashMap<String, (Value, Option<time::Duration>)>>,
    unavailable: bool,
}

#[async_trait]
impl CacheStore for MemoryCache {
    async fn get_json(&self, key: &str) -> Result<Option<Value>> {
        anyhow::ensure!(!self.unavailable, "cache offline");
        Ok(self
            .values
            .lock()
            .unwrap()
            .get(key)
            .map(|(value, _)| value.clone()))
    }
    async fn set_json(&self, key: &str, value: Value, ttl: Option<time::Duration>) -> Result<()> {
        anyhow::ensure!(!self.unavailable, "cache offline");
        self.values
            .lock()
            .unwrap()
            .insert(key.to_owned(), (value, ttl));
        Ok(())
    }
    async fn set_if_absent_json(
        &self,
        _: &str,
        _: Value,
        _: Option<time::Duration>,
    ) -> Result<bool> {
        unreachable!()
    }
    async fn delete(&self, _: &str) -> Result<()> {
        unreachable!()
    }
    async fn increment_counter(&self, _: &str, _: i64, _: Option<time::Duration>) -> Result<i64> {
        unreachable!()
    }
    async fn touch(&self, _: &str, _: time::Duration) -> Result<bool> {
        unreachable!()
    }
}

fn node() -> domain::FrontstageBlockNodeRecord {
    domain::FrontstageBlockNodeRecord {
        block_id: "block".into(),
        workspace_id: Uuid::now_v7(),
        page_id: Uuid::now_v7(),
        tab_id: Uuid::now_v7(),
        parent_block_id: None,
        rank: "a".into(),
        presentation: domain::FrontstageBlockPresentation::Inline,
        title: None,
        description: None,
        code_ref: "frontstage.block.block".into(),
        schema_version: 1,
        input_mapping: BTreeMap::new(),
        output_mapping: BTreeMap::new(),
        runtime_descriptor: json!({}),
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    }
}

#[tokio::test]
async fn digest_hit_skips_source_and_stores_only_a_digest_with_five_minute_ttl() {
    let store = Arc::new(MemoryCache::default());
    let cache = BlockCodeCache(store.clone());
    let node = node();
    let digest = "a".repeat(64);
    assert_eq!(
        cache
            .load(&node, async { Ok(Some(digest.clone())) })
            .await
            .unwrap(),
        Some(digest.clone())
    );
    assert_eq!(
        cache
            .load(&node, async {
                panic!("a digest hit must not read durable source")
            })
            .await
            .unwrap(),
        Some(digest.clone())
    );
    let entries = store.values.lock().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries.values().next().unwrap(),
        &(json!(digest), Some(TTL))
    );
}

#[tokio::test]
async fn late_old_fill_cannot_poison_post_commit_revision_or_other_scope() {
    let cache = BlockCodeCache(Arc::new(MemoryCache::default()));
    let old = node();
    let mut current = old.clone();
    current.updated_at += time::Duration::microseconds(1);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
    let delayed_cache = cache.clone();
    let old_node = old.clone();
    let delayed = tokio::spawn(async move {
        delayed_cache
            .load(&old_node, async {
                started_tx.send(()).unwrap();
                finish_rx.await.unwrap();
                Ok(Some("a".repeat(64)))
            })
            .await
            .unwrap()
    });
    started_rx.await.unwrap();
    assert_eq!(
        cache
            .load(&current, async { Ok(Some("b".repeat(64))) })
            .await
            .unwrap(),
        Some("b".repeat(64))
    );
    finish_tx.send(()).unwrap();
    assert_eq!(delayed.await.unwrap(), Some("a".repeat(64)));
    assert_eq!(
        cache
            .load(&current, async {
                panic!("old fill must not replace the new digest")
            })
            .await
            .unwrap(),
        Some("b".repeat(64))
    );
    current.workspace_id = Uuid::now_v7();
    assert_eq!(
        cache.load(&current, async { Ok(None) }).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn cache_failures_and_corrupt_values_fall_back_to_durable_metadata() {
    let cache = BlockCodeCache(Arc::new(MemoryCache {
        unavailable: true,
        ..Default::default()
    }));
    let record = node();
    assert_eq!(
        cache
            .load(&record, async { Ok(Some("a".repeat(64))) })
            .await
            .unwrap(),
        Some("a".repeat(64))
    );
    let store = Arc::new(MemoryCache::default());
    store
        .set_json(
            &BlockCodeCache::key(&record),
            json!({"source_code":"bad"}),
            Some(TTL),
        )
        .await
        .unwrap();
    let cache = BlockCodeCache(store);
    assert_eq!(
        cache
            .load(&record, async { Ok(Some("b".repeat(64))) })
            .await
            .unwrap(),
        Some("b".repeat(64))
    );
    assert!(cache
        .load(&node(), async { anyhow::bail!("durable offline") })
        .await
        .is_err());
}

#[test]
fn conditional_get_uses_weak_comparison_and_requires_quoted_tags() {
    let digest = "a".repeat(64);
    assert!(etag_matches(&format!("\"{digest}\""), &digest));
    assert!(etag_matches(&format!("\"other\", W/\"{digest}\""), &digest));
    assert!(etag_matches("*", &digest));
    assert!(!etag_matches(&digest, &digest));
    assert!(!etag_matches("\"old\"", &digest));
}

#[tokio::test]
async fn legacy_missing_digest_is_not_cached_or_replaced_with_a_synthetic_digest() {
    let store = Arc::new(MemoryCache::default());
    let cache = BlockCodeCache(store.clone());
    let record = node();
    for _ in 0..2 {
        assert_eq!(cache.load(&record, async { Ok(None) }).await.unwrap(), None);
        assert!(store.values.lock().unwrap().is_empty());
    }
}
