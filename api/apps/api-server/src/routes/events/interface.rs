use crate::{
    app_state::ApiState,
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};
use control_plane::event_center::{EventCenterConfigurationService, EventCenterRuntime};
use domain::event_center::{
    CreateEventDefinitionInput, EventCatalogEntry, EventCatalogQuery, EventDefinitionRecord,
    UpdateEventDefinitionInput,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::{json, Value};
use std::sync::{Arc, Weak};
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct Dependencies {
    store: storage_durable_postgres::MainDurableStore,
    runtime: EventCenterRuntime,
    // The compiled registry owns this handler; retaining Boot here would close a cycle.
    boot: Option<Weak<crate::extension_bus::ExtensionBootSnapshot>>,
    changes: Arc<tokio::sync::Notify>,
    statuses: crate::workers::event_workflow::EventWorkflowStatuses,
    sources: Arc<crate::event_center::catalog::EventSourceCatalog>,
    api_catalog: crate::openapi_interface::OpenApiCapabilityCatalogDependencies,
}
impl Dependencies {
    pub(crate) fn new(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            runtime: state.event_center.clone(),
            boot: state.extension_boot_snapshot.as_ref().map(Arc::downgrade),
            changes: state.event_workflow_changes.clone(),
            statuses: state.event_workflow_statuses.clone(),
            sources: state.event_sources.clone(),
            api_catalog: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                store: state.store.clone(),
                console_operations: state.console_operation_registry.inventory().clone(),
                interface_registry: None,
                api_docs: state.api_docs.clone(),
                template_catalog: state.runtime_engine.template_catalog().clone(),
            },
        }
    }
}
pub(crate) enum Input {
    Catalog(EventCatalogQuery),
    ApiOperations,
    ApiOperation(String),
    Create(CreateEventDefinitionInput),
    Update(Uuid, UpdateEventDefinitionInput),
    Runtime,
}
pub(crate) struct Output(pub Value);
impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "console-events-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(crate::extension_bus::managed_projection::object_schema([(
            "operation",
            crate::extension_bus::managed_projection::text_schema(),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(
            json!({"operation": match self { Self::Catalog(_)=>"catalog", Self::ApiOperations=>"api_operations", Self::ApiOperation(_)=>"api_operation", Self::Create(_)=>"create",Self::Update(..)=>"update",Self::Runtime=>"runtime" }}),
        )
    }
}
impl InterfaceContract for Output {
    const CONTRACT_ID: &'static str = "console-events-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(crate::extension_bus::managed_projection::json_summary_schema())
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(crate::extension_bus::managed_projection::json_summary(
            &self.0,
        ))
    }
}
impl ConsoleInterfacePort<Input, Output> for Dependencies {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: Input,
    ) -> ConsoleInterfaceFuture<'a, Output> {
        Box::pin(async move {
            let result: Result<Value, ApiError> = async {
                let actor = principal.actor();
                // Registry descendants borrow their own generation; an expired binding
                // must not silently become an unbound or newer source catalog.
                let _generation = self
                    .boot
                    .as_ref()
                    .map(|boot| {
                        boot.upgrade()
                            .ok_or_else(crate::generation_dispatch::unavailable)
                    })
                    .transpose()?;
                let registry = _generation
                    .as_ref()
                    .and_then(|boot| boot.interface_registry())
                    .map(|registry| registry.snapshot());
                let external_catalog = _generation
                    .as_ref()
                    .and_then(|boot| boot.external_endpoint_catalog())
                    .map(Arc::as_ref);
                let entries = if matches!(&input, Input::Create(_) | Input::Update(..)) {
                    self.sources.registered_events_with_interfaces(
                        actor,
                        &self.runtime,
                        registry.as_deref(),
                    )
                } else {
                    Vec::new()
                };
                let mut api_catalog = self.api_catalog.clone();
                api_catalog.interface_registry = registry.clone();
                let mut service = EventCenterConfigurationService::with_registered_events(
                    self.store.clone(),
                    actor.current_workspace_id,
                    entries,
                );
                let stopping_unchanged_rule = if let Input::Update(id, value) = &input {
                    !value.enabled
                        && service
                            .get_definition(actor, *id)
                            .await?
                            .interface_event
                            .as_ref()
                            == Some(&value.interface_event)
                } else {
                    false
                };
                let selected_operation = match &input {
                    Input::Create(value) => value.interface_event.operation_id(),
                    Input::Update(_, value) if !stopping_unchanged_rule => {
                        value.interface_event.operation_id()
                    }
                    _ => None,
                };
                if let Some(operation_id) = selected_operation {
                    let selected = crate::event_center::api_catalog::get_with(
                        &api_catalog,
                        actor,
                        operation_id,
                        external_catalog,
                    )
                    .await?
                    .into_iter()
                    .collect();
                    service = service.with_api_operations(actor.current_workspace_id, selected);
                }
                match input {
                    Input::ApiOperations => Ok(serde_json::to_value(
                        crate::event_center::api_catalog::summaries_with(
                            &api_catalog,
                            actor,
                            external_catalog,
                        )
                        .await?,
                    )?),
                    Input::ApiOperation(operation_id) => {
                        let details = crate::event_center::api_catalog::get_with(
                            &api_catalog,
                            actor,
                            &operation_id,
                            external_catalog,
                        )
                        .await?
                        .ok_or(
                            control_plane::errors::ControlPlaneError::NotFound(
                                "api_event_operation",
                            ),
                        )?;
                        Ok(serde_json::to_value(details)?)
                    }
                    Input::Catalog(query) => {
                        catalog_value(service.catalog_with_query(actor, query).await?)
                    }
                    Input::Create(input) => {
                        definition_value(service.create_definition(actor, input).await?)
                    }
                    Input::Update(id, input) => {
                        let record = service.update_definition(actor, id, input).await?;
                        self.changes.notify_one();
                        definition_value(record)
                    }
                    Input::Runtime => {
                        use control_plane_contracts::ports::LifecycleOutboxRepository;
                        let source_queue_refresh_error =
                            if let Some(observer) = &self.sources.lifecycle_observer {
                                match self.store.lifecycle_source_queue_snapshots().await {
                                    Ok(facts) => {
                                        observer.refresh_queue_snapshots(facts);
                                        None
                                    }
                                    Err(_) => Some("source_queue_refresh_unavailable".to_owned()),
                                }
                            } else {
                                Some("source_queue_observer_unavailable".to_owned())
                            };
                        let source_views = crate::event_center::catalog::visible_scopes(actor)
                            .into_iter()
                            .map(|scope| self.runtime.snapshot_sources(scope))
                            .collect();
                        Ok(serde_json::to_value(RuntimeResponse {
                            snapshot: self.runtime.snapshot(actor.current_workspace_id),
                            source_views,
                            source_queue_refresh_error,
                            workflow_subscriptions: self
                                .statuses
                                .snapshot(actor.current_workspace_id),
                            workflow_refresh_error: self.statuses.refresh_error(),
                        })?)
                    }
                }
            }
            .await;
            result.map(Output).map_err(ConsoleInterfaceTargetError)
        })
    }
}
#[derive(serde::Serialize)]
struct RuntimeResponse {
    source_views: Vec<control_plane::event_center::EventSourceRuntimeSnapshot>,
    source_queue_refresh_error: Option<String>,
    #[serde(flatten)]
    snapshot: control_plane::event_center::EventRuntimeSnapshot,
    workflow_subscriptions: Vec<crate::workers::event_workflow::EventWorkflowStatus>,
    workflow_refresh_error: Option<String>,
}
fn catalog_value(entries: Vec<EventCatalogEntry>) -> Result<Value, ApiError> {
    entries
        .into_iter()
        .map(|entry| {
            let timestamps = [
                ("created_at", entry.created_at),
                ("updated_at", entry.updated_at),
            ];
            let mut value = serde_json::to_value(entry)?;
            for (field, timestamp) in timestamps {
                value[field] = match timestamp {
                    Some(timestamp) => Value::String(
                        timestamp
                            .format(&time::format_description::well_known::Rfc3339)
                            .map_err(anyhow::Error::from)?,
                    ),
                    None => Value::Null,
                };
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, ApiError>>()
        .map(Value::Array)
}

fn definition_value(record: EventDefinitionRecord) -> Result<Value, ApiError> {
    let created = record
        .created_at
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(anyhow::Error::from)?;
    let updated = record
        .updated_at
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(anyhow::Error::from)?;
    let mut value = serde_json::to_value(record)?;
    value["created_at"] = Value::String(created);
    value["updated_at"] = Value::String(updated);
    Ok(value)
}
pub(crate) fn compile_registry(
    dependencies: Dependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-events",
        "graph:console-events-v1",
        DECLARATIONS,
        Arc::new(dependencies),
    )
}
pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "events.api_operations.list",
        binding_id: "http.console.events.api_operations.list.v1",
        method: "GET",
        path: "/api/console/events/api-operations",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "events.api_operations.get",
        binding_id: "http.console.events.api_operations.get.v1",
        method: "GET",
        path: "/api/console/events/api-operations/:operation_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "events.catalog.list",
        binding_id: "http.console.events.catalog.list.v1",
        method: "GET",
        path: "/api/console/events",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "events.definitions.create",
        binding_id: "http.console.events.definitions.create.v1",
        method: "POST",
        path: "/api/console/events",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "events.definitions.update",
        binding_id: "http.console.events.definitions.update.v1",
        method: "PUT",
        path: "/api/console/events/:event_definition_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "events.runtime.get",
        binding_id: "http.console.events.runtime.get.v1",
        method: "GET",
        path: "/api/console/events/runtime",
        mutating: false,
    },
];

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/interface.rs"]
mod tests;

impl crate::openapi_interface::contract::ApiInput for Input {
    fn project_document_input(
        &self,
        _: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<Value, Arc<str>> {
        use crate::openapi_interface::contract::to_value;
        Ok(match self {
            Self::Catalog(query) => json!({"query":to_value(query)?}),
            Self::Runtime | Self::ApiOperations => json!({}),
            Self::ApiOperation(operation_id) => json!({"path":{"operation_id":operation_id}}),
            Self::Create(input) => json!({"body":to_value(input)?}),
            Self::Update(id, input) => {
                json!({"path":{"event_definition_id":id},"body":to_value(input)?})
            }
        })
    }
}
impl crate::openapi_interface::contract::ApiOutput for Output {
    fn project_document_output(
        &self,
        _: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<Value, Arc<str>> {
        Ok(self.0.clone())
    }
}
