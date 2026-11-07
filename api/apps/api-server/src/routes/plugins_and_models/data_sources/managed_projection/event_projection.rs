use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct CreateEvent<'a> {
    installation_id: &'a str,
    source_code: &'a str,
    display_name: &'a str,
    config_json: &'a Value,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum BackendEvent<'a> {
    Core {
        durable_backend: &'a str,
    },
    RuntimeExtension {
        installation_id: &'a str,
        source_code: &'a str,
        config_json: &'a Value,
        secret_version: &'a Option<i32>,
        catalog_refresh_status: &'a Option<String>,
        catalog_last_error_message: &'a Option<String>,
        catalog_refreshed_at: &'a Option<String>,
    },
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct SourceEvent<'a> {
    id: &'a str,
    display_name: &'a str,
    status: &'a str,
    enabled: bool,
    fixed: bool,
    default_data_model_status: &'a str,
    capabilities: &'a DataSourceCapabilitiesResponse,
    backend: BackendEvent<'a>,
}
impl<'a> From<&'a DataSourceResponse> for SourceEvent<'a> {
    fn from(value: &'a DataSourceResponse) -> Self {
        let backend = match &value.backend {
            DataSourceBackendResponse::Core { durable_backend } => {
                BackendEvent::Core { durable_backend }
            }
            DataSourceBackendResponse::RuntimeExtension {
                installation_id,
                source_code,
                config_json,
                secret_ref: _,
                secret_version,
                catalog_refresh_status,
                catalog_last_error_message,
                catalog_refreshed_at,
            } => BackendEvent::RuntimeExtension {
                installation_id,
                source_code,
                config_json,
                secret_version,
                catalog_refresh_status,
                catalog_last_error_message,
                catalog_refreshed_at,
            },
        };
        Self {
            id: &value.id,
            display_name: &value.display_name,
            status: &value.status,
            enabled: value.enabled,
            fixed: value.fixed,
            default_data_model_status: &value.default_data_model_status,
            capabilities: &value.capabilities,
            backend,
        }
    }
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct ValidationEvent<'a> {
    data_source: SourceEvent<'a>,
    output: &'a Value,
}
impl ApiEventInput for DataSourcesInput {
    fn document_input_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "data_sources.create" => Some(
                json!({"type":"object","properties":{"body":event::schema::<CreateEvent<'static>>()},"required":["body"],"additionalProperties":false}),
            ),
            "data_sources.secret.rotate" => Some(
                json!({"type":"object","properties":{"path":{"type":"object","properties":{"data_source_id":{"type":"string"}},"required":["data_source_id"]},"body":{"type":"object","properties":{},"additionalProperties":false}},"required":["path","body"],"additionalProperties":false}),
            ),
            _ => None,
        }
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::ListAgentFlowOptions | Self::ListCatalog | Self::List => json!({}),
            Self::Create(body) => {
                json!({"body":event::to_value(&CreateEvent { installation_id:&body.installation_id, source_code:&body.source_code, display_name:&body.display_name, config_json:&body.config_json })?})
            }
            Self::UpdateDefaults {
                data_source_id,
                body,
            } => json!({"path":{"data_source_id":data_source_id},"body":event::to_value(body)?}),
            Self::Validate { data_source_id }
            | Self::ListResources { data_source_id }
            | Self::DiscoverResources { data_source_id } => {
                json!({"path":{"data_source_id":data_source_id}})
            }
            Self::RotateSecret {
                data_source_id,
                body: _,
            } => json!({"path":{"data_source_id":data_source_id},"body":{}}),
            Self::PreviewRead {
                data_source_id,
                body,
            } => json!({"path":{"data_source_id":data_source_id},"body":event::to_value(body)?}),
            Self::MapResourceToModel {
                data_source_id,
                body,
            } => json!({"path":{"data_source_id":data_source_id},"body":event::to_value(body)?}),
        })
    }
}
impl ApiEventOutput for DataSourcesOutput {
    fn document_output_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "data_sources.list" => Some(event::schema::<Vec<SourceEvent<'static>>>()),
            "data_sources.create"
            | "data_sources.defaults.update"
            | "data_sources.secret.rotate" => Some(event::schema::<SourceEvent<'static>>()),
            "data_sources.validate" => Some(event::schema::<ValidationEvent<'static>>()),
            _ => None,
        }
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::AgentFlowOptions(value) => event::to_value(value),
            Self::Catalog(value) => event::to_value(value),
            Self::List(value) => {
                event::to_value(&value.iter().map(SourceEvent::from).collect::<Vec<_>>())
            }
            Self::DataSource(value) => event::to_value(&SourceEvent::from(value)),
            Self::Validation(value) => event::to_value(&ValidationEvent {
                data_source: SourceEvent::from(&value.data_source),
                output: &value.output,
            }),
            Self::Resources(value) => event::to_value(value),
            Self::Preview(value) => event::to_value(value),
            Self::Model(value) => event::to_value(value),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
