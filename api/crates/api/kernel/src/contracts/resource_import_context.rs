//! Host-owned caller capture and task-local scopes shared by MCP and managed calls.
use crate::{app_state::ApiState, error_response::ApiError};
use runtime_core::runtime_backend::RuntimeCallerContext;
use std::future::Future;
tokio::task_local! {
    static SELECTED_MCP_INSTANCES: Vec<String>;
    static RESOURCE_IMPORT_REPLAY: bool;
}
pub fn selected_mcp_scope() -> Option<Vec<String>> {
    SELECTED_MCP_INSTANCES.try_with(Clone::clone).ok()
}
pub fn importing_from_replay() -> bool {
    RESOURCE_IMPORT_REPLAY
        .try_with(|replay| *replay)
        .unwrap_or(false)
}
pub async fn with_selected_scope<F: Future>(ids: Vec<String>, future: F) -> F::Output {
    SELECTED_MCP_INSTANCES.scope(ids, future).await
}
pub async fn with_replay<F: Future>(future: F) -> F::Output {
    RESOURCE_IMPORT_REPLAY.scope(true, future).await
}

/// Called before any worker task is spawned. Task locals are never expected to cross a spawn.
pub async fn capture_caller(
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
