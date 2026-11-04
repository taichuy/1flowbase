//! Declarative system service registration. The host owns routes, authorization and execution.
use crate::{FrameworkResult, PluginExecutionMode, PluginFrameworkError, PluginManifestV1};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const MANAGED_SERVICE_POINT: &str = "1flowbase.managed-service.operation";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedServiceScope {
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceFeature {
    pub feature_id: String,
    pub label: String,
    pub description: String,
    pub route_id: String,
    pub path: String,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceMcpDeclaration {
    /// Package-local fixed operation name; Host prefixes the plugin owner namespace.
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_mapping: Option<Value>,
}

/// Explicit MCP media projection; business responses retain their own schema.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpMediaMapping {
    pub mcp_content: Vec<ManagedMcpImageMapping>,
    pub structured_content_path: String,
    #[serde(default)]
    pub resource_read: bool,
}

/// Canonical typed content declaration; the historical name remains reexported.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedMcpImageMapping {
    Text {
        text_path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations_path: Option<String>,
    },
    Image {
        data_path: String,
        mime_type_path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations_path: Option<String>,
    },
    Audio {
        data_path: String,
        mime_type_path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations_path: Option<String>,
    },
    ResourceLink {
        uri_path: String,
        name_path: String,
        #[serde(default)]
        mime_type_path: Option<String>,
        #[serde(default)]
        description_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations_path: Option<String>,
    },
    Resource {
        uri_path: String,
        #[serde(default)]
        mime_type_path: Option<String>,
        #[serde(default)]
        text_path: Option<String>,
        #[serde(default)]
        blob_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations_path: Option<String>,
    },
}

impl ManagedMcpMediaMapping {
    pub fn parse(value: &Value) -> Result<Self, &'static str> {
        let mapping: Self =
            serde_json::from_value(value.clone()).map_err(|_| "invalid media mapping structure")?;
        fn pointer(path: &str) -> bool {
            if !path.starts_with('/') {
                return false;
            }
            let mut bytes = path.bytes();
            while let Some(byte) = bytes.next() {
                if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
                    return false;
                }
            }
            true
        }
        fn overlaps(a: &str, b: &str) -> bool {
            a == b
                || a.strip_prefix(b).is_some_and(|tail| tail.starts_with('/'))
                || b.strip_prefix(a).is_some_and(|tail| tail.starts_with('/'))
        }
        if mapping.mcp_content.is_empty() || !pointer(&mapping.structured_content_path) {
            return Err("invalid media mapping paths or content type");
        }
        for item in &mapping.mcp_content {
            use ManagedMcpImageMapping::*;
            let paths: Vec<&str> = match item {
                Text {
                    text_path,
                    annotations_path,
                } => std::iter::once(text_path.as_str())
                    .chain(annotations_path.as_deref())
                    .collect(),
                Image {
                    data_path,
                    mime_type_path,
                    annotations_path,
                }
                | Audio {
                    data_path,
                    mime_type_path,
                    annotations_path,
                } => [data_path.as_str(), mime_type_path.as_str()]
                    .into_iter()
                    .chain(annotations_path.as_deref())
                    .collect(),
                ResourceLink {
                    uri_path,
                    name_path,
                    mime_type_path,
                    description_path,
                    annotations_path,
                } => [uri_path.as_str(), name_path.as_str()]
                    .into_iter()
                    .chain(mime_type_path.as_deref())
                    .chain(description_path.as_deref())
                    .chain(annotations_path.as_deref())
                    .collect(),
                Resource {
                    uri_path,
                    mime_type_path,
                    text_path,
                    blob_path,
                    annotations_path,
                } => {
                    if text_path.is_some() == blob_path.is_some() {
                        return Err("resource requires exactly one text or blob path");
                    }
                    std::iter::once(uri_path.as_str())
                        .chain(mime_type_path.as_deref())
                        .chain(text_path.as_deref())
                        .chain(blob_path.as_deref())
                        .chain(annotations_path.as_deref())
                        .collect()
                }
            };
            if paths.iter().enumerate().any(|(index, path)| {
                !pointer(path)
                    || overlaps(path, &mapping.structured_content_path)
                    || paths[..index].iter().any(|other| overlaps(path, other))
            }) {
                return Err("invalid media mapping paths or content type");
            }
            if mapping.resource_read && !matches!(item, Resource { .. } | ResourceLink { .. }) {
                return Err("resource_read requires embedded resource content");
            }
        }
        if mapping.resource_read
            && !mapping
                .mcp_content
                .iter()
                .any(|item| matches!(item, ManagedMcpImageMapping::Resource { .. }))
        {
            return Err("resource_read requires embedded resource content");
        }
        Ok(mapping)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceStream {
    pub input_schema: Value,
    pub event_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceOperation {
    pub interface_id: String,
    pub contribution_id: String,
    pub method: String,
    pub path: String,
    pub summary: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<ManagedServiceStream>,
    #[serde(default)]
    pub invalidates_streams: bool,
    /// Successful execution retires shared actor workers without unmounting contributions.
    #[serde(default)]
    pub invalidates_workers: bool,
    #[serde(default)]
    pub execution_concurrency: ManagedServiceExecutionConcurrency,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<ManagedServiceMcpDeclaration>,
}

/// Mutation serialization is independent of authorization and HTTP mutability.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedServiceExecutionConcurrency {
    #[default]
    Serialized,
    Parallel,
}
impl ManagedServiceOperation {
    pub fn requires_mutation_lock(&self) -> bool {
        self.method != "GET"
            && self.execution_concurrency == ManagedServiceExecutionConcurrency::Serialized
    }
}

/// Carrier lifetime only: every operation retains its own deadline and authority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedServiceWorkerLifetime {
    #[default]
    PerCall,
    ActorScope,
    ActorWorkspaceScope,
}
impl ManagedServiceWorkerLifetime {
    pub fn is_shared(&self) -> bool {
        !self.is_per_call()
    }

    fn is_per_call(&self) -> bool {
        *self == Self::PerCall
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceMcpInstance {
    pub instance_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceManifest {
    pub scope: ManagedServiceScope,
    #[serde(
        default,
        skip_serializing_if = "ManagedServiceWorkerLifetime::is_per_call"
    )]
    pub worker_lifetime: ManagedServiceWorkerLifetime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_idle_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_instance: Option<ManagedServiceMcpInstance>,
    pub feature: ManagedServiceFeature,
    pub operations: Vec<ManagedServiceOperation>,
}

fn invalid(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_package(message)
}

pub(crate) fn validate_managed_service(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    let Some(service) = &manifest.managed_service else {
        return Ok(());
    };
    if service.worker_idle_timeout_ms == Some(0)
        || (service.worker_lifetime == ManagedServiceWorkerLifetime::PerCall
            && service.worker_idle_timeout_ms.is_some())
    {
        return Err(invalid(
            "managed service idle timeout requires a shared worker lifetime and must be positive",
        ));
    }
    let owner = manifest.plugin_code()?;
    let managed = manifest
        .managed
        .as_ref()
        .ok_or_else(|| invalid("managed_service requires managed declarations"))?;
    if manifest.manifest_version != 2
        || manifest.execution_mode != PluginExecutionMode::ProcessPerCall
        || manifest.binding_targets != ["system"]
        || service.operations.is_empty()
    {
        return Err(invalid(
            "managed_service requires v2 process_per_call system package and at least one operation",
        ));
    }
    if let Some(instance) = &service.mcp_instance {
        if instance.name.trim().is_empty()
            || instance.description.trim().is_empty()
            || !instance.instance_id.starts_with(&owner.replace('-', "_"))
            || !instance
                .instance_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || !service
                .operations
                .iter()
                .any(|operation| operation.mcp.is_some())
        {
            return Err(invalid(
                "managed MCP instance requires an owned instance_id, name, description and MCP operation",
            ));
        }
    }
    let namespace = format!("{owner}.");
    let owned_id = |value: &str| {
        value.starts_with(&namespace)
            && value.len() > namespace.len()
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    };
    let concrete_route = |value: &str| {
        value.split('/').skip(1).all(|segment| {
            if let Some(parameter) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                !parameter.is_empty()
                    && parameter
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            } else {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            }
        })
    };
    let feature = &service.feature;
    if !owned_id(&feature.feature_id)
        || feature.route_id.is_empty()
        || !feature
            .route_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || feature.path != format!("/settings/{}", feature.route_id)
        || feature.label.trim().is_empty()
        || feature.description.trim().is_empty()
    {
        return Err(invalid(
            "managed service feature must have an owned identity and concrete settings route",
        ));
    }
    if manifest.settings_pages.len() > 1
        || manifest
            .settings_pages
            .iter()
            .any(|p| p.feature_id != feature.feature_id)
    {
        return Err(invalid(
            "managed service settings page must belong to its feature",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut routes = BTreeSet::new();
    let mut contributions = BTreeSet::new();
    let mut mcp_names = BTreeSet::new();
    let mut shared_runtime = None;
    for operation in &service.operations {
        if let Some(mcp) = &operation.mcp {
            if let Some(mapping) = &mcp.output_mapping {
                ManagedMcpMediaMapping::parse(mapping).map_err(invalid)?;
            }
            let mut parameter_names = BTreeSet::new();
            for location in ["path", "query", "body"] {
                if let Some(fields) = operation
                    .input_schema
                    .pointer(&format!("/properties/{location}/properties"))
                    .and_then(Value::as_object)
                {
                    if fields.keys().any(|name| {
                        name.is_empty()
                            || !name
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                            || !parameter_names.insert(name)
                    }) {
                        return Err(invalid(
                            "managed MCP parameter names must be unique across path, query and body",
                        ));
                    }
                }
            }
            if mcp.name.is_empty()
                || mcp.name.len() > 64
                || !mcp
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                || mcp.description.trim().is_empty()
                || !mcp_names.insert(&mcp.name)
            {
                return Err(invalid(
                    "managed MCP operations require unique package-local names and descriptions",
                ));
            }
        }
        if !owned_id(&operation.interface_id)
            || !ids.insert(&operation.interface_id)
            || !contributions.insert(&operation.contribution_id)
            || !matches!(
                operation.method.as_str(),
                "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
            )
            || !operation
                .path
                .starts_with(&format!("/api/console/managed-services/{owner}/"))
            || !concrete_route(&operation.path)
            || operation.path.contains("..")
            || operation.path.contains(['?', '#', '*'])
            || !routes.insert((&operation.method, &operation.path))
            || operation.summary.trim().is_empty()
            || operation.description.trim().is_empty()
        {
            return Err(invalid(
                "managed service operations require unique owned identities and concrete method/routes",
            ));
        }
        let contribution = managed
            .module
            .contributions
            .iter()
            .find(|c| c.contribution_id.as_str() == operation.contribution_id)
            .ok_or_else(|| invalid("managed service operation contribution missing"))?;
        let binding = managed
            .execution_bindings
            .iter()
            .find(|b| b.contribution_id == contribution.contribution_id)
            .ok_or_else(|| invalid("managed service operation execution binding missing"))?;
        if contribution.point_id.as_str() != MANAGED_SERVICE_POINT
            || contribution.contract_version.as_str() != "1"
            || !contribution
                .required_permissions
                .iter()
                .any(|p| p.as_str() == "service.execute")
            || binding.execution_mode != PluginExecutionMode::ProcessPerCall
            || !(binding.runtime.protocol == extension_contracts::STDIO_JSON_MULTIPLEX_V1
                || (binding.runtime.protocol == "stdio_json"
                    && service.worker_lifetime == ManagedServiceWorkerLifetime::PerCall
                    && operation.stream.is_none()))
            || binding.interface_protocol.is_some()
            || binding.payload.is_some()
        {
            return Err(invalid(
                "managed service operation requires authorized unary or multiplex capability binding",
            ));
        }
        if operation.stream.is_some()
            && (operation.method != "POST"
                || !binding
                    .runtime
                    .capabilities
                    .iter()
                    .any(|value| value == "runtime_duplex/v1"))
        {
            return Err(invalid(
                "managed stream requires POST and runtime_duplex/v1 multiplex capability",
            ));
        }
        if operation.stream.is_some() && operation.mcp.is_some() {
            return Err(invalid("managed stream is a socket transport operation"));
        }
        if service.worker_lifetime.is_shared() {
            if let Some(runtime) = shared_runtime {
                if runtime != &binding.runtime {
                    return Err(invalid(
                        "shared worker operations must share one runtime entry and limits",
                    ));
                }
            } else {
                shared_runtime = Some(&binding.runtime);
            }
        }
        let mut schemas = vec![&operation.input_schema, &operation.output_schema];
        if let Some(stream) = &operation.stream {
            schemas.extend([&stream.input_schema, &stream.event_schema]);
        }
        for schema in schemas {
            fn references(value: &Value) -> bool {
                match value {
                    Value::Object(fields) => {
                        fields.contains_key("$ref")
                            || fields.contains_key("$dynamicRef")
                            || fields.values().any(references)
                    }
                    Value::Array(values) => values.iter().any(references),
                    _ => false,
                }
            }
            if references(schema) {
                return Err(invalid("managed service schemas must be self-contained"));
            }
            jsonschema::options()
                .with_draft(jsonschema::Draft::Draft202012)
                .build(schema)
                .map_err(|_| invalid("managed service schema is invalid"))?;
        }
    }
    Ok(())
}
