use std::{sync::Arc, time::Duration};

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

pub(super) type DocumentCache = MokaReadThroughCache<ScopeKey, EncodedApiDocument>;

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
