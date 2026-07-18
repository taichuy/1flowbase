use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::application_public_api::protocol_translation::{
    anonymous_unknown_source_paths, TranslationSafeRepresentation,
};

const METADATA_PATH: &str = "$.metadata";

/// The Native request metadata has one durable owner: the external trace id.
/// Keeping this type closed prevents opaque wire metadata from becoming a
/// second request, fingerprint, or response truth.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeRequestMetadata {
    trace_id: Option<String>,
}

impl NativeRequestMetadata {
    pub(super) fn from_object(
        object: &Map<String, Value>,
    ) -> Result<Self, NativeRequestMetadataParseError> {
        let unknown_fields = object
            .keys()
            .filter(|field| field.as_str() != "trace_id")
            .collect::<Vec<_>>();
        if !unknown_fields.is_empty() {
            return Err(NativeRequestMetadataParseError::unknown_fields(
                unknown_fields,
            ));
        }
        let trace_id = match object.get("trace_id") {
            Some(value) => Some(
                value
                    .as_str()
                    .ok_or_else(NativeRequestMetadataParseError::trace_id)?
                    .to_owned(),
            ),
            None => None,
        };
        Ok(Self { trace_id })
    }

    pub fn with_trace_id(trace_id: Option<String>) -> Self {
        Self { trace_id }
    }

    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }

    pub fn as_value(&self) -> Value {
        let mut metadata = Map::new();
        if let Some(trace_id) = &self.trace_id {
            metadata.insert("trace_id".to_string(), Value::String(trace_id.clone()));
        }
        Value::Object(metadata)
    }
}

impl Serialize for NativeRequestMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NativeRequestMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| de::Error::custom("expected object"))?;
        Self::from_object(object).map_err(de::Error::custom)
    }
}

#[derive(Debug)]
pub(super) struct NativeRequestMetadataParseError {
    source_paths: Vec<String>,
    pub(super) message: &'static str,
    pub(super) reason: &'static str,
    pub(super) effective_value: TranslationSafeRepresentation,
}

impl NativeRequestMetadataParseError {
    fn trace_id() -> Self {
        Self {
            source_paths: vec![format!("{METADATA_PATH}.trace_id")],
            message: "metadata trace_id must be a string",
            reason: "Native metadata trace_id must be a string",
            effective_value: TranslationSafeRepresentation::Present,
        }
    }

    fn unknown_fields<'a>(fields: impl IntoIterator<Item = &'a String>) -> Self {
        Self {
            source_paths: anonymous_unknown_source_paths(METADATA_PATH, fields),
            message: "unknown Native metadata field",
            reason: "Native metadata field has no canonical owner",
            effective_value: TranslationSafeRepresentation::Present,
        }
    }

    pub(super) fn source_paths(&self) -> &[String] {
        &self.source_paths
    }
}

impl std::fmt::Display for NativeRequestMetadataParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}
