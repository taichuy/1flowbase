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
    pub mcp: Option<ManagedServiceMcpDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceManifest {
    pub scope: ManagedServiceScope,
    pub feature: ManagedServiceFeature,
    pub operations: Vec<ManagedServiceOperation>,
}

fn invalid(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_package(message)
}

pub(crate) fn validate_managed_service(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    let Some(service) = &manifest.managed_service else {
        if manifest
            .binding_targets
            .iter()
            .any(|scope| scope == "system")
        {
            return Err(invalid("system binding requires managed_service"));
        }
        return Ok(());
    };
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
    for operation in &service.operations {
        if let Some(mcp) = &operation.mcp {
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
                        return Err(invalid("managed MCP parameter names must be unique across path, query and body"));
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
            return Err(invalid("managed service operations require unique owned identities and concrete method/routes"));
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
            || binding.runtime.protocol != extension_contracts::STDIO_JSON_MULTIPLEX_V1
            || binding.interface_protocol.is_some()
            || binding.payload.is_some()
        {
            return Err(invalid(
                "managed service operation requires authorized multiplex capability binding",
            ));
        }
        for schema in [&operation.input_schema, &operation.output_schema] {
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
