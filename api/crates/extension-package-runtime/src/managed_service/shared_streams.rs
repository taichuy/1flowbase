//! Generic Host-owned shared producer and snapshot declarations.
use super::{
    invalid, ManagedServiceManifest, ManagedServiceOperation, ManagedServiceRecordSelector,
};
use crate::FrameworkResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceSharedSource {
    /// Local unary record/open operation returning a required `revision: string`.
    pub resolve_interface_id: String,
    /// Public request JSON pointer to the requested sampling interval in milliseconds.
    pub interval_pointer: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceSharedSnapshot {
    pub source_interface_id: String,
    /// JSON pointers into the original source event, never a Host envelope.
    pub value_pointer: String,
    pub ready_pointer: String,
    pub ready_value: Value,
    pub interval_pointer: String,
}

/// Resolve only concrete schema properties, with strict RFC 6901 escaping. Empty
/// pointer projects the whole value; request interval pointers must be nonempty.
fn schema_pointer<'a>(schema: &'a Value, pointer: &str, required: bool) -> Option<&'a Value> {
    if pointer.is_empty() {
        return Some(schema);
    }
    if !pointer.starts_with('/') || pointer.len() > 1024 {
        return None;
    }
    let mut current = schema;
    for raw in pointer[1..].split('/') {
        let mut chars = raw.chars();
        while let Some(character) = chars.next() {
            if character == '~' && !matches!(chars.next(), Some('0' | '1')) {
                return None;
            }
        }
        let key = raw.replace("~1", "/").replace("~0", "~");
        if key.is_empty()
            || (required
                && !current
                    .get("required")?
                    .as_array()?
                    .iter()
                    .any(|v| v.as_str() == Some(&key)))
        {
            return None;
        }
        current = current.get("properties")?.get(&key)?;
    }
    Some(current)
}

fn interval(operation: &ManagedServiceOperation, pointer: &str) -> bool {
    ["/path/", "/query/", "/body/"]
        .iter()
        .any(|prefix| pointer.starts_with(prefix))
        && schema_pointer(&operation.input_schema, pointer, true)
            .is_some_and(|schema| schema.get("type").and_then(Value::as_str) == Some("integer"))
}

fn record_open(operation: &ManagedServiceOperation) -> bool {
    operation
        .record_permission
        .as_ref()
        .is_some_and(|permission| {
            permission.action_code == "open"
                && permission.response_handle_pointer.is_none()
                && matches!(
                    permission.selector,
                    ManagedServiceRecordSelector::Record {
                        allow_missing: false,
                        ..
                    }
                )
        })
}

fn same_record(left: &ManagedServiceOperation, right: &ManagedServiceOperation) -> bool {
    record_open(left) && record_open(right) && left.record_permission == right.record_permission
}

fn source<'a>(
    service: &'a ManagedServiceManifest,
    id: &str,
) -> Option<&'a ManagedServiceOperation> {
    service.operations.iter().find(|operation| {
        operation.interface_id == id
            && operation.stream.as_ref().is_some_and(|stream| {
                stream.shared_source.is_some() && stream.aggregate_source_interface_id.is_none()
            })
    })
}

pub(super) fn validate_shared_streams(service: &ManagedServiceManifest) -> FrameworkResult<()> {
    for operation in &service.operations {
        if let Some(stream) = &operation.stream {
            if let Some(shared) = &stream.shared_source {
                let resolver = service
                    .operations
                    .iter()
                    .find(|candidate| candidate.interface_id == shared.resolve_interface_id)
                    .ok_or_else(|| invalid("shared source resolver must be a local operation"))?;
                if stream.aggregate_source_interface_id.is_some()
                    || operation.shared_snapshot.is_some()
                    || !interval(operation, &shared.interval_pointer)
                    || !same_record(operation, resolver)
                    || resolver.stream.is_some()
                    || resolver.shared_snapshot.is_some()
                    || !schema_pointer(&resolver.output_schema, "/revision", true).is_some_and(
                        |schema| schema.get("type").and_then(Value::as_str) == Some("string"),
                    )
                {
                    return Err(invalid("shared source requires an interval and unary same-record open resolver with required revision string"));
                }
            }
            if let Some(id) = &stream.aggregate_source_interface_id {
                if stream.shared_source.is_some()
                    || operation.shared_snapshot.is_some()
                    || operation.record_permission.is_some()
                    || source(service, id).is_none()
                {
                    return Err(invalid("aggregate stream requires a local shared record source and per-member authorization"));
                }
            }
        }
        if let Some(snapshot) = &operation.shared_snapshot {
            let source = source(service, &snapshot.source_interface_id)
                .ok_or_else(|| invalid("shared snapshot requires a local shared record source"))?;
            let event = &source
                .stream
                .as_ref()
                .expect("validated source stream")
                .event_schema;
            let ready = schema_pointer(event, &snapshot.ready_pointer, false).and_then(|schema| {
                jsonschema::options()
                    .with_draft(jsonschema::Draft::Draft202012)
                    .build(schema)
                    .ok()
            });
            if operation.stream.is_some()
                || !same_record(operation, source)
                || !interval(operation, &snapshot.interval_pointer)
                || snapshot.ready_pointer.is_empty()
                || !ready.is_some_and(|validator| validator.is_valid(&snapshot.ready_value))
                || schema_pointer(event, &snapshot.value_pointer, false)
                    != Some(&operation.output_schema)
            {
                return Err(invalid("shared snapshot requires unary same-record open scope, interval and schema-compatible ready/value projection"));
            }
        }
    }
    Ok(())
}
