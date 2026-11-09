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

async fn frozen_operation_fixture() -> (ApplicationPublicApiTestHarness, domain::ApplicationRecord)
{
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(actor_user_id(), "Frozen HTTP workflow");
    ApplicationPublicationService::new(harness.repository())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: workflow_extension_mapping("frozen-http"),
            api_enabled: true,
        })
        .await
        .unwrap();
    (harness, application)
}
#[tokio::test]
async fn workflow_preparation_freezes_actual_publication_even_when_same_row_is_republished() {
    use control_plane::application_public_api::workflow_extension::WorkflowHttpPrincipal;
    let (harness, application) = frozen_operation_fixture().await;
    let repository = harness.repository();
    let service = WorkflowExtensionRunService::new(repository.clone());
    let actor = domain::ActorContext::root(actor_user_id(), application.workspace_id, "root");
    let authorized = service
        .authorize_operation(&actor, WorkflowExtensionHttpMethod::Post, "frozen-http")
        .await
        .unwrap();
    let original_id = authorized.operation().publication_version_id;
    let original_generation = authorized.generation().to_string();
    let original_plan = authorized.operation().publication.compiled_plan_id;
    let prepared = service
        .prepare_authorized_run(
            authorized,
            WorkflowHttpPrincipal::User,
            &WorkflowExtensionRequestParameters::default(),
            "frozen-http",
        )
        .unwrap();
    repository.set_active_publication_document_snapshot(application.id,serde_json::json!({"graph":{"nodes":[{"id":"replacement-start","type":"workflow_start","config":{"input_fields":[{"key":"new_required","source":"body","valueType":"string","required":true}]}},{"id":"end","type":"workflow_end","outputs":[]}]}}));
    let replacement = service
        .authorize_operation(&actor, WorkflowExtensionHttpMethod::Post, "frozen-http")
        .await
        .unwrap();
    assert_eq!(
        replacement.operation().publication_version_id,
        original_id,
        "publication row UUID is not a generation"
    );
    assert_ne!(replacement.generation(), original_generation);
    assert!(service
        .prepare_authorized_run(
            replacement,
            WorkflowHttpPrincipal::User,
            &WorkflowExtensionRequestParameters::default(),
            "frozen-http"
        )
        .is_err());
    let run = service.create_prepared_run(prepared).await.unwrap();
    let stored = repository
        .get_flow_run(application.id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.publication_version_id, Some(original_id));
    assert_eq!(stored.compiled_plan_id, Some(original_plan));
    assert!(stored.input_payload.get("replacement-start").is_none());
    assert_eq!(
        stored.input_payload["node-workflow-start"],
        serde_json::json!({})
    );
}
#[tokio::test]
async fn withdraw_blocks_new_authorization_without_replacing_an_already_prepared_target() {
    use control_plane::application_public_api::workflow_extension::{
        WorkflowExtensionRunError, WorkflowHttpPrincipal,
    };
    let (harness, application) = frozen_operation_fixture().await;
    let repository = harness.repository();
    let service = WorkflowExtensionRunService::new(repository.clone());
    let actor = domain::ActorContext::root(actor_user_id(), application.workspace_id, "root");
    let authorized = service
        .authorize_operation_id(
            &actor,
            &format!("published_workflow_operation:{}", application.id),
        )
        .await
        .unwrap();
    let prepared = service
        .prepare_authorized_run(
            authorized,
            WorkflowHttpPrincipal::User,
            &WorkflowExtensionRequestParameters::default(),
            "frozen-http",
        )
        .unwrap();
    ApplicationPublicationService::new(repository.clone())
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_enabled: false,
        })
        .await
        .unwrap();
    assert!(matches!(
        service
            .authorize_operation(&actor, WorkflowExtensionHttpMethod::Post, "frozen-http")
            .await,
        Err(WorkflowExtensionRunError::ExtensionNotFound)
    ));
    assert!(service.create_prepared_run(prepared).await.is_ok());
}
#[tokio::test]
async fn authorized_workflow_input_uses_resolved_path_coercions_and_declared_values_only() {
    use control_plane::application_public_api::workflow_extension::WorkflowHttpPrincipal;
    let (harness, application) = frozen_operation_fixture().await;
    let repository = harness.repository();
    repository.set_active_publication_document_snapshot(application.id,serde_json::json!({"graph":{"nodes":[{"id":"start","type":"workflow_start","config":{"input_fields":[{"key":"count","source":"query","valueType":"number","required":true},{"key":"enabled","source":"query","valueType":"boolean","defaultValue":true}]}},{"id":"end","type":"workflow_end","outputs":[]}]}}));
    let service = WorkflowExtensionRunService::new(repository.clone());
    let actor = domain::ActorContext::root(actor_user_id(), application.workspace_id, "root");
    let authorized = service
        .authorize_operation(&actor, WorkflowExtensionHttpMethod::Post, "frozen-http")
        .await
        .unwrap();
    let mut parameters = WorkflowExtensionRequestParameters::default();
    parameters.query = serde_json::from_value(
        serde_json::json!({"count":"7","credential-sentinel":"must-not-project"}),
    )
    .unwrap();
    parameters
        .path
        .insert("injected".into(), serde_json::json!("other-target"));
    let prepared = service
        .prepare_authorized_run(
            authorized,
            WorkflowHttpPrincipal::User,
            &parameters,
            "frozen-http",
        )
        .unwrap();
    let expected = serde_json::json!({"start":{"count":7.0,"enabled":true}});
    assert_eq!(prepared.node_input_payload(), &expected);
    let run = service.create_prepared_run(prepared).await.unwrap();
    let stored = repository
        .get_flow_run(application.id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.input_payload["start"], expected["start"]);
    assert!(!stored
        .input_payload
        .to_string()
        .contains("credential-sentinel"));
}
#[tokio::test]
async fn workflow_operation_proof_rejects_cross_workspace_even_for_root() {
    use control_plane::application_public_api::workflow_extension::WorkflowExtensionRunError;
    let (harness, application) = frozen_operation_fixture().await;
    let service = WorkflowExtensionRunService::new(harness.repository());
    let actor = domain::ActorContext::root(actor_user_id(), Uuid::now_v7(), "root");
    assert!(matches!(
        service
            .authorize_operation_id(
                &actor,
                &format!("published_workflow_operation:{}", application.id)
            )
            .await,
        Err(WorkflowExtensionRunError::Forbidden)
    ));
}
