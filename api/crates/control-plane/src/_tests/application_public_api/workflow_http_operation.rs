use super::*;

#[tokio::test]
async fn workflow_http_operation_runs_with_authenticated_user_api_key_principal() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, true),
            application_row_operation(
                access_control::APPLICATIONS_VIEW_OPERATION_ID,
                domain::ConsoleOperationRowScope::Own,
            ),
        ]),
    ]);
    let application = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow");
    let repository = harness.repository();
    let mapping = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: Some(WorkflowExtensionApiConfig {
            slug: "open-ticket".into(),
            method: WorkflowExtensionHttpMethod::Post,
            response_mode: WorkflowExtensionResponseMode::Async,
        }),
    };
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping,
            api_enabled: true,
        })
        .await
        .unwrap();

    let forbidden = WorkflowExtensionRunService::new(repository.clone())
        .create_run(CreateWorkflowExtensionRunCommand {
            actor: domain::ActorContext {
                user_id: other_user_id(),
                tenant_id: Uuid::nil(),
                current_workspace_id: application.workspace_id,
                effective_display_role: "member".into(),
                is_root: false,
                permissions: std::collections::HashSet::new(),
            },
            principal: control_plane::application_public_api::workflow_extension::WorkflowHttpPrincipal::UserApiKey {
                api_key_id: Uuid::now_v7(),
            },
            request_path: "open-ticket".into(),
            method: WorkflowExtensionHttpMethod::Post,
            parameters: WorkflowExtensionRequestParameters::default(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        forbidden,
        control_plane::application_public_api::workflow_extension::WorkflowExtensionRunError::Forbidden
    ));

    let api_key_id = Uuid::now_v7();
    let run = WorkflowExtensionRunService::new(repository.clone())
        .create_run(CreateWorkflowExtensionRunCommand {
            actor: domain::ActorContext::root(actor_user_id(), application.workspace_id, "root"),
            principal: control_plane::application_public_api::workflow_extension::WorkflowHttpPrincipal::UserApiKey {
                api_key_id,
            },
            request_path: "open-ticket".into(),
            method: WorkflowExtensionHttpMethod::Post,
            parameters: WorkflowExtensionRequestParameters::default(),
        })
        .await
        .unwrap();
    let stored = repository
        .get_flow_run(application.id, run.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(run.status, domain::FlowRunStatus::Queued);
    assert_eq!(
        stored.input_payload["node-workflow-start"],
        serde_json::json!({})
    );
    assert_eq!(
        stored.input_payload["trigger"],
        serde_json::json!({ "type": "extension" })
    );
    let expected_trace = format!(
        "workflow-http:published_workflow_operation:{}",
        application.id
    );
    assert_eq!(
        stored.external_trace_id.as_deref(),
        Some(expected_trace.as_str())
    );
    assert_eq!(stored.compatibility_mode, None);
    assert_eq!(run.api_key_id, Some(api_key_id));
}
