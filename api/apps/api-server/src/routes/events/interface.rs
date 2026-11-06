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
    CreateEventDefinitionInput, EventDefinitionRecord, UpdateEventDefinitionInput,
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
}
impl Dependencies {
    pub(crate) fn new(state: &ApiState) -> Self {
        Self {
            store: state.store.clone(),
            runtime: state.event_center.clone(),
            boot: state.extension_boot_snapshot.as_ref().map(Arc::downgrade),
            changes: state.event_workflow_changes.clone(),
            statuses: state.event_workflow_statuses.clone(),
        }
    }
}
pub(crate) enum Input {
    Catalog,
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
            json!({"operation": match self { Self::Catalog=>"catalog", Self::Create(_)=>"create",Self::Update(..)=>"update",Self::Runtime=>"runtime" }}),
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
                let service = EventCenterConfigurationService::new(self.store.clone());
                match input {
                    Input::Catalog => {
                        // None is an intentionally unbound catalog. An expired generation must
                        // fail, rather than silently substituting an empty or newer catalog.
                        let boot = self
                            .boot
                            .as_ref()
                            .map(|boot| {
                                boot.upgrade()
                                    .ok_or_else(crate::generation_dispatch::unavailable)
                            })
                            .transpose()?;
                        let entries =
                            crate::event_center::catalog::registered_events(boot.as_deref(), actor);
                        let service = EventCenterConfigurationService::with_registered_events(
                            self.store.clone(),
                            actor.current_workspace_id,
                            entries,
                        );
                        Ok(serde_json::to_value(service.catalog(actor).await?)?)
                    }
                    Input::Create(input) => {
                        definition_value(service.create_definition(actor, input).await?)
                    }
                    Input::Update(id, input) => {
                        let record = service.update_definition(actor, id, input).await?;
                        self.changes.notify_one();
                        definition_value(record)
                    }
                    Input::Runtime => Ok(serde_json::to_value(RuntimeResponse {
                        snapshot: self.runtime.snapshot(actor.current_workspace_id),
                        workflow_subscriptions: self.statuses.snapshot(actor.current_workspace_id),
                        workflow_refresh_error: self.statuses.refresh_error(),
                    })?),
                }
            }
            .await;
            result.map(Output).map_err(ConsoleInterfaceTargetError)
        })
    }
}
#[derive(serde::Serialize)]
struct RuntimeResponse {
    #[serde(flatten)]
    snapshot: control_plane::event_center::EventRuntimeSnapshot,
    workflow_subscriptions: Vec<crate::workers::event_workflow::EventWorkflowStatus>,
    workflow_refresh_error: Option<String>,
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
