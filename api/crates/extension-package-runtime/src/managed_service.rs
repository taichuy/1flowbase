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
pub struct ManagedServiceOperation {
    pub interface_id: String,
    pub contribution_id: String,
    pub method: String,
    pub path: String,
    pub summary: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
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
        || service.operations.len() > 128
    {
        return Err(invalid(
            "managed_service requires v2 process_per_call system package and 1..128 operations",
        ));
    }
    let namespace = format!("{owner}.");
    let feature = &service.feature;
    if !feature.feature_id.starts_with(&namespace)
        || feature.feature_id.len() == namespace.len()
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
    for operation in &service.operations {
        if !operation.interface_id.starts_with(&namespace)
            || !ids.insert(&operation.interface_id)
            || !contributions.insert(&operation.contribution_id)
            || !matches!(
                operation.method.as_str(),
                "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
            )
            || !operation
                .path
                .starts_with(&format!("/api/console/managed-services/{owner}/"))
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
        for (kind, schema) in [
            ("input", &operation.input_schema),
            ("output", &operation.output_schema),
        ] {
            extension_contracts::ManagedProjectionContract {
                contract_id: format!("{}.{}", operation.interface_id, kind),
                contract_version: "1".into(),
                schema: schema.clone(),
            }
            .compile_for_registration()
            .map_err(|_| invalid("managed service schema must be bounded and self-contained"))?;
        }
    }
    Ok(())
}
