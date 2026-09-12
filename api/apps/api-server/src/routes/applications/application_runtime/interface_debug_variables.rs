use std::sync::Arc;

use control_plane::{
    application::ApplicationService,
    errors::ControlPlaneError,
    flow::FlowService,
    ports::{
        DebugVariableCacheKey, DeleteDebugVariableCacheEntriesInput,
        OrchestrationRuntimeRepository, UpsertDebugVariableCacheEntryInput,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    debug_variable_cache::{
        DeleteDebugVariableCacheEntriesBody, UpsertDebugVariableCacheEntryBody,
    },
    debug_variable_snapshot::{build_debug_variable_snapshot, DebugVariableSnapshotResponse},
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum ApplicationRuntimeDebugVariablesInput {
    Snapshot {
        application_id: Uuid,
    },
    Upsert {
        application_id: Uuid,
        body: UpsertDebugVariableCacheEntryBody,
    },
    Delete {
        application_id: Uuid,
        body: DeleteDebugVariableCacheEntriesBody,
    },
}

impl InterfaceContract for ApplicationRuntimeDebugVariablesInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Snapshot")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Upsert")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("node_id", mp::text_schema()),
                        (
                            "variable_key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("value", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "keys",
                        serde_json::json!({"anyOf": [serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("node_id",mp::text_schema()), ("variable_key",mp::object_schema(&[("byte_count",mp::count_schema())]))])}), {"type":"null"}]}),
                    )]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Snapshot {
                application_id: _field_application_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Snapshot".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
            ]),
            Self::Upsert {
                application_id: _field_application_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Upsert".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        ("node_id", mp::text(&(_field_body).node_id)?),
                        (
                            "variable_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).variable_key).len()),
                            )]),
                        ),
                        ("value", mp::json_summary(&(_field_body).value)),
                    ]),
                ),
            ]),
            Self::Delete {
                application_id: _field_application_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Delete".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[(
                        "keys",
                        match (&(_field_body).keys).as_ref() {
                            Some(item) => {
                                if (item).len() > 32 {
                                    return None;
                                }
                                serde_json::Value::Array(
                                    (item)
                                        .iter()
                                        .map(|item| {
                                            Some(mp::object_value(&[
                                                ("node_id", mp::text(&(item).node_id)?),
                                                (
                                                    "variable_key",
                                                    mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!(
                                                            (&(item).variable_key).len()
                                                        ),
                                                    )]),
                                                ),
                                            ]))
                                        })
                                        .collect::<Option<Vec<_>>>()?,
                                )
                            }
                            None => serde_json::Value::Null,
                        },
                    )]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-variables-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed debug output is projected immediately into the console response"
)]
pub(crate) enum ApplicationRuntimeDebugVariablesOutput {
    Snapshot(DebugVariableSnapshotResponse),
    Updated,
}

impl InterfaceContract for ApplicationRuntimeDebugVariablesOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Snapshot")),
                (
                    "0",
                    mp::object_schema(&[
                        ("snapshot_schema_version", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        ("actor_user_id", mp::text_schema()),
                        ("draft_id", mp::text_schema()),
                        ("flow_schema_version", mp::text_schema()),
                        (
                            "document_hash",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "latest_run_scope",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("flow_run_id",mp::text_schema()), ("run_mode",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("target_node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))]), {"type":"null"}]}),
                        ),
                        (
                            "snapshot_completeness",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("source_flow_run_ids", mp::json_summary_schema()),
                        ("source_node_run_ids", mp::json_summary_schema()),
                        ("variable_cache", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Updated"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Snapshot(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Snapshot".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "snapshot_schema_version",
                            mp::text(&(_field_0).snapshot_schema_version)?,
                        ),
                        ("workspace_id", mp::text(&(_field_0).workspace_id)?),
                        ("actor_user_id", mp::text(&(_field_0).actor_user_id)?),
                        ("draft_id", mp::text(&(_field_0).draft_id)?),
                        (
                            "flow_schema_version",
                            mp::text(&(_field_0).flow_schema_version)?,
                        ),
                        (
                            "document_hash",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).document_hash).len()),
                            )]),
                        ),
                        (
                            "latest_run_scope",
                            match (&(_field_0).latest_run_scope).as_ref() {
                                Some(item) => mp::object_value(&[
                                    ("flow_run_id", mp::text(&(item).flow_run_id)?),
                                    (
                                        "run_mode",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).run_mode).len()),
                                        )]),
                                    ),
                                    ("status", mp::text(&(item).status)?),
                                    (
                                        "target_node_id",
                                        match (&(item).target_node_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "snapshot_completeness",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).snapshot_completeness).len()),
                            )]),
                        ),
                        (
                            "source_flow_run_ids",
                            mp::json_summary(&(_field_0).source_flow_run_ids),
                        ),
                        (
                            "source_node_run_ids",
                            mp::json_summary(&(_field_0).source_node_run_ids),
                        ),
                        (
                            "variable_cache",
                            mp::json_summary(&(_field_0).variable_cache),
                        ),
                    ]),
                ),
            ]),
            Self::Updated => {
                mp::object_value(&[("variant", serde_json::Value::String("Updated".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-variables-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationRuntimeDebugVariablesAdapter {
    store: MainDurableStore,
}

impl ApplicationRuntimeDebugVariablesAdapter {
    async fn visible_application(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
    ) -> Result<(), ApiError> {
        ApplicationService::new(self.store.for_actor(actor.clone()))
            .get_application(actor.user_id, application_id)
            .await?;
        Ok(())
    }

    async fn editor_state(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
    ) -> Result<domain::FlowEditorState, ApiError> {
        Ok(FlowService::new(self.store.for_actor(actor.clone()))
            .get_or_create_editor_state(actor.user_id, application_id)
            .await?)
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationRuntimeDebugVariablesInput,
    ) -> Result<ApplicationRuntimeDebugVariablesOutput, ApiError> {
        let actor = principal.actor();
        match input {
            ApplicationRuntimeDebugVariablesInput::Snapshot { application_id } => {
                self.visible_application(actor, application_id).await?;
                let editor_state = self.editor_state(actor, application_id).await?;
                Ok(ApplicationRuntimeDebugVariablesOutput::Snapshot(
                    build_debug_variable_snapshot(
                        &self.store,
                        application_id,
                        actor.current_workspace_id,
                        actor.user_id,
                        &editor_state,
                    )
                    .await?,
                ))
            }
            ApplicationRuntimeDebugVariablesInput::Upsert {
                application_id,
                body,
            } => {
                self.visible_application(actor, application_id).await?;
                let editor_state = self.editor_state(actor, application_id).await?;
                let node_id = body.node_id.trim().to_string();
                let variable_key = body.variable_key.trim().to_string();
                if node_id.is_empty() || variable_key.is_empty() {
                    return Err(ControlPlaneError::InvalidInput("debug_variable_cache_key").into());
                }
                <_ as OrchestrationRuntimeRepository>::upsert_debug_variable_cache_entry(
                    &self.store,
                    &UpsertDebugVariableCacheEntryInput {
                        workspace_id: actor.current_workspace_id,
                        application_id,
                        draft_id: editor_state.draft.id,
                        actor_user_id: actor.user_id,
                        node_id,
                        variable_key,
                        value: body.value,
                    },
                )
                .await?;
                Ok(ApplicationRuntimeDebugVariablesOutput::Updated)
            }
            ApplicationRuntimeDebugVariablesInput::Delete {
                application_id,
                body,
            } => {
                self.visible_application(actor, application_id).await?;
                let editor_state = self.editor_state(actor, application_id).await?;
                let keys = body.keys.map(|keys| {
                    keys.into_iter()
                        .filter_map(|key| {
                            let node_id = key.node_id.trim().to_string();
                            let variable_key = key.variable_key.trim().to_string();
                            (!node_id.is_empty() && !variable_key.is_empty()).then_some(
                                DebugVariableCacheKey {
                                    node_id,
                                    variable_key,
                                },
                            )
                        })
                        .collect()
                });
                <_ as OrchestrationRuntimeRepository>::delete_debug_variable_cache_entries(
                    &self.store,
                    &DeleteDebugVariableCacheEntriesInput {
                        application_id,
                        draft_id: editor_state.draft.id,
                        actor_user_id: actor.user_id,
                        keys,
                    },
                )
                .await?;
                Ok(ApplicationRuntimeDebugVariablesOutput::Updated)
            }
        }
    }
}

impl
    ConsoleInterfacePort<
        ApplicationRuntimeDebugVariablesInput,
        ApplicationRuntimeDebugVariablesOutput,
    > for ApplicationRuntimeDebugVariablesAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationRuntimeDebugVariablesInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeDebugVariablesOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-variables.snapshot.get",
        binding_id: "http.console.applications.runtime.debug-variables.snapshot.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/orchestration/debug-variable-snapshot",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-variables.cache.upsert",
        binding_id: "http.console.applications.runtime.debug-variables.cache.upsert.v1",
        method: "PUT",
        path: "/api/console/applications/:id/orchestration/debug-variable-cache",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-variables.cache.delete",
        binding_id: "http.console.applications.runtime.debug-variables.cache.delete.v1",
        method: "DELETE",
        path: "/api/console/applications/:id/orchestration/debug-variable-cache",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-runtime-debug-variables",
        "graph:console-application-runtime-debug-variables-v1",
        DECLARATIONS,
        Arc::new(ApplicationRuntimeDebugVariablesAdapter { store }),
    )
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f09r3_registry_freezes_debug_variable_bindings() {
        struct Unavailable;
        impl
            ConsoleInterfacePort<
                ApplicationRuntimeDebugVariablesInput,
                ApplicationRuntimeDebugVariablesOutput,
            > for Unavailable
        {
            fn execute<'a>(
                &'a self,
                _principal: &'a UserPrincipal,
                _input: ApplicationRuntimeDebugVariablesInput,
            ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeDebugVariablesOutput> {
                Box::pin(async {
                    Err(ConsoleInterfaceTargetError(
                        anyhow::anyhow!("fixture unavailable").into(),
                    ))
                })
            }
        }

        let registry = console_interface::compile_registry(
            "api-server.console-application-runtime-debug-variables",
            "graph:console-application-runtime-debug-variables-v1",
            DECLARATIONS,
            Arc::new(Unavailable),
        )
        .unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
