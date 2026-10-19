//! Trace write-only ownership. Domain reads and stored/wire JSON remain ordinary Value.
use serde::{ser::SerializeMap, Serialize, Serializer};
use serde_json::Value;
use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt,
    ops::{Index, IndexMut},
    sync::Arc,
};

#[derive(Debug, Clone)]
pub enum TraceProjectionWritePayload {
    Plain(Value),
    SharedObject(TraceProjectionSharedObject),
}

/// A trace object whose explicitly named aliases share one body until mutation.
/// Construction accepts fields, so inserting shared fields cannot encounter scalar JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraceProjectionSharedObject {
    fields: BTreeMap<String, Arc<Value>>,
}

impl TraceProjectionSharedObject {
    pub fn from_fields<K: Into<String>>(fields: impl IntoIterator<Item = (K, Value)>) -> Self {
        Self {
            fields: fields
                .into_iter()
                .map(|(key, value)| (key.into(), Arc::new(value)))
                .collect(),
        }
    }

    /// The caller identifies equal aliases; distinct existing fields are never deduplicated.
    pub fn set_shared_fields(&mut self, keys: &[&str], value: Value) {
        let shared = Arc::new(value);
        for key in keys {
            self.fields.insert((*key).to_owned(), Arc::clone(&shared));
        }
    }
}

impl From<TraceProjectionSharedObject> for TraceProjectionWritePayload {
    fn from(object: TraceProjectionSharedObject) -> Self {
        Self::SharedObject(object)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceProjectionObjectRequired;

impl fmt::Display for TraceProjectionObjectRequired {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("shared trace fields require an object or null payload")
    }
}

impl std::error::Error for TraceProjectionObjectRequired {}

impl From<Value> for TraceProjectionWritePayload {
    fn from(value: Value) -> Self {
        Self::Plain(value)
    }
}

impl TraceProjectionWritePayload {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Plain(value) => value.get(key),
            Self::SharedObject(object) => object.fields.get(key).map(Arc::as_ref),
        }
    }

    /// Null is an explicit empty-object seed, as with ordinary JSON field insertion.
    /// Other non-object JSON is rejected without changing or discarding its value.
    pub fn set_shared_fields(
        &mut self,
        keys: &[&str],
        value: Value,
    ) -> Result<(), TraceProjectionObjectRequired> {
        match self {
            Self::SharedObject(object) => object.set_shared_fields(keys, value),
            Self::Plain(Value::Object(fields)) => {
                let mut object = TraceProjectionSharedObject::from_fields(std::mem::take(fields));
                object.set_shared_fields(keys, value);
                *self = Self::SharedObject(object);
            }
            Self::Plain(Value::Null) => {
                let mut object = TraceProjectionSharedObject::default();
                object.set_shared_fields(keys, value);
                *self = Self::SharedObject(object);
            }
            Self::Plain(_) => return Err(TraceProjectionObjectRequired),
        }
        Ok(())
    }

    /// Only the per-content persistence boundary expands shared aliases to owned JSON.
    /// Ordinary content borrows its existing Value and incurs no additional clone here.
    pub fn materialize_for_persistence(&self) -> Cow<'_, Value> {
        match self {
            Self::Plain(value) => Cow::Borrowed(value),
            Self::SharedObject(object) => Cow::Owned(Value::Object(
                object
                    .fields
                    .iter()
                    .map(|(key, value)| (key.clone(), value.as_ref().clone()))
                    .collect(),
            )),
        }
    }
}

impl Serialize for TraceProjectionWritePayload {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Plain(value) => value.serialize(serializer),
            Self::SharedObject(shared) => {
                let mut object = serializer.serialize_map(Some(shared.fields.len()))?;
                for (key, value) in &shared.fields {
                    object.serialize_entry(key, value.as_ref())?;
                }
                object.end()
            }
        }
    }
}

impl Index<&str> for TraceProjectionWritePayload {
    type Output = Value;
    fn index(&self, key: &str) -> &Value {
        self.get(key).unwrap_or(&Value::Null)
    }
}

impl IndexMut<&str> for TraceProjectionWritePayload {
    fn index_mut(&mut self, key: &str) -> &mut Value {
        match self {
            Self::Plain(value) => &mut value[key],
            Self::SharedObject(object) => Arc::make_mut(
                object
                    .fields
                    .entry(key.to_owned())
                    .or_insert_with(|| Arc::new(Value::Null)),
            ),
        }
    }
}

impl PartialEq for TraceProjectionWritePayload {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Plain(a), Self::Plain(b)) => a == b,
            (Self::SharedObject(a), Self::SharedObject(b)) => a == b,
            (Self::Plain(value), Self::SharedObject(shared))
            | (Self::SharedObject(shared), Self::Plain(value)) => {
                value.as_object().is_some_and(|object| {
                    object.len() == shared.fields.len()
                        && object.iter().all(|(key, value)| {
                            shared
                                .fields
                                .get(key)
                                .is_some_and(|shared| shared.as_ref() == value)
                        })
                })
            }
        }
    }
}
impl Eq for TraceProjectionWritePayload {}

impl fmt::Display for TraceProjectionWritePayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&serde_json::to_string(self).map_err(|_| fmt::Error)?)
    }
}

#[cfg(test)]
#[path = "_tests/write_payload.rs"]
mod tests;
