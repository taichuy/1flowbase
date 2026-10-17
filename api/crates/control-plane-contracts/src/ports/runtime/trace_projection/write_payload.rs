//! Trace write-only ownership. Domain reads and stored/wire JSON remain ordinary Value.
use serde::{Serialize, Serializer, ser::SerializeMap};
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
    SharedObject(BTreeMap<String, Arc<Value>>),
}

impl From<Value> for TraceProjectionWritePayload {
    fn from(value: Value) -> Self {
        Self::Plain(value)
    }
}

impl TraceProjectionWritePayload {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Plain(value) => value.get(key),
            Self::SharedObject(fields) => fields.get(key).map(Arc::as_ref),
        }
    }

    /// The caller explicitly identifies equal aliases; never deduplicate distinct fields.
    /// A new owned body backs these fields directly, before ordinary JSON expansion.
    pub fn set_shared_fields(&mut self, keys: &[&str], value: Value) {
        if let Self::Plain(plain) = self {
            let fields = match std::mem::take(plain) {
                Value::Object(fields) => fields
                    .into_iter()
                    .map(|(key, value)| (key, Arc::new(value)))
                    .collect(),
                Value::Null => BTreeMap::new(),
                _ => panic!("shared trace fields require an object payload"),
            };
            *self = Self::SharedObject(fields);
        }
        let Self::SharedObject(fields) = self else {
            unreachable!()
        };
        let shared = Arc::new(value);
        for key in keys {
            fields.insert((*key).to_owned(), Arc::clone(&shared));
        }
    }

    /// Only the per-content persistence boundary expands shared aliases to owned JSON.
    /// Ordinary content borrows its existing Value and incurs no additional clone here.
    pub fn materialize_for_persistence(&self) -> Cow<'_, Value> {
        match self {
            Self::Plain(value) => Cow::Borrowed(value),
            Self::SharedObject(fields) => Cow::Owned(Value::Object(
                fields
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
            Self::SharedObject(fields) => {
                let mut object = serializer.serialize_map(Some(fields.len()))?;
                for (key, value) in fields {
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
            Self::SharedObject(fields) => Arc::make_mut(
                fields
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
            (Self::Plain(value), Self::SharedObject(fields))
            | (Self::SharedObject(fields), Self::Plain(value)) => {
                value.as_object().is_some_and(|object| {
                    object.len() == fields.len()
                        && object.iter().all(|(key, value)| {
                            fields
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
