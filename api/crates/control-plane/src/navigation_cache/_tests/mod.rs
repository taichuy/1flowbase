use super::*;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Mutex};

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
        key: &str,
        value: Value,
        ttl: Option<time::Duration>,
    ) -> Result<bool> {
        anyhow::ensure!(!self.unavailable, "cache offline");
        let mut values = self.values.lock().unwrap();
        if values.contains_key(key) {
            return Ok(false);
        }
        values.insert(key.to_owned(), (value, ttl));
        Ok(true)
    }
    async fn delete(&self, key: &str) -> Result<()> {
        self.values.lock().unwrap().remove(key);
        Ok(())
    }
    async fn increment_counter(&self, _: &str, _: i64, _: Option<time::Duration>) -> Result<i64> {
        unreachable!()
    }
    async fn touch(&self, _: &str, _: time::Duration) -> Result<bool> {
        unreachable!()
    }
}

#[tokio::test]
async fn reuses_navigation_and_isolates_permissions_workspace_and_registry() {
    let store = Arc::new(MemoryCache::default());
    let cache = NavigationCache(store.clone());
    let mut actor = domain::ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root");
    let registry = json!(["boot-1", []]);
    assert_eq!(
        cache
            .console_navigation(&actor, registry.clone(), async { Ok(json!(["settings"])) })
            .await
            .unwrap(),
        json!(["settings"])
    );
    let hit: Value = cache
        .console_navigation(&actor, registry.clone(), async {
            panic!("cache hit must not read durable navigation")
        })
        .await
        .unwrap();
    assert_eq!(hit, json!(["settings"]));
    actor.is_root = false;
    actor.effective_display_role = "member".into();
    assert_eq!(
        cache
            .console_navigation(&actor, registry.clone(), async { Ok(json!([])) })
            .await
            .unwrap(),
        json!([])
    );
    actor.current_workspace_id = Uuid::now_v7();
    assert_eq!(
        cache
            .console_navigation(&actor, registry.clone(), async { Ok(json!(["other"])) })
            .await
            .unwrap(),
        json!(["other"])
    );
    actor.permissions.insert("new.permission".into());
    assert_eq!(
        cache
            .console_navigation(&actor, registry, async { Ok(json!(["new"])) })
            .await
            .unwrap(),
        json!(["new"])
    );
    assert_eq!(
        cache
            .console_navigation(&actor, json!(["boot-1", ["plugin-route"]]), async {
                Ok(json!([]))
            })
            .await
            .unwrap(),
        json!([])
    );
    assert!(store
        .values
        .lock()
        .unwrap()
        .values()
        .filter(|(_, ttl)| ttl.is_some())
        .all(|(_, ttl)| *ttl == Some(TTL)));
}

#[tokio::test]
async fn mutation_generation_prevents_late_read_from_restoring_stale_pages() {
    let cache = NavigationCache(Arc::new(MemoryCache::default()));
    let workspace = Uuid::now_v7();
    let domain = NavigationCacheDomain::FrontstagePages;
    let old: Value = cache
        .load(domain, workspace, "pages", async {
            // The write commits while an earlier durable read is still in flight.
            cache.invalidate(domain, workspace).await;
            Ok(json!(["deleted-page"]))
        })
        .await
        .unwrap();
    assert_eq!(old, json!(["deleted-page"]));
    let current = cache
        .load(domain, workspace, "pages", async { Ok(json!([])) })
        .await
        .unwrap();
    assert_eq!(current, json!([]));
    let hit: Value = cache
        .load(domain, workspace, "pages", async {
            panic!("current generation must be cached")
        })
        .await
        .unwrap();
    assert_eq!(hit, json!([]));
}

#[tokio::test]
async fn console_order_invalidation_expires_all_actor_projections() {
    let cache = NavigationCache(Arc::new(MemoryCache::default()));
    let actor = domain::ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root");
    cache
        .console_navigation(&actor, json!("boot"), async { Ok(json!(["a", "b"])) })
        .await
        .unwrap();
    cache
        .invalidate(
            NavigationCacheDomain::ConsoleRoutes,
            actor.current_workspace_id,
        )
        .await;
    assert_eq!(
        cache
            .console_navigation(&actor, json!("boot"), async { Ok(json!(["b", "a"])) })
            .await
            .unwrap(),
        json!(["b", "a"])
    );
}

#[tokio::test]
async fn unavailable_or_corrupt_cache_reads_durable_source_and_never_caches_source_errors() {
    let cache = NavigationCache(Arc::new(MemoryCache {
        unavailable: true,
        ..Default::default()
    }));
    let workspace = Uuid::now_v7();
    assert_eq!(
        cache
            .load(
                NavigationCacheDomain::FrontstagePages,
                workspace,
                "pages",
                async { Ok(vec![1]) }
            )
            .await
            .unwrap(),
        vec![1]
    );
    let store = Arc::new(MemoryCache::default());
    let cache = NavigationCache(store.clone());
    let key = cache
        .generation_key(NavigationCacheDomain::FrontstagePages, workspace)
        .await
        .unwrap();
    store
        .set_json(&format!("{key}:pages"), json!({"invalid": true}), Some(TTL))
        .await
        .unwrap();
    let value: Vec<i32> = cache
        .load(
            NavigationCacheDomain::FrontstagePages,
            workspace,
            "pages",
            async { Ok(vec![2]) },
        )
        .await
        .unwrap();
    assert_eq!(value, vec![2]);
    cache
        .invalidate(NavigationCacheDomain::FrontstagePages, workspace)
        .await;
    let failure: Result<Value> = cache
        .load(
            NavigationCacheDomain::FrontstagePages,
            workspace,
            "pages",
            async { anyhow::bail!("durable unavailable") },
        )
        .await;
    assert!(failure.is_err());
    assert_eq!(
        cache
            .load(
                NavigationCacheDomain::FrontstagePages,
                workspace,
                "pages",
                async { Ok(json!([3])) }
            )
            .await
            .unwrap(),
        json!([3])
    );
}
