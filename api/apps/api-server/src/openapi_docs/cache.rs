use std::{io, ops::Deref, sync::Arc};

use serde::{Serialize, Serializer};
use serde_json::Value;
use storage_ephemeral::MokaReadThroughCache;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum ScopeKey {
    Operation(String),
    Category(String),
}

pub(super) type DocumentCache = MokaReadThroughCache<ScopeKey, Value>;

/// Immutable response body; cloning shares the JSON tree rather than copying it.
#[derive(Debug, Clone)]
pub struct SharedApiDocument(Arc<Value>);

impl From<Value> for SharedApiDocument {
    fn from(value: Value) -> Self {
        Self(Arc::new(value))
    }
}

impl From<Arc<Value>> for SharedApiDocument {
    fn from(value: Arc<Value>) -> Self {
        Self(value)
    }
}

impl Deref for SharedApiDocument {
    type Target = Value;

    fn deref(&self) -> &Value {
        &self.0
    }
}

impl Serialize for SharedApiDocument {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_ref().serialize(serializer)
    }
}

pub(super) fn document_weight(value: &Value) -> u64 {
    struct Counter(u64);
    impl io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len() as u64);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    // Value serialization to this infallible sink cannot encounter user IO errors.
    if serde_json::to_writer(&mut counter, value).is_err() {
        return u64::MAX;
    }
    counter.0
}

#[cfg(test)]
#[path = "../_tests/route_docs/docs_cache_tests.rs"]
mod tests;
