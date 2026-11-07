//! Plugin declaration only. Host supplies actor, workspace and policy evaluation.
use super::{invalid, ManagedServiceManifest};
use crate::{FrameworkResult, PluginManifestV1};
use extension_contracts::PluginDataFieldType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceRecordResource {
    pub resource_code: String,
    pub collection_code: String,
    pub id_field: String,
    pub label: String,
    pub description: String,
    pub actions: Vec<ManagedServiceRecordAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceRecordAction {
    pub action_code: String,
    pub label: String,
    pub description: String,
    pub operation_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceRecordPermission {
    pub resource_code: String,
    pub action_code: String,
    pub selector: ManagedServiceRecordSelector,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_handle_pointer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedServiceRecordSelector {
    List,
    Record {
        pointer: String,
        #[serde(default)]
        allow_missing: bool,
    },
    Handle {
        pointer: String,
    },
}

fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= 63
        && bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Resolve concrete JSON object properties only. The schema must make the selector a string.
/// Required selectors cannot disappear through an optional parent or an unproven union branch.
fn schema_string_pointer(schema: &Value, pointer: &str, required: bool) -> bool {
    if !pointer.starts_with('/') || pointer.len() > 1024 {
        return false;
    }
    let mut current = schema;
    for raw in pointer[1..].split('/') {
        let mut chars = raw.chars();
        while let Some(character) = chars.next() {
            if character == '~' && !matches!(chars.next(), Some('0' | '1')) {
                return false;
            }
        }
        let field = raw.replace("~1", "/").replace("~0", "~");
        if field.is_empty() {
            return false;
        }
        if required
            && !current
                .get("required")
                .and_then(Value::as_array)
                .is_some_and(|fields| {
                    fields
                        .iter()
                        .any(|candidate| candidate.as_str() == Some(&field))
                })
        {
            return false;
        }
        let Some(child) = current
            .get("properties")
            .and_then(|properties| properties.get(&field))
        else {
            return false;
        };
        current = child;
    }
    current.get("type").and_then(Value::as_str) == Some("string")
}

pub(super) fn validate_record_permissions(
    manifest: &PluginManifestV1,
    service: &ManagedServiceManifest,
) -> FrameworkResult<()> {
    let mut resource_codes = BTreeSet::new();
    let mut collections = BTreeSet::new();
    if service.record_resources.len() > 64 {
        return Err(invalid("too many managed record resources"));
    }
    for resource in &service.record_resources {
        if !identifier(&resource.resource_code)
            || !identifier(&resource.collection_code)
            || !identifier(&resource.id_field)
            || !resource_codes.insert(&resource.resource_code)
            || !collections.insert(&resource.collection_code)
            || resource.label.trim().is_empty()
            || resource.description.trim().is_empty()
            || resource.actions.is_empty()
            || resource.actions.len() > 32
        {
            return Err(invalid("managed record resource requires unique code, collection, identity and action descriptors"));
        }
        let owned: Vec<_> = manifest
            .data_models
            .iter()
            .flat_map(|model| &model.owned_collections)
            .filter(|collection| collection.collection_code == resource.collection_code)
            .collect();
        if owned.len() != 1 {
            return Err(invalid(
                "managed record resource must reference one plugin-owned collection",
            ));
        }
        // id is the Host-owned nonnullable UUID column for every owned collection.
        if resource.id_field != "id"
            && !owned[0].fields.iter().any(|field| {
                field.field_code == resource.id_field
                    && !field.nullable
                    && matches!(
                        field.field_type,
                        PluginDataFieldType::String
                            | PluginDataFieldType::Text
                            | PluginDataFieldType::Uuid
                    )
            })
        {
            return Err(invalid(
                "managed record identity must be a nonnullable owned string or UUID field",
            ));
        }
        let mut actions = BTreeSet::new();
        for action in &resource.actions {
            if !identifier(&action.action_code)
                || !actions.insert(&action.action_code)
                || action.label.trim().is_empty()
                || action.description.trim().is_empty()
                || action.operation_ids.is_empty()
            {
                return Err(invalid("managed record actions require unique codes, descriptors and operation references"));
            }
            let mut references = BTreeSet::new();
            for id in &action.operation_ids {
                let Some(operation) = service
                    .operations
                    .iter()
                    .find(|operation| &operation.interface_id == id)
                else {
                    return Err(invalid(
                        "managed record action references an unknown operation",
                    ));
                };
                if !references.insert(id)
                    || !operation.record_permission.as_ref().is_some_and(|binding| {
                        binding.resource_code == resource.resource_code
                            && binding.action_code == action.action_code
                    })
                {
                    return Err(invalid("managed record action operation reference must match its resource and action"));
                }
            }
        }
    }
    for operation in &service.operations {
        let Some(binding) = &operation.record_permission else {
            continue;
        };
        let Some(resource) = service
            .record_resources
            .iter()
            .find(|resource| resource.resource_code == binding.resource_code)
        else {
            return Err(invalid(
                "managed record operation references an unknown resource",
            ));
        };
        if !resource.actions.iter().any(|action| {
            action.action_code == binding.action_code
                && action.operation_ids.contains(&operation.interface_id)
        }) {
            return Err(invalid(
                "managed record operation references an undeclared action",
            ));
        }
        match &binding.selector {
            ManagedServiceRecordSelector::List
                if binding.action_code != "view" || operation.method != "GET" =>
            {
                return Err(invalid(
                    "managed record list selector requires a GET view action",
                ));
            }
            ManagedServiceRecordSelector::Record {
                pointer,
                allow_missing,
            } => {
                if !schema_string_pointer(&operation.input_schema, pointer, !allow_missing) {
                    return Err(invalid("managed record selector must reference a schema string with explicit requiredness"));
                }
                if !["/path/", "/query/", "/body/"]
                    .iter()
                    .any(|prefix| pointer.starts_with(prefix))
                {
                    return Err(invalid(
                        "managed record selector must use a public request location",
                    ));
                }
            }
            ManagedServiceRecordSelector::Handle { pointer } => {
                if binding.action_code != "open"
                    || !schema_string_pointer(&operation.input_schema, pointer, true)
                    || !["/path/", "/query/", "/body/"]
                        .iter()
                        .any(|prefix| pointer.starts_with(prefix))
                {
                    return Err(invalid("managed handle selector requires an open action and a required schema string"));
                }
            }
            _ => {}
        }
        if let Some(pointer) = &binding.response_handle_pointer {
            if binding.action_code != "open"
                || !matches!(
                    binding.selector,
                    ManagedServiceRecordSelector::Record {
                        allow_missing: false,
                        ..
                    }
                )
                || !schema_string_pointer(&operation.output_schema, pointer, true)
            {
                return Err(invalid("managed response handle requires an open record selector and required output string"));
            }
        }
    }
    Ok(())
}
