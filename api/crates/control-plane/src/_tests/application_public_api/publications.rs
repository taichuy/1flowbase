use control_plane::{
    application_public_api::{
        mapping::{
            ApplicationCompactOperationBindings, ApplicationOperationBindings,
            ApplicationOperationTargetBinding,
        },
        publications::UnpublishApplicationCommand,
    },
    errors::ControlPlaneError,
    ports::ApplicationPublicationRepository,
};
use orchestration_runtime::compiled_plan::{CompiledLlmRuntime, CompiledNode, CompiledPlan};

use super::*;

fn unpublish_command(application_id: Uuid) -> UnpublishApplicationCommand {
    UnpublishApplicationCommand {
        actor_user_id: actor_user_id(),
        application_id,
    }
}

fn compiled_plan(nodes: Vec<CompiledNode>) -> CompiledPlan {
    CompiledPlan {
        flow_id: Uuid::now_v7(),
        source_draft_id: Uuid::now_v7().to_string(),
        schema_version: domain::FLOW_SCHEMA_VERSION.into(),
        topological_order: nodes.iter().map(|node| node.node_id.clone()).collect(),
        edges: Vec::new(),
        nodes: nodes
            .into_iter()
            .map(|node| (node.node_id.clone(), node))
            .collect(),
        compile_issues: Vec::new(),
    }
}

fn compiled_node(
    node_id: &str,
    node_type: &str,
    llm_runtime: Option<CompiledLlmRuntime>,
) -> CompiledNode {
    CompiledNode {
        node_id: node_id.into(),
        node_type: node_type.into(),
        alias: node_id.into(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: Default::default(),
        outputs: Vec::new(),
        config: serde_json::json!({}),
        plugin_runtime: None,
        llm_runtime,
        code_runtime: None,
    }
}

fn complete_llm_runtime() -> CompiledLlmRuntime {
    CompiledLlmRuntime {
        provider_instance_id: Uuid::now_v7().to_string(),
        provider_instance_display_name: "Fixture Provider".into(),
        provider_code: "fixture_provider".into(),
        protocol: "fixture".into(),
        model: "fixture-model".into(),
        routing: None,
    }
}

fn generate_binding(target_node_id: &str) -> ApplicationOperationBindings {
    ApplicationOperationBindings {
        generate: Some(ApplicationOperationTargetBinding {
            target_node_id: target_node_id.into(),
        }),
        count_tokens: None,
        compact: ApplicationCompactOperationBindings::default(),
    }
}

/// Root #1366 AC-003 / AC-006: legacy preview never guesses between targets.
#[test]
fn legacy_operation_binding_preview_backfills_generate_only_for_one_runnable_llm_target() {
    let zero = compiled_plan(vec![compiled_node("node-start", "start", None)]);
    let one = compiled_plan(vec![compiled_node(
        "node-llm-a",
        "llm",
        Some(complete_llm_runtime()),
    )]);
    let two = compiled_plan(vec![
        compiled_node("node-llm-a", "llm", Some(complete_llm_runtime())),
        compiled_node("node-llm-b", "llm", Some(complete_llm_runtime())),
    ]);

    assert_eq!(
        control_plane::application_public_api::publications::preview_legacy_operation_bindings(
            &zero
        )
        .generate,
        None
    );
    assert_eq!(
        control_plane::application_public_api::publications::preview_legacy_operation_bindings(
            &one
        )
        .generate
        .as_ref()
        .map(|binding| binding.target_node_id.as_str()),
        Some("node-llm-a")
    );
    assert_eq!(
        control_plane::application_public_api::publications::preview_legacy_operation_bindings(
            &two
        )
        .generate,
        None
    );
}

/// Root #1366 AC-003 / AC-005: bound targets must be exact runnable compiled LLM identities.
#[test]
fn publish_binding_validation_rejects_missing_non_llm_and_incomplete_runtime_targets() {
    let mut incomplete_runtime = complete_llm_runtime();
    incomplete_runtime.model.clear();
    let plan = compiled_plan(vec![
        compiled_node("node-start", "start", None),
        compiled_node("node-llm-missing-runtime", "llm", None),
        compiled_node("node-llm-incomplete", "llm", Some(incomplete_runtime)),
        compiled_node("node-llm-ready", "llm", Some(complete_llm_runtime())),
    ]);

    for target_node_id in [
        " missing ",
        "missing",
        "node-start",
        "node-llm-missing-runtime",
        "node-llm-incomplete",
    ] {
        let error = control_plane::application_public_api::publications::validate_operation_binding_targets(
            &generate_binding(target_node_id),
            &plan,
        )
        .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::InvalidInput(
                "operation_bindings.generate"
            ))
        ));
    }

    control_plane::application_public_api::publications::validate_operation_binding_targets(
        &generate_binding("node-llm-ready"),
        &plan,
    )
    .unwrap();
}

/// Root #1366 AC-003 / AC-006: editing the draft never rewrites its published binding snapshot.
#[tokio::test]
async fn published_operation_bindings_are_immutable_after_draft_replacement() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Immutable Binding App");
    let publication_service = ApplicationPublicationService::new(repository.clone());
    let publication = publication_service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();

    let draft = ApplicationApiMappingService::new(repository.clone())
        .replace_mapping_draft(
            ReplaceApplicationApiMappingCommand {
                actor_user_id: actor_user_id(),
                application_id: application.id,
                mapping: ApplicationApiMappingConfig::default_native(),
            },
            Some(generate_binding("future-node")),
        )
        .await
        .unwrap();
    let reloaded = publication_service
        .get_publication_version(publication.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        draft
            .operation_bindings
            .generate
            .as_ref()
            .map(|binding| binding.target_node_id.as_str()),
        Some("future-node")
    );
    assert_eq!(
        reloaded.operation_bindings,
        ApplicationOperationBindings::default()
    );
}

/// #1286 AC-001 / AC-004
#[tokio::test]
async fn unpublish_reverts_active_publication_and_allows_republish() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Lifecycle App");
    let service = ApplicationPublicationService::new(repository.clone());
    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();

    service
        .unpublish(unpublish_command(application.id))
        .await
        .unwrap();

    assert!(repository
        .load_active_application_publication(application.id)
        .await
        .unwrap()
        .is_none());
    let load_error = service
        .load_active_publication(LoadActiveApplicationPublicationCommand {
            application_id: application.id,
        })
        .await
        .unwrap_err();
    assert_eq!(load_error.to_string(), "application_not_published");

    let republished = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    assert!(republished.active);
    assert!(republished.api_enabled);
    assert!(repository
        .load_active_application_publication(application.id)
        .await
        .unwrap()
        .is_some());
}

/// #1286 AC-002
#[tokio::test]
async fn unpublish_removes_enabled_extension_registration() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_workflow_application(actor_user_id(), "Extension Lifecycle");
    let service = ApplicationPublicationService::new(repository.clone());
    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: workflow_extension_mapping("lifecycle-hook"),
            api_enabled: true,
        })
        .await
        .unwrap();
    assert_eq!(
        repository
            .list_enabled_extension_publications()
            .await
            .unwrap()
            .len(),
        1
    );

    service
        .unpublish(unpublish_command(application.id))
        .await
        .unwrap();

    assert!(repository
        .list_enabled_extension_publications()
        .await
        .unwrap()
        .is_empty());
    assert!(repository
        .load_active_application_publication_by_extension_slug("lifecycle-hook")
        .await
        .unwrap()
        .is_none());
}

/// #1286 AC-003
#[tokio::test]
async fn unpublish_stops_schedule_dispatch() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_workflow_application(actor_user_id(), "Schedule Lifecycle");
    harness.set_workflow_trigger_type(application.id, domain::WorkflowTriggerType::Schedule);
    let schedule_service = WorkflowScheduleTriggerService::new(repository.clone());
    schedule_service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "* * * * *".into(),
            timezone: "UTC".into(),
            input_payload: serde_json::json!({}),
        })
        .await
        .unwrap();
    let publication_service = ApplicationPublicationService::new(repository.clone());
    publication_service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let dispatched = schedule_service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at: time::OffsetDateTime::UNIX_EPOCH,
            },
            None,
        )
        .await
        .unwrap();
    assert!(matches!(
        dispatched,
        WorkflowScheduleDispatchOutcome::Dispatched(_)
    ));

    publication_service
        .unpublish(unpublish_command(application.id))
        .await
        .unwrap();

    let stopped = schedule_service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at: time::OffsetDateTime::UNIX_EPOCH + time::Duration::minutes(1),
            },
            None,
        )
        .await
        .unwrap();
    assert!(matches!(
        stopped,
        WorkflowScheduleDispatchOutcome::Skipped {
            reason: "application_not_published",
        }
    ));
}

/// #1286 AC-006
#[tokio::test]
async fn unpublish_requires_applications_publish_console_operation() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![application_simple_operation(
            access_control::APPLICATIONS_PUBLISH_OPERATION_ID,
            false,
        )]),
    ]);
    let application = harness.seed_application(other_user_id(), "Locked Lifecycle");
    let service = ApplicationPublicationService::new(harness.repository());

    let denied = service
        .unpublish(unpublish_command(application.id))
        .await
        .unwrap_err();

    assert!(matches!(
        denied.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied(_))
    ));
}

/// #1286 AC-006
#[tokio::test]
async fn unpublish_is_governed_by_publish_operation_not_api_status_operation() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, true),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                false,
            ),
        ]),
    ]);
    let repository = harness.repository();
    let application = harness.seed_application(other_user_id(), "Publish Governed");
    let service = ApplicationPublicationService::new(repository.clone());
    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();

    service
        .unpublish(unpublish_command(application.id))
        .await
        .unwrap();

    assert!(repository
        .load_active_application_publication(application.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn unpublish_without_active_publication_returns_publication_not_found() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Draft Only");
    let service = ApplicationPublicationService::new(harness.repository());

    let error = service
        .unpublish(unpublish_command(application.id))
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("publication"))
    ));
}
