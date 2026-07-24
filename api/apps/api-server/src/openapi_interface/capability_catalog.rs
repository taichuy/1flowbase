use std::collections::BTreeSet;

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

#[derive(Debug, Clone)]
pub struct OpenApiCapabilityCatalogSummary {
    pub interface_id: String,
    pub method: String,
    pub path: String,
    pub source: OpenApiCapabilitySource,
}

#[derive(Debug, Clone)]
pub struct OpenApiCapabilityCatalogQuery {
    pub path_prefixes: Vec<String>,
    pub path_query: Option<String>,
    pub adapter_id: Option<String>,
    pub method: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct OpenApiCapabilityCatalogPage {
    pub items: Vec<OpenApiCapabilityCatalogSummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub adapter_ids: Vec<String>,
    pub methods: Vec<String>,
}

pub async fn query_openapi_capability_catalog(
    state: &ApiState,
    workspace_id: uuid::Uuid,
    query: OpenApiCapabilityCatalogQuery,
) -> Result<OpenApiCapabilityCatalogPage, ApiError> {
    let mut summaries = openapi_capability_catalog_summaries(state, workspace_id).await?;
    let adapter_ids = summaries
        .iter()
        .map(|entry| entry.source.adapter_id().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let methods = summaries
        .iter()
        .map(|entry| entry.method.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    if !query.path_prefixes.is_empty() {
        summaries.retain(|entry| {
            query
                .path_prefixes
                .iter()
                .any(|prefix| entry.path.starts_with(prefix))
        });
    }

    if let Some(path_query) = query
        .path_query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let path_query = path_query.to_ascii_lowercase();
        summaries.retain(|entry| entry.path.to_ascii_lowercase().contains(&path_query));
    }
    if let Some(adapter_id) = query
        .adapter_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        summaries.retain(|entry| entry.source.adapter_id() == adapter_id);
    }
    if let Some(method) = query
        .method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let method = method.to_ascii_uppercase();
        summaries.retain(|entry| entry.method == method);
    }

    let total = summaries.len();
    let offset = query.offset.min(total);
    let limit = query.limit.max(1);
    let end = offset.saturating_add(limit).min(total);
    let has_more = end < total;
    Ok(OpenApiCapabilityCatalogPage {
        items: summaries[offset..end].to_vec(),
        total,
        offset,
        limit,
        has_more,
        next_offset: has_more.then_some(end),
        adapter_ids,
        methods,
    })
}

pub async fn get_openapi_capability(
    state: &ApiState,
    workspace_id: uuid::Uuid,
    interface_id: &str,
) -> Result<Option<OpenApiCapabilityCatalogEntry>, ApiError> {
    for category in &state.api_docs.catalog().categories {
        let Some(operations) = state.api_docs.category_operations(&category.id) else {
            continue;
        };
        let Some(operation) = operations
            .operations
            .iter()
            .find(|operation| operation.id == interface_id)
        else {
            continue;
        };
        if !static_operation_is_bindable(&operation.path) {
            return Ok(None);
        }
        let Some(spec) = state.api_docs.operation_spec(interface_id) else {
            return Ok(None);
        };
        let Some(interface) = catalog_entry_from_operation(operation, spec) else {
            return Ok(None);
        };
        return Ok(Some(OpenApiCapabilityCatalogEntry {
            risk_level: operation_risk_level(&interface.method),
            interface,
            source: OpenApiCapabilitySource::StaticApiDocs,
            bindable: true,
            disabled_reason: None,
        }));
    }

    let Ok(Some((model_id, kind))) = runtime_data_model_docs::parse_operation_id(interface_id)
    else {
        return Ok(None);
    };
    let Some(model) = state
        .store
        .get_model_definition(workspace_id, model_id)
        .await?
        .filter(|model| model.status == domain::DataModelStatus::Published)
    else {
        return Ok(None);
    };
    let operation =
        runtime_data_model_docs::build_category_operations(std::slice::from_ref(&model))
            .operations
            .into_iter()
            .find(|operation| operation.id == interface_id);
    let Some(operation) = operation else {
        return Ok(None);
    };
    let spec = runtime_data_model_docs::build_operation_openapi(&model, kind);
    let Some(interface) = catalog_entry_from_operation(&operation, &spec) else {
        return Ok(None);
    };
    Ok(Some(OpenApiCapabilityCatalogEntry {
        risk_level: operation_risk_level(&interface.method),
        interface,
        source: OpenApiCapabilitySource::RuntimeDataModelCrud,
        bindable: true,
        disabled_reason: None,
    }))
}

pub async fn get_openapi_capability_by_route(
    state: &ApiState,
    workspace_id: uuid::Uuid,
    method: &str,
    path: &str,
) -> Result<Option<OpenApiCapabilityCatalogEntry>, ApiError> {
    let mut static_match = None;
    for category in &state.api_docs.catalog().categories {
        let Some(operations) = state.api_docs.category_operations(&category.id) else {
            continue;
        };
        let Some(operation) = operations
            .operations
            .iter()
            .find(|operation| operation.method == method && operation.path == path)
        else {
            continue;
        };
        if static_match.is_some() {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "openapi_capability_route",
            )
            .into());
        }
        static_match = Some(operation.clone());
    }
    if let Some(operation) = static_match {
        if !static_operation_is_bindable(&operation.path) {
            return Ok(None);
        }
        let Some(spec) = state.api_docs.operation_spec(&operation.id) else {
            return Ok(None);
        };
        let Some(interface) = catalog_entry_from_operation(&operation, spec) else {
            return Ok(None);
        };
        return Ok(Some(OpenApiCapabilityCatalogEntry {
            risk_level: operation_risk_level(&interface.method),
            interface,
            source: OpenApiCapabilitySource::StaticApiDocs,
            bindable: true,
            disabled_reason: None,
        }));
    }

    if !path.starts_with("/api/runtime/models/") {
        return Ok(None);
    }
    let mut models = state.store.list_model_definitions(workspace_id).await?;
    models.retain(|model| model.status == domain::DataModelStatus::Published);
    let Some(operation) = runtime_data_model_docs::build_category_operations(&models)
        .operations
        .into_iter()
        .find(|operation| operation.method == method && operation.path == path)
    else {
        return Ok(None);
    };
    let Ok(Some((model_id, kind))) = runtime_data_model_docs::parse_operation_id(&operation.id)
    else {
        return Ok(None);
    };
    let Some(model) = models.iter().find(|model| model.id == model_id) else {
        return Ok(None);
    };
    let spec = runtime_data_model_docs::build_operation_openapi(model, kind);
    let Some(interface) = catalog_entry_from_operation(&operation, &spec) else {
        return Ok(None);
    };
    Ok(Some(OpenApiCapabilityCatalogEntry {
        risk_level: operation_risk_level(&interface.method),
        interface,
        source: OpenApiCapabilitySource::RuntimeDataModelCrud,
        bindable: true,
        disabled_reason: None,
    }))
}

async fn openapi_capability_catalog_summaries(
    state: &ApiState,
    workspace_id: uuid::Uuid,
) -> Result<Vec<OpenApiCapabilityCatalogSummary>, ApiError> {
    let mut summaries = Vec::new();
    for category in &state.api_docs.catalog().categories {
        let Some(operations) = state.api_docs.category_operations(&category.id) else {
            continue;
        };
        summaries.extend(
            operations
                .operations
                .iter()
                .filter(|operation| static_operation_is_bindable(&operation.path))
                .map(|operation| OpenApiCapabilityCatalogSummary {
                    interface_id: operation.id.clone(),
                    method: operation.method.clone(),
                    path: operation.path.clone(),
                    source: OpenApiCapabilitySource::StaticApiDocs,
                }),
        );
    }

    let mut models = state.store.list_model_definitions(workspace_id).await?;
    models.retain(|model| model.status == domain::DataModelStatus::Published);
    summaries.extend(
        runtime_data_model_docs::build_category_operations(&models)
            .operations
            .into_iter()
            .map(|operation| OpenApiCapabilityCatalogSummary {
                interface_id: operation.id,
                method: operation.method,
                path: operation.path,
                source: OpenApiCapabilitySource::RuntimeDataModelCrud,
            }),
    );
    summaries.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.method.cmp(&right.method))
            .then_with(|| left.interface_id.cmp(&right.interface_id))
    });
    Ok(summaries)
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
            let bindable = static_operation_is_bindable(&operation.path);
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

fn static_operation_is_bindable(path: &str) -> bool {
    path.starts_with("/api/console/") || path.starts_with(crate::routes::PUBLIC_API_PATH_PREFIX)
}
