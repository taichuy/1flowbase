//! Manifest-owned safe business JSON. Only explicit owner sensitivity markers
//! remove fields; names of ordinary business properties do not imply credentials.
use crate::openapi_interface::contract::{ApiInput, ApiOperationContext, ApiOutput};
use interface_runtime::{CompiledInterfaceRegistry, RegistryCompiler};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub(super) struct OperationPolicy {
    interface_id: String,
    method: String,
    path: String,
    input: Value,
    output: Value,
    request_schema: Option<Value>,
    result_schema: Option<Value>,
    streaming: bool,
}

impl OperationPolicy {
    pub(super) fn new(operation: &plugin_framework::ManagedServiceOperation) -> Self {
        let input = request_schema_without_headers(
            super::registration::operation_document_input_schema(operation),
        );
        Self {
            interface_id: operation.interface_id.clone(),
            method: operation.method.clone(),
            path: operation.path.clone(),
            request_schema: safe_schema(&operation.input_schema).and_then(|_| safe_schema(&input)),
            input,
            output: operation.output_schema.clone(),
            result_schema: safe_schema(&operation.output_schema),
            streaming: operation.stream.is_some(),
        }
    }

    fn check(&self, operation: &ApiOperationContext) -> Result<(), Arc<str>> {
        if operation.interface_id != self.interface_id
            || operation.method != self.method
            || crate::openapi_interface::contract::route_identity(
                &operation.method,
                &operation.path,
            ) != crate::openapi_interface::contract::route_identity(&self.method, &self.path)
        {
            return Err(Arc::from("managed_service_event_policy_mismatch"));
        }
        Ok(())
    }

    fn result_schema(&self) -> Option<Value> {
        self.result_schema.as_ref().map(|result| {
            if self.streaming {
                // Ticket issuance and duplex execution share the same binding.
                // Both are real typed terminals; the ticket secret is omitted.
                json!({"anyOf":[ticket_schema(),result]})
            } else {
                result.clone()
            }
        })
    }
}

fn request_schema_without_headers(schema: Value) -> Value {
    let mut schema = crate::openapi_interface::contract::safe_document_request_schema(schema);
    // Logical branches at the request root still describe the same canonical
    // header carrier. Business body properties are not traversed by this rule.
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = schema.get_mut(keyword).and_then(Value::as_array_mut) {
            for branch in branches {
                *branch = request_schema_without_headers(branch.clone());
            }
        }
    }
    for keyword in ["if", "then", "else", "not"] {
        if let Some(branch) = schema.get_mut(keyword) {
            *branch = request_schema_without_headers(branch.clone());
        }
    }
    schema
}

/// A terminal kind, not a fabricated stream-completion flag. The HTTP owner still
/// consumes the original value unchanged in ManagedServiceOutput.0.
pub(super) enum OutputPolicy {
    TicketIssued(Arc<OperationPolicy>),
    BusinessResult(Arc<OperationPolicy>),
}

fn ticket_schema() -> Value {
    json!({"type":"object","properties":{},"additionalProperties":false})
}

/// These are schema annotations supplied by the owning manifest, not field-name
/// guesses or a string-pattern scan over arbitrary request/response values.
fn sensitive(schema: &Value) -> bool {
    schema.get("secret").and_then(Value::as_bool) == Some(true)
        || schema.get("writeOnly").and_then(Value::as_bool) == Some(true)
        || schema.get("format").and_then(Value::as_str) == Some("password")
        || ["allOf", "anyOf", "oneOf"].iter().any(|keyword| {
            schema
                .get(*keyword)
                .and_then(Value::as_array)
                .is_some_and(|branches| branches.iter().any(sensitive))
        })
        || schema.get("items").is_some_and(sensitive)
}

fn contains_marker(schema: &Value) -> bool {
    if sensitive(schema) {
        return true;
    }
    let Some(fields) = schema.as_object() else {
        return schema
            .as_array()
            .is_some_and(|schemas| schemas.iter().any(contains_marker));
    };
    ["properties", "patternProperties", "dependentSchemas"]
        .iter()
        .any(|keyword| {
            fields
                .get(*keyword)
                .and_then(Value::as_object)
                .is_some_and(|children| children.values().any(contains_marker))
        })
        || [
            "items",
            "prefixItems",
            "additionalProperties",
            "allOf",
            "anyOf",
            "oneOf",
            "not",
            "if",
            "then",
            "else",
            "contains",
            "unevaluatedProperties",
            "unevaluatedItems",
            "propertyNames",
            "contentSchema",
        ]
        .iter()
        .any(|keyword| fields.get(*keyword).is_some_and(contains_marker))
}

fn complex_schema(schema: &Value) -> bool {
    [
        "allOf",
        "not",
        "if",
        "then",
        "else",
        "contains",
        "prefixItems",
        "patternProperties",
        "dependentSchemas",
        "unevaluatedProperties",
        "unevaluatedItems",
        "propertyNames",
        "contentSchema",
    ]
    .iter()
    .any(|keyword| schema.get(*keyword).is_some())
        || schema.get("items").is_some_and(Value::is_array)
        || (schema.get("properties").is_some()
            && ["anyOf", "oneOf"]
                .iter()
                .any(|keyword| schema.get(*keyword).is_some()))
}

fn safe_schema(source: &Value) -> Option<Value> {
    if sensitive(source) {
        return None;
    }
    if complex_schema(source) {
        // Preserve authentic complex business contracts. A credential marker in
        // an unevaluated/conditional/tuple contract has no unambiguous safe field
        // selection here, so omit that explicit credential carrier fail-closed.
        return (!contains_marker(source)).then(|| source.clone());
    }
    let mut schema = source.clone();
    let Some(fields) = schema.as_object_mut() else {
        // True/false are authentic JSON Schemas, including ordinary untyped JSON.
        return Some(schema);
    };
    let mut omitted = Vec::new();
    // Managed schemas prohibit references, so unused definitions are not part
    // of the business value contract or its safe discovery view.
    fields.remove("$defs");
    fields.remove("definitions");
    if let Some(properties) = fields.get_mut("properties").and_then(Value::as_object_mut) {
        properties.retain(|name, child| match safe_schema(child) {
            Some(safe) => {
                *child = safe;
                true
            }
            None => {
                omitted.push(name.clone());
                false
            }
        });
        // A declared DTO exposes its declared safe fields, not unknown extras.
        // Explicitly typed maps retain their actual business entries below.
        if !fields
            .get("additionalProperties")
            .is_some_and(Value::is_object)
        {
            fields.insert("additionalProperties".into(), Value::Bool(false));
        }
    }
    if let Some(required) = fields.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|name| {
            name.as_str()
                .is_none_or(|name| !omitted.iter().any(|item| item == name))
        });
    }
    if !omitted.is_empty() {
        // Object-size and cross-property constraints refer to the original
        // credential-bearing object; they do not describe the selected safe view.
        fields.remove("minProperties");
        fields.remove("maxProperties");
        if let Some(dependencies) = fields
            .get_mut("dependentRequired")
            .and_then(Value::as_object_mut)
        {
            dependencies.retain(|name, dependencies| {
                if omitted.contains(name) {
                    return false;
                }
                if let Some(names) = dependencies.as_array_mut() {
                    names.retain(|name| {
                        name.as_str()
                            .is_none_or(|name| !omitted.iter().any(|item| item == name))
                    });
                }
                true
            });
        }
    }
    for keyword in [
        "items",
        "additionalProperties",
        "contains",
        "not",
        "if",
        "then",
        "else",
        "propertyNames",
    ] {
        if let Some(child) = fields.get_mut(keyword) {
            *child = safe_schema(child).unwrap_or(Value::Bool(false));
        }
    }
    for keyword in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(children) = fields.get_mut(keyword).and_then(Value::as_array_mut) {
            for child in children {
                *child = safe_schema(child).unwrap_or(Value::Bool(false));
            }
        }
    }
    Some(schema)
}

fn project(value: &Value, schema: &Value) -> Result<Value, Arc<str>> {
    if sensitive(schema) {
        return Err(Arc::from("managed_service_event_credential_boundary"));
    }
    if complex_schema(schema) {
        if contains_marker(schema) {
            return Err(Arc::from("managed_service_event_credential_boundary"));
        }
        return Ok(value.clone());
    }
    for keyword in ["anyOf", "oneOf"] {
        if let Some(branches) = schema.get(keyword).and_then(Value::as_array) {
            let branch = branches
                .iter()
                .find(|branch| {
                    jsonschema::validator_for(branch)
                        .is_ok_and(|validator| validator.is_valid(value))
                })
                .ok_or_else(|| Arc::<str>::from("invalid_managed_service_event_union"))?;
            return project(value, branch);
        }
    }
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        let Some(values) = value.as_object() else {
            return jsonschema::validator_for(schema)
                .is_ok_and(|validator| validator.is_valid(value))
                .then(|| value.clone())
                .ok_or_else(|| Arc::from("invalid_managed_service_event_object"));
        };
        let additional = schema
            .get("additionalProperties")
            .filter(|schema| schema.is_object());
        let mut output = serde_json::Map::new();
        for (name, value) in values {
            if let Some(child) = properties.get(name).or(additional) {
                if safe_schema(child).is_some() {
                    output.insert(name.clone(), project(value, child)?);
                }
            }
        }
        return Ok(Value::Object(output));
    }
    if let Some(items) = schema.get("items") {
        let Some(values) = value.as_array() else {
            return jsonschema::validator_for(schema)
                .is_ok_and(|validator| validator.is_valid(value))
                .then(|| value.clone())
                .ok_or_else(|| Arc::from("invalid_managed_service_event_array"));
        };
        return values
            .iter()
            .map(|value| project(value, items))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    if let Some(additional) = schema
        .get("additionalProperties")
        .filter(|schema| schema.is_object())
    {
        let values = value
            .as_object()
            .ok_or_else(|| Arc::<str>::from("invalid_managed_service_event_map"))?;
        return values
            .iter()
            .filter(|_| !sensitive(additional))
            .map(|(key, value)| Ok((key.clone(), project(value, additional)?)))
            .collect::<Result<serde_json::Map<_, _>, Arc<str>>>()
            .map(Value::Object);
    }
    // Unconstrained JSON remains ordinary business JSON. Secret-bearing carriers
    // require the owning manifest's explicit sensitivity annotation.
    Ok(value.clone())
}

impl ApiInput for super::interface::ManagedServiceInput {
    fn project_document_input(&self, operation: &ApiOperationContext) -> Result<Value, Arc<str>> {
        self.event_policy.check(operation)?;
        if self.interface_id != operation.interface_id {
            return Err(Arc::from("managed_service_event_input_mismatch"));
        }
        if self.event_policy.request_schema.is_none() {
            return Err(Arc::from("managed_service_event_credential_boundary"));
        }
        // StreamInput's Origin, cancel receiver and RuntimeManagedDuplex are never
        // inspected or serialized; only the current post-Before business payload.
        let mut value = project(&self.payload, &self.event_policy.input)?;
        if let Some(fields) = value.as_object_mut() {
            fields.remove("headers");
        }
        Ok(value)
    }
}

impl ApiOutput for super::interface::ManagedServiceOutput {
    fn project_document_output(&self, operation: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match &self.1 {
            OutputPolicy::TicketIssued(policy) => {
                policy.check(operation)?;
                Ok(json!({}))
            }
            OutputPolicy::BusinessResult(policy) => {
                policy.check(operation)?;
                if policy.result_schema.is_none() {
                    return Err(Arc::from("managed_service_event_credential_boundary"));
                }
                project(&self.0, &policy.output)
            }
        }
    }
}

pub(super) fn refine_registry(
    compiler: &mut RegistryCompiler,
    registry: &CompiledInterfaceRegistry,
    policies: &BTreeMap<String, Arc<OperationPolicy>>,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    for capability in registry.document_operations() {
        let Some(binding) = registry.binding(capability.binding_id) else {
            continue;
        };
        let Some(route) = binding.projection().http_route() else {
            continue;
        };
        let policy = policies
            .get(binding.interface_identity().interface_id().as_str())
            .ok_or_else(|| {
                interface_runtime::RegistryCompilationError::DocumentOperationMismatch(
                    capability.binding_id.clone(),
                )
            })?;
        // Generic console compilation already registered the typed codec once.
        // Refine only its descriptor from this same manifest, before publication.
        let mut descriptor = capability
            .static_descriptor
            .expect("managed operation is static within its frozen generation")
            .clone();
        let context = ApiOperationContext {
            interface_id: policy.interface_id.clone(),
            method: route.method().into(),
            path: route.path().into(),
        };
        if policy.check(&context).is_err() {
            return Err(
                interface_runtime::RegistryCompilationError::DocumentOperationMismatch(
                    capability.binding_id.clone(),
                ),
            );
        }
        descriptor.schemas.request = policy.request_schema.clone();
        descriptor.schemas.result = policy.result_schema();
        descriptor.schemas.request_unavailable = policy
            .request_schema
            .is_none()
            .then(|| Arc::from("credential_boundary"));
        descriptor.schemas.result_unavailable = policy
            .result_schema
            .is_none()
            .then(|| Arc::from("credential_boundary"));
        compiler.rebind_document_operation_descriptor(capability.binding_id, descriptor)?;
    }
    Ok(())
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/document_event_projection.rs"]
mod tests;
