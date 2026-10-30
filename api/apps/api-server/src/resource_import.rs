//! API composition adapter for the resource replay + file upload Host Service.
mod exchange_services;
use crate::{app_state::ApiState, error_response::ApiError};
use extension_contracts::{FileImportError, FileImportRequest};
use runtime_core::runtime_backend::{
    FileImportBinding, FileImportFuture, FileImportPort, RuntimeCallerContext,
};
use std::{
    future::Future,
    sync::{Arc, OnceLock, Weak},
};

tokio::task_local! {
    static SELECTED_MCP_INSTANCES: Vec<String>;
    static RESOURCE_IMPORT_REPLAY: bool;
}
pub(crate) fn selected_mcp_scope() -> Option<Vec<String>> {
    SELECTED_MCP_INSTANCES.try_with(Clone::clone).ok()
}
fn importing_from_replay() -> bool {
    RESOURCE_IMPORT_REPLAY
        .try_with(|replay| *replay)
        .unwrap_or(false)
}
pub(crate) async fn with_selected_scope<F: Future>(ids: Vec<String>, future: F) -> F::Output {
    SELECTED_MCP_INSTANCES.scope(ids, future).await
}
pub(crate) async fn with_replay<F: Future>(future: F) -> F::Output {
    RESOURCE_IMPORT_REPLAY.scope(true, future).await
}

/// Called before any worker task is spawned. Task locals are never expected to cross a spawn.
pub(crate) async fn capture_caller(
    state: &ApiState,
    principal: &interface_runtime::UserPrincipal,
    interface_id: &str,
) -> Result<Option<RuntimeCallerContext>, ApiError> {
    if importing_from_replay() {
        return Ok(None);
    }
    let actor = principal.actor().clone();
    let user = state
        .store
        .find_user_by_id(actor.user_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?;
    if matches!(user.status, domain::UserStatus::Disabled) {
        return Err(control_plane::errors::ControlPlaneError::NotAuthenticated.into());
    }
    let session_version = if let Some(delegated) =
        crate::middleware::require_session::server_delegated_request_context()
    {
        if delegated.actor != actor {
            return Err(control_plane::errors::ControlPlaneError::NotAuthenticated.into());
        }
        delegated.user.session_version
    } else if let Some(session) = principal.authenticated_session() {
        state
            .session_store
            .get(session.expose_to_trusted_handler())
            .await?
            .filter(|session| {
                session.user_id == actor.user_id
                    && session.current_workspace_id == actor.current_workspace_id
                    && session.tenant_id == actor.tenant_id
            })
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .session_version
    } else {
        user.session_version
    };
    if session_version != user.session_version {
        return Err(control_plane::errors::ControlPlaneError::NotAuthenticated.into());
    }
    Ok(Some(RuntimeCallerContext {
        actor,
        session_version,
        session_identity: principal.authenticated_session().map(|s| {
            runtime_core::runtime_backend::RuntimeSessionIdentity::new(
                s.expose_to_trusted_handler().to_owned(),
            )
        }),
        interface_id: interface_id.to_owned(),
        selected_mcp_instance_ids: selected_mcp_scope(),
    }))
}

/// Weak publisher binding follows the current generation without retaining old state or routers.
#[derive(Default)]
pub(crate) struct ApiFileImportPort {
    publisher: OnceLock<Weak<crate::managed_publication::ManagedGenerationPublisher>>,
}
impl ApiFileImportPort {
    pub(crate) fn bind(
        &self,
        publisher: &Arc<crate::managed_publication::ManagedGenerationPublisher>,
    ) -> anyhow::Result<()> {
        self.publisher
            .set(Arc::downgrade(publisher))
            .map_err(|_| anyhow::anyhow!("file import host already bound"))
    }
    pub(crate) fn current_generation(&self) -> Option<crate::generation_dispatch::GenerationLease> {
        self.publisher.get()?.upgrade()?.current_generation()
    }
    pub(crate) async fn current_caller(
        &self,
        state: &ApiState,
        binding: &FileImportBinding,
    ) -> Result<(domain::ActorContext, domain::UserRecord), FileImportError> {
        let denied = || FileImportError::new("file_import_caller_invalid");
        let delegated = &binding.caller;
        let user = state
            .store
            .find_user_by_id(delegated.actor.user_id)
            .await
            .map_err(|_| denied())?
            .ok_or_else(denied)?;
        if user.session_version != delegated.session_version
            || matches!(user.status, domain::UserStatus::Disabled)
        {
            return Err(denied());
        }
        if let Some(identity) = &delegated.session_identity {
            let session = state
                .session_store
                .get(identity.expose_to_trusted_handler())
                .await
                .map_err(|_| denied())?
                .ok_or_else(denied)?;
            if !session_matches_caller(
                &session,
                delegated,
                time::OffsetDateTime::now_utc().unix_timestamp(),
            ) {
                return Err(denied());
            }
        }
        let actor = state
            .store
            .load_actor_context(
                user.id,
                delegated.actor.tenant_id,
                delegated.actor.current_workspace_id,
                Some(&delegated.actor.effective_display_role),
            )
            .await
            .map_err(|_| denied())?;
        if actor.effective_display_role != delegated.actor.effective_display_role {
            return Err(denied());
        }
        self.current_operation(state, binding, &actor).await?;
        Ok((actor, user))
    }
    async fn current_operation(
        &self,
        state: &ApiState,
        binding: &FileImportBinding,
        actor: &domain::ActorContext,
    ) -> Result<(), FileImportError> {
        use control_plane_contracts::ports::RoleConsolePolicyReader;
        let denied = || FileImportError::new("file_import_caller_invalid");
        let service = state
            .console_surface_registry
            .managed_services()
            .iter()
            .find(|s| s.installation_id.to_string() == binding.subject.installation_id().as_str())
            .ok_or_else(denied)?;
        let operation = service
            .declaration
            .operations
            .iter()
            .find(|op| {
                op.interface_id == binding.caller.interface_id
                    && op.contribution_id == binding.subject.contribution_id().as_str()
            })
            .ok_or_else(denied)?;
        let policies = state
            .store
            .load_role_console_policies_for_user(actor)
            .await
            .map_err(|_| denied())?;
        let group =
            domain::ConsolePolicyGroup::settings_feature(&service.declaration.feature.feature_id)
                .map_err(|_| denied())?;
        let operation_id = domain::ConsoleOperationId::try_from(operation.interface_id.as_str())
            .map_err(|_| denied())?;
        if !actor.is_root
            && !domain::effective_console_simple_operation(&policies, &group, &operation_id)
        {
            return Err(denied());
        }
        if let Some(ids) = &binding.caller.selected_mcp_instance_ids {
            use crate::routes::mcp_protocol::virtual_ui;
            let mut catalog =
                control_plane::mcp_management::McpManagementService::new(state.store.clone())
                    .read_catalog_for_actor(actor)
                    .await
                    .map_err(|_| denied())?;
            let interfaces =
                crate::routes::mcp_management::mcp_interface_catalog_entries(state, actor)
                    .await
                    .map_err(|_| denied())?;
            let interfaces = virtual_ui::McpInterfaceCatalogSnapshot::new(interfaces);
            crate::managed_service_mcp::append(
                &state.store,
                state.console_surface_registry.managed_services(),
                actor,
                &mut catalog,
                ids,
                &interfaces,
            )
            .await
            .map_err(|_| denied())?;
            let scope = virtual_ui::VirtualMcpScope::from_instance_ids(ids.clone());
            if !catalog.bindings.iter().any(|entry| virtual_ui::visible_tool(&catalog, &scope, &entry.tool_id, false).is_some_and(|(_, tool)| matches!(&tool.execution_target, domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } if interface_id == &operation.interface_id))) {
                return Err(denied());
            }
        }
        Ok(())
    }
}
impl FileImportPort for ApiFileImportPort {
    fn execute<'a>(
        &'a self,
        binding: &'a FileImportBinding,
        request: &'a FileImportRequest,
    ) -> FileImportFuture<'a> {
        Box::pin(async move {
            request.validate()?;
            let state = self
                .current_generation()
                .ok_or_else(|| FileImportError::new("file_import_host_unavailable"))?;
            let (actor, user) = self.current_caller(&state, binding).await?;
            // Check the explicit opt-in before replay, but do not hold the same authority lock
            // while replay re-enters canonical admission. Target effects reacquire the lease.
            let lease =
                control_plane::file_management::authorize_resource_import(&state.store, binding)
                    .await?;
            lease
                .release()
                .await
                .map_err(|_| FileImportError::new("file_import_authority_unavailable"))?;
            let source = crate::routes::mcp_protocol::resources::replay_for_import(
                &state,
                binding,
                &actor,
                user,
                &request.resource_uri,
            )
            .await?;
            let (actor, _) = self.current_caller(&state, binding).await?;
            let upload = control_plane::file_management::FileUploadService::new(
                state.store.clone(),
                state.file_storage_registry.clone(),
                state.runtime_engine.clone(),
            );
            control_plane::file_management::ResourceImportService::new(
                upload,
                Arc::new(state.store.clone()),
            )
            .import(
                binding,
                actor,
                uuid::Uuid::parse_str(&request.file_table_id)
                    .map_err(|_| FileImportError::new("file_import_request_invalid"))?,
                source,
            )
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn trusted_scope_capture_distinguishes_direct_console_and_empty_selection_and_blocks_recursive_import(
    ) {
        assert!(selected_mcp_scope().is_none());
        with_selected_scope(vec![], async {
            assert_eq!(selected_mcp_scope(), Some(vec![]));
        })
        .await;
        with_selected_scope(vec!["selected".into()], async {
            let captured = selected_mcp_scope();
            assert_eq!(captured, Some(vec!["selected".into()]));
            assert!(!importing_from_replay());
            with_replay(async {
                assert!(importing_from_replay());
                with_replay(async {
                    assert!(importing_from_replay());
                })
                .await;
                assert!(importing_from_replay());
            })
            .await;
            assert!(!importing_from_replay());
            // Spawned callbacks must receive the captured Host value, not guess from their URI.
            tokio::spawn(async move {
                assert_eq!(captured, Some(vec!["selected".into()]));
                assert!(selected_mcp_scope().is_none());
            })
            .await
            .unwrap();
        })
        .await;
        assert!(selected_mcp_scope().is_none());
    }
}

impl runtime_core::runtime_backend::FileReadPort for ApiFileImportPort {
    fn authorize<'a>(
        &'a self,
        binding: &'a runtime_core::runtime_backend::FileReadBinding,
    ) -> runtime_core::runtime_backend::FileReadFuture<'a, ()> {
        Box::pin(async move {
            let state = self.current_generation().ok_or_else(|| {
                extension_contracts::FileReadError::new("file_read_host_unavailable")
            })?;
            self.current_caller(&state, binding)
                .await
                .map_err(|_| extension_contracts::FileReadError::new("file_read_caller_invalid"))?;
            let lease = control_plane::file_management::authorize_file_service(
                &state.store,
                binding,
                "file.read.content",
            )
            .await
            .map_err(|_| extension_contracts::FileReadError::new("file_read_authority_denied"))?;
            lease.release().await.map_err(|_| {
                extension_contracts::FileReadError::new("file_read_authority_unavailable")
            })?;
            Ok(())
        })
    }
    fn open<'a>(
        &'a self,
        binding: &'a runtime_core::runtime_backend::FileReadBinding,
        file_table_id: &'a str,
        record_id: &'a str,
    ) -> runtime_core::runtime_backend::FileReadFuture<
        'a,
        runtime_core::runtime_backend::FileReadSource,
    > {
        Box::pin(async move {
            use extension_contracts::FileReadError;
            let denied = || FileReadError::new("file_read_permission_denied");
            let state = self
                .current_generation()
                .ok_or_else(|| FileReadError::new("file_read_host_unavailable"))?;
            let (actor, _) = self
                .current_caller(&state, binding)
                .await
                .map_err(|_| FileReadError::new("file_read_caller_invalid"))?;
            let lease = control_plane::file_management::authorize_file_service(
                &state.store,
                binding,
                "file.read.content",
            )
            .await
            .map_err(|_| FileReadError::new("file_read_authority_denied"))?;
            let content = control_plane::file_management::FileAccessService::new(
                state.store.clone(),
                state.file_storage_registry.clone(),
                state.runtime_engine.clone(),
                state.infrastructure.ephemeral_kv_store(),
            )
            .read(
                actor,
                uuid::Uuid::parse_str(file_table_id).map_err(|_| denied())?,
                uuid::Uuid::parse_str(record_id).map_err(|_| denied())?,
            )
            .await
            .map_err(|_| denied())?;
            lease
                .release()
                .await
                .map_err(|_| FileReadError::new("file_read_authority_unavailable"))?;
            Ok(runtime_core::runtime_backend::FileReadSource {
                metadata: extension_contracts::FileReadMetadata {
                    filename: content.filename,
                    content_type: Some(content.content_type),
                    content_length: content.content_length,
                },
                reader: content
                    .reader
                    .into_inner()
                    .map_err(|_| FileReadError::new("file_read_io_failed"))?,
            })
        })
    }
}

fn session_matches_caller(
    session: &domain::SessionRecord,
    caller: &RuntimeCallerContext,
    now: i64,
) -> bool {
    session.user_id == caller.actor.user_id
        && session.tenant_id == caller.actor.tenant_id
        && session.current_workspace_id == caller.actor.current_workspace_id
        && session.session_version == caller.session_version
        && session.active_role_code == caller.actor.effective_display_role
        && session.expires_at_unix > now
}

#[cfg(test)]
mod file_read_identity_tests {
    use super::*;
    #[test]
    fn read_and_import_session_currentness_rejects_controlled_identity_changes_and_redacts_handle()
    {
        let actor: domain::ActorContext=serde_json::from_value(serde_json::json!({"user_id":"00000000-0000-0000-0000-000000000001","tenant_id":"00000000-0000-0000-0000-000000000000","current_workspace_id":"00000000-0000-0000-0000-000000000002","effective_display_role":"root","is_root":true,"permissions":[]})).unwrap();
        let caller = RuntimeCallerContext {
            actor: actor.clone(),
            session_version: 7,
            session_identity: Some(runtime_core::runtime_backend::RuntimeSessionIdentity::new(
                "private-session".into(),
            )),
            interface_id: "ssh.download".into(),
            selected_mcp_instance_ids: None,
        };
        let session = domain::SessionRecord {
            session_id: "private-session".into(),
            user_id: actor.user_id,
            tenant_id: actor.tenant_id,
            current_workspace_id: actor.current_workspace_id,
            active_role_code: actor.effective_display_role,
            session_version: 7,
            csrf_token: "never-forwarded".into(),
            expires_at_unix: 200,
        };
        assert!(session_matches_caller(&session, &caller, 100));
        assert!(!format!("{caller:?}").contains("private-session"));
        for case in 0..5 {
            let mut invalid = session.clone();
            match case {
                0 => invalid.session_version += 1,
                1 => invalid.user_id = uuid::Uuid::now_v7(),
                2 => invalid.current_workspace_id = uuid::Uuid::now_v7(),
                3 => invalid.active_role_code = "other".into(),
                _ => invalid.expires_at_unix = 100,
            }
            assert!(
                !session_matches_caller(&invalid, &caller, 100),
                "case {case}"
            );
        }
    }
}
