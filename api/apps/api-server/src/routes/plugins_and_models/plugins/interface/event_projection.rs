use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for PluginInterfaceInput {
    fn document_input_unavailable(
        operation: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        matches!(
            operation.interface_id.as_str(),
            "plugins.install.upload" | "model_provider_plugins.install.upload"
        )
        .then_some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::ListCatalog { query, locale: _ }
            | Self::ListFamilies { query, locale: _ }
            | Self::ModelFamilies { query, locale: _ } => json!({"query":event::to_value(query)?}),
            Self::ListOfficial { query, locale: _ } | Self::ModelOfficial { query, locale: _ } => {
                json!({"query":event::to_value(query)?})
            }
            Self::InstallPath(body) => json!({"body":event::to_value(body)?}),
            Self::InstallUploaded { .. } | Self::ModelInstallUploaded { .. } => {
                return Err(Arc::from(
                    "plugin package upload has no JSON request event view",
                ))
            }
            Self::InstallOfficial(body) | Self::ModelInstallOfficial(body) => {
                json!({"body":event::to_value(body)?})
            }
            Self::RefreshCatalogProjection { installation_id }
            | Self::RefreshArtifact { installation_id }
            | Self::InstallArtifact { installation_id }
            | Self::Enable { installation_id }
            | Self::Assign { installation_id }
            | Self::ModelRefreshArtifact { installation_id }
            | Self::ModelInstallArtifact { installation_id } => {
                json!({"path":{"installation_id":installation_id}})
            }
            Self::UpgradeLatest {
                provider_code,
                body,
            }
            | Self::ModelUpgradeLatest {
                provider_code,
                body,
            } => {
                let mut value = json!({"path":{"provider_code":provider_code}});
                if let Some(body) = body {
                    value["body"] = event::to_value(body)?;
                }
                value
            }
            Self::SwitchVersion {
                provider_code,
                body,
            }
            | Self::ModelSwitchVersion {
                provider_code,
                body,
            } => json!({"path":{"provider_code":provider_code},"body":event::to_value(body)?}),
            Self::DeleteFamily { provider_code } | Self::ModelDeleteFamily { provider_code } => {
                json!({"path":{"provider_code":provider_code}})
            }
            Self::ListTasks => json!({}),
            Self::GetTask { task_id } | Self::ModelGetTask { task_id } => {
                json!({"path":{"task_id":task_id}})
            }
        })
    }
}
impl ApiEventOutput for PluginInterfaceOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Catalog(value) => event::to_value(value),
            Self::Families(value) => event::to_value(value),
            Self::Official(value) => event::to_value(value),
            Self::Installed(value) => event::to_value(value),
            Self::Projection(value) => event::to_value(value),
            Self::Artifact(value) => event::to_value(value),
            Self::Task(value) => event::to_value(value),
            Self::Tasks(value) => event::to_value(value),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
