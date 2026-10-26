use std::{
    future::Future,
    sync::{Arc, OnceLock},
    time::Duration,
};

use anyhow::Context;
use axum::{
    body::Bytes,
    http::header,
    response::{IntoResponse, Response},
};
use serde_json::Value;
use storage_ephemeral::{MokaReadThroughCache, ReadThroughCachePolicy};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum ScopeKey {
    Operation(String),
    Category(String),
}

#[derive(Debug, Clone)]
pub(super) struct DocumentCache(Arc<DeferredCache>);

#[derive(Debug)]
struct DeferredCache {
    policy: ReadThroughCachePolicy,
    runtime: tokio::runtime::Handle,
    active: OnceLock<MokaReadThroughCache<ScopeKey, EncodedApiDocument>>,
}

impl DocumentCache {
    pub(super) fn new(policy: ReadThroughCachePolicy) -> anyhow::Result<Self> {
        Ok(Self(Arc::new(DeferredCache {
            policy,
            runtime: tokio::runtime::Handle::try_current()
                .context("read-through cache requires a Tokio runtime")?,
            active: OnceLock::new(),
        })))
    }

    pub(super) async fn get_or_load(
        &self,
        key: ScopeKey,
        load: impl Future<Output = EncodedApiDocument> + Send,
    ) -> Arc<EncodedApiDocument> {
        let cache = self.0.active.get_or_init(|| {
            // Keep maintenance on the constructing runtime, even if a different runtime
            // polls the first request. The enter guard ends before any await.
            let _runtime = self.0.runtime.enter();
            MokaReadThroughCache::new(self.0.policy, document_weight)
                .expect("captured Tokio runtime and validated document cache policy")
        });
        cache.get_or_load(key, load).await
    }

    #[cfg(test)]
    async fn run_pending_tasks(&self) {
        if let Some(cache) = self.0.active.get() {
            cache.run_pending_tasks().await;
        }
    }

    #[cfg(test)]
    fn entry_count(&self) -> u64 {
        self.0.active.get().map_or(0, |cache| cache.entry_count())
    }
}

pub(super) fn default_policy() -> anyhow::Result<ReadThroughCachePolicy> {
    // Disposable wire payload budget, never a document or request capacity limit.
    // 1MiB admits the largest current category (629,203B) with room for other scopes.
    ReadThroughCachePolicy::new(1024 * 1024, Duration::from_secs(60))
}

#[derive(Debug)]
pub(super) struct EncodedApiDocument {
    body: Bytes,
    managed_projection: Value,
}

impl From<Value> for EncodedApiDocument {
    fn from(value: Value) -> Self {
        let managed_projection = super::managed_projection::DocsOutputProjection::OpenApi(&value)
            .project_for_managed_hook()
            .expect("OpenAPI JSON summary is always representable");
        // A Value has only valid JSON keys/numbers, and writing to a Vec has no IO failure.
        let body = serde_json::to_vec(&value)
            .expect("serializing a JSON Value into memory is infallible")
            .into();
        Self {
            body,
            managed_projection,
        }
    }
}

/// Immutable, already encoded HTTP body; cloning and responding share its byte allocation.
#[derive(Debug, Clone)]
pub struct SharedApiDocument(Arc<EncodedApiDocument>);

impl From<Value> for SharedApiDocument {
    fn from(value: Value) -> Self {
        Arc::new(EncodedApiDocument::from(value)).into()
    }
}

impl From<Arc<EncodedApiDocument>> for SharedApiDocument {
    fn from(value: Arc<EncodedApiDocument>) -> Self {
        Self(value)
    }
}

impl SharedApiDocument {
    pub(crate) fn managed_projection(&self) -> Value {
        self.0.managed_projection.clone()
    }
}

impl IntoResponse for SharedApiDocument {
    fn into_response(self) -> Response {
        (
            [(header::CONTENT_TYPE, "application/json")],
            self.0.body.clone(),
        )
            .into_response()
    }
}

pub(super) fn document_weight(value: &EncodedApiDocument) -> u64 {
    value.body.len() as u64
}

#[cfg(test)]
#[path = "../_tests/route_docs/docs_cache_tests.rs"]
mod tests;
