use super::super::super::{
    ApiMcpRuntimeToolInvoker, ConsoleRouterMcpInterfaceDispatchPort, McpInterfaceCatalogSnapshot,
    RuntimeInternalToolInvokerDependencies, RuntimeInternalToolInvokerFactory,
};
use super::*;
use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    test_api_state_with_database_url,
};
use crate::middleware::require_session::require_session;
use control_plane_contracts::ports::{
    CreateApplicationInput, CreateFlowRunShellInput, OrchestrationRuntimeRepository,
    UpdateFlowRunInput,
};
use std::sync::Arc;

async fn shell(
    state: &crate::app_state::ApiState,
    application: &domain::ApplicationRecord,
    executor: Uuid,
) -> Uuid {
    let editor = state
        .store
        .get_or_create_editor_state(
            application.workspace_id,
            application.id,
            application.created_by,
        )
        .await
        .unwrap();
    state
        .store
        .create_flow_run_shell(&CreateFlowRunShellInput {
            actor_user_id: executor,
            application_id: application.id,
            flow_id: editor.flow.id,
            flow_draft_id: editor.draft.id,
            debug_session_id: Uuid::now_v7().to_string(),
            flow_schema_version: "1".into(),
            document_hash: "workflow-credential-fixture".into(),
            run_mode: domain::FlowRunMode::DebugFlowRun,
            target_node_id: None,
            title: "Credential fixture".into(),
            status: domain::FlowRunStatus::Queued,
            input_payload: json!({}),
            started_at: time::OffsetDateTime::now_utc(),
            api_key_id: None,
            publication_version_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn host_workflow_credentials_preserve_actual_role_revalidate_and_reject_replay() {
    let (state, _) = test_api_state_with_database_url().await;
    let router = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&router, "root", "change-me").await;
    let mut headers = HeaderMap::new();
    headers.insert("cookie", cookie.parse().unwrap());
    let root = require_session(&state, &headers).await.unwrap();
    let member = create_member(
        &router,
        &cookie,
        &csrf,
        "workflow-credential-executor",
        "temp-pass",
    )
    .await;
    for role in ["workflow_cred_default", "workflow_cred_selected"] {
        create_role(&router, &cookie, &csrf, role).await;
    }
    replace_member_roles(
        &router,
        &cookie,
        &csrf,
        &member,
        &["workflow_cred_default", "workflow_cred_selected"],
    )
    .await;
    let user_id: Uuid = member.parse().unwrap();
    let actor = state
        .store
        .load_actor_context(
            user_id,
            root.actor.tenant_id,
            root.actor.current_workspace_id,
            Some("workflow_cred_selected"),
        )
        .await
        .unwrap();
    let application = state
        .store
        .create_application(&CreateApplicationInput {
            actor_user_id: root.user.id,
            workspace_id: root.actor.current_workspace_id,
            application_type: domain::ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Credential identities".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let run = shell(&state, &application, user_id).await;
    let other_run = shell(&state, &application, user_id).await;
    let factory = RuntimeInternalToolInvokerFactory::new(
        RuntimeInternalToolInvokerDependencies::new(
            state.store.clone(),
            state.infrastructure.cache_store(),
            state.provider_secret_master_key.clone(),
        ),
        Arc::new(McpInterfaceCatalogSnapshot::new(vec![])),
        Arc::new(ConsoleRouterMcpInterfaceDispatchPort::new(router.clone())),
    );
    let invoker =
        ApiMcpRuntimeToolInvoker::new(factory.clone(), HeaderMap::new(), actor.clone(), vec![])
            .await
            .unwrap();
    let refs = invoker.prepare_workflow_credentials(run).await.unwrap();
    let snapshot = state
        .store
        .get_workflow_run_identity(run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        snapshot.executor.as_ref().unwrap().active_role_code,
        "workflow_cred_selected"
    );
    assert_eq!(
        snapshot.executor.as_ref().unwrap().workspace_id,
        actor.current_workspace_id
    );
    assert_eq!(
        snapshot.executor.as_ref().unwrap().tenant_id,
        actor.tenant_id
    );
    assert_eq!(snapshot.creator.as_ref().unwrap().user_id, root.user.id);
    assert_ne!(
        snapshot.creator.as_ref().unwrap().active_role_code,
        snapshot.executor.as_ref().unwrap().active_role_code
    );
    // Repeated preparation from a new invoker with the default role cannot replace the frozen role.
    let default_actor = state
        .store
        .load_actor_context(
            user_id,
            actor.tenant_id,
            actor.current_workspace_id,
            Some("workflow_cred_default"),
        )
        .await
        .unwrap();
    let restored =
        ApiMcpRuntimeToolInvoker::new(factory.clone(), HeaderMap::new(), default_actor, vec![])
            .await
            .unwrap();
    assert_eq!(
        restored.prepare_workflow_credentials(run).await.unwrap(),
        refs
    );
    assert_eq!(
        state
            .store
            .get_workflow_run_identity(run)
            .await
            .unwrap()
            .unwrap(),
        snapshot
    );
    let mut attempted_overwrite = snapshot.clone();
    attempted_overwrite
        .executor
        .as_mut()
        .unwrap()
        .active_role_code = "workflow_cred_default".into();
    assert_eq!(
        state
            .store
            .freeze_workflow_run_identity(&attempted_overwrite)
            .await
            .unwrap(),
        snapshot
    );
    let mut wrong_workspace = invoker.clone();
    let super::super::super::RuntimeMcpAuthorization::ServerDelegation(delegation) =
        &mut wrong_workspace.authorization;
    delegation.workspace_id = Uuid::now_v7();
    assert_eq!(
        wrong_workspace
            .prepare_workflow_credentials(run)
            .await
            .unwrap_err()
            .to_string(),
        "workflow_credential_run_binding_mismatch"
    );
    // Non-user application authority never becomes its owner's executor identity. The author
    // can still deliberately select their independent creator identity in the workflow.
    let public_run = shell(&state, &application, root.user.id).await;
    let mut public_actor = root.actor.clone();
    public_actor.effective_display_role = "application_api_key".into();
    public_actor.is_root = false;
    public_actor.permissions.clear();
    let public_invoker =
        ApiMcpRuntimeToolInvoker::new(factory.clone(), HeaderMap::new(), public_actor, vec![])
            .await
            .unwrap();
    let public_refs = public_invoker
        .prepare_workflow_credentials(public_run)
        .await
        .unwrap();
    let public_snapshot = state
        .store
        .get_workflow_run_identity(public_run)
        .await
        .unwrap()
        .unwrap();
    assert!(public_snapshot.executor.is_none());
    assert_eq!(
        public_snapshot.creator.as_ref().unwrap().user_id,
        root.user.id
    );
    assert_eq!(
        public_invoker
            .invoke_workflow_internal_http(
                public_run,
                &public_refs.creator_credential,
                request("/api/console/me")
            )
            .await
            .unwrap()
            .status,
        200
    );
    assert_eq!(
        public_invoker
            .invoke_workflow_internal_http(
                public_run,
                &public_refs.executor_credential,
                request("/api/console/me")
            )
            .await
            .unwrap_err()
            .to_string(),
        "workflow_executor_credential_unavailable"
    );
    for (reference, expected_user, expected_role) in [
        (&refs.creator_credential, root.user.id, "root"),
        (&refs.executor_credential, user_id, "workflow_cred_selected"),
    ] {
        let response = restored
            .invoke_workflow_internal_http(run, reference, request("/api/console/me"))
            .await
            .unwrap();
        assert_eq!(response.status, 200);
        let body: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(body["data"]["id"], expected_user.to_string());
        assert_eq!(body["data"]["effective_display_role"], expected_role);
    }
    // The same Console operation still performs its normal policy authorization.
    let creator_roles = invoker
        .invoke_workflow_internal_http(
            run,
            &refs.creator_credential,
            request("/api/console/settings/roles"),
        )
        .await
        .unwrap();
    assert_eq!(creator_roles.status, 200);
    let executor_roles = invoker
        .invoke_workflow_internal_http(
            run,
            &refs.executor_credential,
            request("/api/console/settings/roles"),
        )
        .await
        .unwrap();
    assert_eq!(executor_roles.status, 403);
    assert_eq!(
        invoker
            .invoke_workflow_internal_http(
                other_run,
                &refs.executor_credential,
                request("/api/console/me")
            )
            .await
            .unwrap_err()
            .to_string(),
        "workflow_credential_reference_invalid"
    );
    let mut external_headers = HeaderMap::new();
    external_headers.insert(
        "authorization",
        format!("Bearer {}", refs.creator_credential)
            .parse()
            .unwrap(),
    );
    external_headers.insert("x-workflow-run-id", run.to_string().parse().unwrap());
    assert!(require_session(&state, &external_headers).await.is_err());
    // Disabling the creator does not prevent executor-only runs, including initial preparation.
    sqlx::query("update users set status = 'disabled' where id = $1")
        .bind(root.user.id)
        .execute(state.store.pool())
        .await
        .unwrap();
    let missing_creator_refs = invoker
        .prepare_workflow_credentials(other_run)
        .await
        .unwrap();
    assert!(state
        .store
        .get_workflow_run_identity(other_run)
        .await
        .unwrap()
        .unwrap()
        .creator
        .is_none());
    assert!(invoker
        .invoke_workflow_internal_http(
            other_run,
            &missing_creator_refs.creator_credential,
            request("/api/console/me")
        )
        .await
        .is_err());
    assert_eq!(
        invoker
            .invoke_workflow_internal_http(
                other_run,
                &missing_creator_refs.executor_credential,
                request("/api/console/me")
            )
            .await
            .unwrap()
            .status,
        200
    );
    assert!(invoker
        .invoke_workflow_internal_http(run, &refs.creator_credential, request("/api/console/me"))
        .await
        .is_err());
    // Current user status/session version and the exact frozen role must all remain valid.
    sqlx::query("update users set status = 'disabled' where id = $1")
        .bind(user_id)
        .execute(state.store.pool())
        .await
        .unwrap();
    assert!(invoker
        .invoke_workflow_internal_http(run, &refs.executor_credential, request("/api/console/me"))
        .await
        .is_err());
    sqlx::query(
        "update users set status = 'active', session_version = session_version + 1 where id = $1",
    )
    .bind(user_id)
    .execute(state.store.pool())
    .await
    .unwrap();
    assert!(invoker
        .invoke_workflow_internal_http(run, &refs.executor_credential, request("/api/console/me"))
        .await
        .is_err());
    sqlx::query("update users set session_version = session_version - 1 where id = $1")
        .bind(user_id)
        .execute(state.store.pool())
        .await
        .unwrap();
    sqlx::query("delete from workspace_memberships where user_id = $1 and workspace_id = $2")
        .bind(user_id)
        .bind(actor.current_workspace_id)
        .execute(state.store.pool())
        .await
        .unwrap();
    assert_eq!(
        invoker
            .invoke_workflow_internal_http(
                run,
                &refs.executor_credential,
                request("/api/console/me")
            )
            .await
            .unwrap_err()
            .to_string(),
        "workflow_credential_workspace_revoked"
    );
    sqlx::query("delete from user_role_bindings where user_id = $1 and role_id in (select id from roles where code = 'workflow_cred_selected')")
        .bind(user_id).execute(state.store.pool()).await.unwrap();
    assert!(invoker
        .invoke_workflow_internal_http(run, &refs.executor_credential, request("/api/console/me"))
        .await
        .is_err());
    state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: other_run,
            status: domain::FlowRunStatus::Succeeded,
            output_payload: json!({}),
            error_payload: None,
            finished_at: Some(time::OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
    assert_eq!(
        invoker
            .prepare_workflow_credentials(other_run)
            .await
            .unwrap_err()
            .to_string(),
        "workflow_credential_run_terminal"
    );
    assert!(invoker
        .invoke_workflow_internal_http(
            other_run,
            &missing_creator_refs.executor_credential,
            request("/api/console/me")
        )
        .await
        .is_err());
    // Deleting a run also deletes the identity snapshot through the formal migration's FK.
    sqlx::query("delete from flow_runs where id = $1")
        .bind(run)
        .execute(state.store.pool())
        .await
        .unwrap();
    assert!(state
        .store
        .get_workflow_run_identity(run)
        .await
        .unwrap()
        .is_none());
}
