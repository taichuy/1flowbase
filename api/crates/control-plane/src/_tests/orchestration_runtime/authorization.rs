use control_plane::errors::ControlPlaneError;
use control_plane::orchestration_runtime::{
    CancelFlowRunCommand, CompleteCallbackTaskCommand, OrchestrationRuntimeService,
    PrepareFlowDebugRunCommand, ResumeFlowRunCommand, StartFlowDebugRunCommand,
    StartNodeDebugPreviewCommand,
};
use domain::{ConsoleOperationId, ConsoleOperationPolicy, ConsolePolicyGroup};
use serde_json::json;
use uuid::Uuid;

fn applications_group() -> ConsolePolicyGroup {
    ConsolePolicyGroup::settings_feature(access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID)
        .expect("applications settings feature id must be valid")
}

fn run_policy(enabled: bool) -> domain::RoleConsolePolicy {
    domain::RoleConsolePolicy::new(
        Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::custom(
            applications_group(),
            vec![ConsoleOperationPolicy::simple(
                ConsoleOperationId::try_from(access_control::APPLICATIONS_RUN_OPERATION_ID)
                    .expect("applications run operation id must be valid"),
                enabled,
            )],
        )],
    )
}

fn view_policy(scope: domain::ConsoleOperationRowScope) -> domain::RoleConsolePolicy {
    domain::RoleConsolePolicy::new(
        Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::custom(
            applications_group(),
            vec![ConsoleOperationPolicy::row(
                ConsoleOperationId::try_from(access_control::APPLICATIONS_VIEW_OPERATION_ID)
                    .expect("applications view operation id must be valid"),
                scope,
            )],
        )],
    )
}

#[tokio::test]
async fn ac_005_ac_007_run_requires_simple_grant_and_persisted_view_owner() {
    let service = OrchestrationRuntimeService::for_tests_with_application_console_policies(
        Vec::new(),
        vec![
            run_policy(false),
            run_policy(true),
            view_policy(domain::ConsoleOperationRowScope::Own),
        ],
    );
    let seeded = service.seed_application_with_flow("Owned run").await;
    let peer = service.seed_application_with_flow("Peer run").await;

    let shell = service
        .open_flow_debug_run_shell(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .expect("run own must allow the persisted owner");

    assert_eq!(shell.status, domain::FlowRunStatus::Queued);

    let error = service
        .open_flow_debug_run_shell(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: peer.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .expect_err("view own must not run a same-workspace peer application");

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied("permission_denied"))
    ));
}

#[tokio::test]
async fn ac_1271_run_simple_allows_flow_and_node_debug_with_view_own() {
    let service = OrchestrationRuntimeService::for_tests_with_application_console_policies(
        Vec::new(),
        vec![
            run_policy(true),
            view_policy(domain::ConsoleOperationRowScope::Own),
        ],
    );
    let seeded = service
        .seed_application_with_flow("Simple debug only")
        .await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .expect("flow debug must require applications.run with view own");
    assert_eq!(started.flow_run.status, domain::FlowRunStatus::Running);

    let preview = service
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            node_id: "node-llm".to_string(),
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .expect("node debug must require applications.run with view own");
    assert_eq!(preview.node_run.status, domain::NodeRunStatus::Succeeded);
}

#[tokio::test]
async fn ac_1271_run_simple_allows_cancel_resume_and_callback_with_view_own() {
    let service = OrchestrationRuntimeService::for_tests();
    let cancellable = service.seed_application_with_flow("Cancellable").await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: cancellable.actor_user_id,
            application_id: cancellable.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .expect("seed cancellable run");
    let resumable = service.seed_waiting_human_run("Resumable").await;
    let callback = service.seed_waiting_callback_run("Callback").await;
    service.replace_application_console_policies_for_tests(vec![
        run_policy(true),
        view_policy(domain::ConsoleOperationRowScope::Own),
    ]);

    let cancelled = service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: cancellable.actor_user_id,
            application_id: cancellable.application_id,
            flow_run_id: started.flow_run.id,
        })
        .await
        .expect("cancel must use applications.run independently");
    assert_eq!(cancelled.flow_run.status, domain::FlowRunStatus::Cancelled);

    let resumed = service
        .resume_flow_run(ResumeFlowRunCommand {
            actor_user_id: resumable.actor_user_id,
            application_id: resumable.application_id,
            flow_run_id: resumable.flow_run_id,
            checkpoint_id: resumable.checkpoint_id,
            input_payload: json!({ "node-human": { "input": "approved" } }),
        })
        .await
        .expect("resume must use applications.run independently");
    assert_eq!(resumed.flow_run.status, domain::FlowRunStatus::Succeeded);

    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: callback.actor_user_id,
            application_id: callback.application_id,
            callback_task_id: callback.callback_task_id,
            response_payload: json!({ "result": { "status": "ok" } }),
        })
        .await
        .expect("callback completion must use applications.run independently");
    assert_eq!(completed.callback_tasks[0].status.as_str(), "completed");
}

#[tokio::test]
async fn ac_1271_legacy_view_does_not_authorize_debug_shell_when_run_simple_is_disabled() {
    let service = OrchestrationRuntimeService::for_tests_with_application_console_policies(
        vec!["application.view.all", "application.create.all"],
        vec![
            view_policy(domain::ConsoleOperationRowScope::ScopeAll),
            run_policy(false),
        ],
    );
    let seeded = service.seed_application_with_flow("Legacy view only").await;

    let error = service
        .open_flow_debug_run_shell(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .expect_err("view row scope must not authorize applications.run without its simple grant");

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied("permission_denied"))
    ));
}

#[tokio::test]
async fn ac_1271_disabled_run_is_rejected_before_run_task_checkpoint_or_node_lookup() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Denied runtime actions")
        .await;
    service.replace_application_console_policies_for_tests(vec![
        view_policy(domain::ConsoleOperationRowScope::ScopeAll),
        run_policy(false),
    ]);

    let cancel_error = service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: Uuid::now_v7(),
        })
        .await
        .expect_err("disabled run must reject cancel before loading the run");
    let resume_error = service
        .resume_flow_run(ResumeFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: Uuid::now_v7(),
            checkpoint_id: Uuid::now_v7(),
            input_payload: json!({}),
        })
        .await
        .expect_err("disabled run must reject resume before loading run or checkpoint");
    let callback_error = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: Uuid::now_v7(),
            response_payload: json!({}),
        })
        .await
        .expect_err("disabled run must reject before loading callback task");
    let node_error = service
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            node_id: "missing-node".to_string(),
            input_payload: json!({}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .expect_err("disabled run must reject before resolving node");

    for error in [cancel_error, resume_error, callback_error, node_error] {
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::PermissionDenied("permission_denied"))
        ));
    }
}

#[tokio::test]
async fn ac_1271_revoked_run_is_rejected_before_preparing_existing_shell() {
    let service = OrchestrationRuntimeService::for_tests_with_application_console_policies(
        Vec::new(),
        vec![
            run_policy(true),
            view_policy(domain::ConsoleOperationRowScope::Own),
        ],
    );
    let seeded = service
        .seed_application_with_flow("Revoked before prepare")
        .await;
    let input_payload = json!({ "node-start": { "query": "hello" } });
    let shell = service
        .open_flow_debug_run_shell(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: input_payload.clone(),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .expect("run policy should allow opening the shell");

    service.replace_application_console_policies_for_tests(vec![
        view_policy(domain::ConsoleOperationRowScope::Own),
        run_policy(false),
    ]);
    let error = service
        .prepare_flow_debug_run_from_shell(PrepareFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: shell.id,
            input_payload,
            document_snapshot: None,
            debug_session_id: String::new(),
        })
        .await
        .expect_err("revoked applications.run must reject before shell preparation");

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied("permission_denied"))
    ));
}

#[tokio::test]
async fn ac_1271_run_authorization_never_crosses_actor_current_workspace() {
    let service = OrchestrationRuntimeService::for_tests();
    let actor_user_id = Uuid::now_v7();
    let foreign = service
        .seed_application_in_workspace_for_tests(Uuid::now_v7(), actor_user_id, "Foreign workspace")
        .await;

    let error = service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id,
            application_id: foreign.id,
            flow_run_id: Uuid::now_v7(),
        })
        .await
        .expect_err("run grant must not cross actor workspace");

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("application"))
    ));
}

#[tokio::test]
async fn ac_1271_root_bypasses_run_policy_but_remains_in_current_workspace() {
    let workspace_id = Uuid::now_v7();
    let root_user_id = Uuid::now_v7();
    let service = OrchestrationRuntimeService::for_tests_as_root_in_workspace(workspace_id);
    let local = service
        .seed_application_in_workspace_for_tests(workspace_id, root_user_id, "Root local")
        .await;
    service.replace_application_console_policies_for_tests(Vec::new());

    let local_error = service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: root_user_id,
            application_id: local.id,
            flow_run_id: Uuid::now_v7(),
        })
        .await
        .expect_err("root reaches the run state check without a role policy");
    assert!(local_error.to_string().contains("flow run not found"));

    let foreign = service
        .seed_application_in_workspace_for_tests(Uuid::now_v7(), root_user_id, "Root foreign")
        .await;
    let foreign_error = service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: root_user_id,
            application_id: foreign.id,
            flow_run_id: Uuid::now_v7(),
        })
        .await
        .expect_err("root must not cross current workspace");
    assert!(matches!(
        foreign_error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("application"))
    ));
}
