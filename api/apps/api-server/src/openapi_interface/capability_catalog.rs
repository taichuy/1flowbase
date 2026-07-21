use control_plane::ports::ModelDefinitionRepository;

use crate::{app_state::ApiState, error_response::ApiError, runtime_data_model_docs};

use super::{catalog_entry_from_operation, OpenApiInterfaceCatalogEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenApiCapabilitySource {
    StaticApiDocs,
    RuntimeDataModelCrud,
}

impl OpenApiCapabilitySource {
    pub fn adapter_id(self) -> &'static str {
        match self {
            Self::StaticApiDocs => "console_openapi",
            Self::RuntimeDataModelCrud => "runtime_data_model",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenApiCapabilityCatalogEntry {
    pub interface: OpenApiInterfaceCatalogEntry,
    pub source: OpenApiCapabilitySource,
    pub risk_level: &'static str,
    pub bindable: bool,
    pub disabled_reason: Option<&'static str>,
}

pub async fn build_openapi_capability_catalog(
    state: &ApiState,
    workspace_id: uuid::Uuid,
) -> Result<Vec<OpenApiCapabilityCatalogEntry>, ApiError> {
    let mut entries = Vec::new();

    for category in &state.api_docs.catalog().categories {
        let Some(operations) = state.api_docs.category_operations(&category.id) else {
            continue;
        };
        for operation in &operations.operations {
            let Some(spec) = state.api_docs.operation_spec(&operation.id) else {
                continue;
            };
            let Some(interface) = catalog_entry_from_operation(operation, spec) else {
                continue;
            };
            let bindable = operation.path.starts_with("/api/console/");
            entries.push(OpenApiCapabilityCatalogEntry {
                risk_level: operation_risk_level(&interface.method),
                interface,
                source: OpenApiCapabilitySource::StaticApiDocs,
                bindable,
                disabled_reason: (!bindable).then_some("unsupported_interface_scope"),
            });
        }
    }

    let mut models = state.store.list_model_definitions(workspace_id).await?;
    models.retain(|model| model.status == domain::DataModelStatus::Published);
    models.sort_by(|left, right| left.code.cmp(&right.code));
    let operations = runtime_data_model_docs::build_category_operations(&models);
    for operation in operations.operations {
        let Ok(Some((model_id, kind))) = runtime_data_model_docs::parse_operation_id(&operation.id)
        else {
            continue;
        };
        let Some(model) = models.iter().find(|model| model.id == model_id) else {
            continue;
        };
        let spec = runtime_data_model_docs::build_operation_openapi(model, kind);
        let Some(interface) = catalog_entry_from_operation(&operation, &spec) else {
            continue;
        };
        entries.push(OpenApiCapabilityCatalogEntry {
            risk_level: operation_risk_level(&interface.method),
            interface,
            source: OpenApiCapabilitySource::RuntimeDataModelCrud,
            bindable: true,
            disabled_reason: None,
        });
    }

    entries.sort_by(|left, right| {
        left.interface
            .operation_id
            .cmp(&right.interface.operation_id)
    });
    Ok(entries)
}

pub fn operation_risk_level(method: &str) -> &'static str {
    match method.to_ascii_uppercase().as_str() {
        "GET" | "HEAD" | "OPTIONS" => "low",
        "DELETE" => "critical",
        "POST" | "PUT" | "PATCH" => "high",
        _ => "medium",
    }
}
