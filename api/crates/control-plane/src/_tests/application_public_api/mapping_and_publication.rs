use super::*;

#[tokio::test]
async fn application_public_api_mapping_service_returns_default_then_replaces_stored_mapping() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiMappingService::new(harness.repository());

    let default_mapping = service
        .get_mapping(GetApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();
    let replacement = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: Some("node-start".into()),
            history_target: Some("node-start.history".into()),
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput {
            answer_selector: Some("answer.text".into()),
            usage_selector: None,
            files_selector: None,
            error_selector: None,
        },
        extension: None,
    };
    service
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: replacement.clone(),
        })
        .await
        .unwrap();
    let stored = service
        .get_mapping(GetApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();

    assert_eq!(
        default_mapping,
        ApplicationApiMappingConfig::default_native()
    );
    assert_eq!(stored, replacement);
}

#[tokio::test]
async fn application_public_api_mapping_draft_retains_extension_identity() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiMappingService::new(harness.repository());
    let mapping = workflow_extension_mapping("open-ticket");

    service
        .replace_mapping_draft(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: mapping.clone(),
        })
        .await
        .unwrap();
    service
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: mapping.clone(),
        })
        .await
        .unwrap();

    let immutable_error = service
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: workflow_extension_mapping("closed-ticket"),
        })
        .await
        .unwrap_err();
    let stored = service
        .get_mapping_draft(GetApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();

    assert_eq!(
        immutable_error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict(
            "workflow_extension_registration_immutable"
        ))
    );
    assert_eq!(stored.mapping, mapping);
}

#[tokio::test]
async fn application_public_api_publication_rejects_changed_persisted_extension_identity() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow");
    let mapping = workflow_extension_mapping("open-ticket");

    ApplicationApiMappingService::new(repository.clone())
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping,
        })
        .await
        .unwrap();
    let error = ApplicationPublicationService::new(repository)
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: workflow_extension_mapping("closed-ticket"),
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict(
            "workflow_extension_registration_immutable"
        ))
    );
}

#[tokio::test]
async fn application_public_api_mapping_service_requires_edit_permission_for_replace() {
    let harness =
        ApplicationPublicApiTestHarness::new_with_permissions(vec!["application.view.all"]);
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiMappingService::new(harness.repository());

    let error = service
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn application_public_api_mapping_service_rejects_duplicate_extension_slug() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first = harness.seed_application(actor_user_id(), "Support Bot A");
    let second = harness.seed_application(actor_user_id(), "Support Bot B");
    let service = ApplicationApiMappingService::new(harness.repository());

    service
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: first.id,
            mapping: workflow_extension_mapping("open-ticket-conflict"),
        })
        .await
        .unwrap();
    let error = service
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: second.id,
            mapping: workflow_extension_mapping("open-ticket-conflict"),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("extension_slug"));
}

#[tokio::test]
async fn application_public_api_mapping_service_rejects_extension_slug_used_by_publication() {
    let harness = ApplicationPublicApiTestHarness::new();
    let published = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow");
    let draft = harness.seed_application(actor_user_id(), "Draft Workflow");
    let repository = harness.repository();

    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: published.id,
            mapping: workflow_extension_mapping("open-ticket-cross-table"),
            api_enabled: true,
        })
        .await
        .unwrap();
    let error = ApplicationApiMappingService::new(repository)
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: draft.id,
            mapping: workflow_extension_mapping("open-ticket-cross-table"),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("extension_slug"));
}

#[tokio::test]
async fn application_public_api_publish_updates_current_publication_record() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationPublicationService::new(harness.repository());

    let first = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let second = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig {
                input: ApplicationApiMappingInput {
                    query_target: "node-start.query".into(),
                    model_target: None,
                    inputs_target: Some("node-start".into()),
                    history_target: None,
                    attachments_target: None,
                },
                output: ApplicationApiMappingOutput::default(),
                extension: None,
            },
            api_enabled: true,
        })
        .await
        .unwrap();

    let reloaded = service
        .get_publication_version(first.id)
        .await
        .unwrap()
        .unwrap();
    let versions = service
        .list_publication_versions(application.id)
        .await
        .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(versions, vec![second.clone()]);
    assert_eq!(reloaded.mapping_snapshot, second.mapping_snapshot);
    assert_eq!(reloaded.compiled_plan_id, second.compiled_plan_id);
    assert!(reloaded.active);
}

#[tokio::test]
async fn application_public_api_js_dependency_snapshot_is_empty_without_selection() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationPublicationService::new(harness.repository());

    let publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let reloaded = service
        .get_publication_version(publication.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(publication.dependency_snapshot, Vec::new());
    assert_eq!(reloaded.dependency_snapshot, Vec::new());
}

#[tokio::test]
async fn application_public_api_js_dependency_snapshot_updates_current_publication() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());

    ApplicationJsDependencySelectionRepository::replace_application_js_dependency_selection(
        &repository,
        &ReplaceApplicationJsDependencySelectionInput {
            actor_user_id: actor_user_id(),
            workspace_id: application.workspace_id,
            application_id: application.id,
            installation_id: Uuid::from_u128(0x90000000000000000000000000000001),
            provider_code: "fixture_js_dependency_pack_3".into(),
            plugin_id: "fixture_js_dependency_pack@3.24.0".into(),
            plugin_version: "3.24.0".into(),
            alias: "zod".into(),
            package: "zod".into(),
            version: "3.24.0".into(),
            target: "backend_code".into(),
            artifact_path: "artifacts/zod-3.24.0.backend.mjs".into(),
            artifact_hash: "sha256-zod-3.24.0".into(),
            integrity: "sha256-zod-3.24.0".into(),
            permissions: domain::JsDependencyPermissions {
                network: "outbound_only".into(),
                filesystem: "deny".into(),
                env: "deny".into(),
            },
        },
    )
    .await
    .unwrap();

    let first = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();

    ApplicationJsDependencySelectionRepository::replace_application_js_dependency_selection(
        &repository,
        &ReplaceApplicationJsDependencySelectionInput {
            actor_user_id: actor_user_id(),
            workspace_id: application.workspace_id,
            application_id: application.id,
            installation_id: Uuid::from_u128(0x90000000000000000000000000000002),
            provider_code: "fixture_js_dependency_pack_4".into(),
            plugin_id: "fixture_js_dependency_pack@4.0.0".into(),
            plugin_version: "4.0.0".into(),
            alias: "zod".into(),
            package: "zod".into(),
            version: "4.0.0".into(),
            target: "backend_code".into(),
            artifact_path: "artifacts/zod-4.0.0.backend.mjs".into(),
            artifact_hash: "sha256-zod-4.0.0".into(),
            integrity: "sha256-zod-4.0.0".into(),
            permissions: domain::JsDependencyPermissions {
                network: "outbound_only".into(),
                filesystem: "deny".into(),
                env: "deny".into(),
            },
        },
    )
    .await
    .unwrap();

    let second = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let reloaded = service
        .get_publication_version(first.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(reloaded.dependency_snapshot.len(), 1);
    assert_eq!(reloaded.dependency_snapshot[0].alias, "zod");
    assert_eq!(reloaded.dependency_snapshot[0].package, "zod");
    assert_eq!(reloaded.dependency_snapshot[0].version, "4.0.0");
    assert_eq!(
        reloaded.dependency_snapshot[0].artifact_hash,
        "sha256-zod-4.0.0"
    );
    assert_eq!(
        reloaded.dependency_snapshot[0].permissions.network,
        "outbound_only"
    );
    assert_eq!(second.dependency_snapshot[0].version, "4.0.0");
    assert_eq!(
        second.dependency_snapshot[0].artifact_hash,
        "sha256-zod-4.0.0"
    );
}

#[tokio::test]
async fn application_public_api_js_dependency_compile_context_enables_code_imports() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());
    let editor_state = repository
        .get_or_create_editor_state(application.workspace_id, application.id, actor_user_id())
        .await
        .unwrap();

    FlowRepository::save_draft(
        &repository,
        application.workspace_id,
        application.id,
        actor_user_id(),
        application_public_api_code_js_dependency_document(editor_state.flow.id),
        domain::FlowChangeKind::Logical,
        "Add code dependency import",
    )
    .await
    .unwrap();

    ApplicationJsDependencySelectionRepository::replace_application_js_dependency_selection(
        &repository,
        &ReplaceApplicationJsDependencySelectionInput {
            actor_user_id: actor_user_id(),
            workspace_id: application.workspace_id,
            application_id: application.id,
            installation_id: Uuid::from_u128(0x90000000000000000000000000000003),
            provider_code: "fixture_js_dependency_pack_3".into(),
            plugin_id: "fixture_js_dependency_pack@3.24.0".into(),
            plugin_version: "3.24.0".into(),
            alias: "zod".into(),
            package: "zod".into(),
            version: "3.24.0".into(),
            target: "backend_code".into(),
            artifact_path: "artifacts/zod-3.24.0.backend.mjs".into(),
            artifact_hash: "sha256-zod-3.24.0".into(),
            integrity: "sha256-zod-3.24.0".into(),
            permissions: domain::JsDependencyPermissions {
                network: "outbound_only".into(),
                filesystem: "deny".into(),
                env: "deny".into(),
            },
        },
    )
    .await
    .unwrap();

    let publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let compiled_plan = repository
        .get_compiled_plan(publication.compiled_plan_id)
        .await
        .unwrap()
        .expect("publish should persist a compiled plan");

    assert_eq!(
        compiled_plan.plan["compile_issues"],
        serde_json::json!([]),
        "application compile context should include selected backend_code::zod"
    );
}

#[tokio::test]
async fn application_public_api_publish_uses_real_flow_version_and_compiled_plan_records() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());

    let publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let editor_state = repository
        .get_or_create_editor_state(application.workspace_id, application.id, actor_user_id())
        .await
        .unwrap();
    let compiled_plan = repository
        .get_compiled_plan(publication.compiled_plan_id)
        .await
        .unwrap()
        .expect("publish should persist a compiled plan");

    assert_eq!(publication.flow_id, editor_state.flow.id);
    assert!(
        editor_state
            .versions
            .iter()
            .any(|version| version.id == publication.flow_version_id
                && version.is_current_publication)
    );
    assert_eq!(compiled_plan.flow_id, editor_state.flow.id);
    assert_eq!(publication.document_snapshot, editor_state.draft.document);
    assert_ne!(
        publication.flow_schema_version,
        "application-public-api-placeholder-v1"
    );
    assert_ne!(
        publication.document_snapshot["source"],
        "application_public_api_placeholder"
    );
}

#[tokio::test]
async fn republishing_moves_current_publication_without_user_protecting_the_previous_version() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Republished Support Bot");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());

    let first_publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let second_publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let editor_state = repository
        .get_or_create_editor_state(application.workspace_id, application.id, actor_user_id())
        .await
        .unwrap();
    let first_version = editor_state
        .versions
        .iter()
        .find(|version| version.id == first_publication.flow_version_id)
        .unwrap();
    let second_version = editor_state
        .versions
        .iter()
        .find(|version| version.id == second_publication.flow_version_id)
        .unwrap();

    assert!(!first_version.is_current_publication);
    assert!(!first_version.is_user_protected);
    assert!(second_version.is_current_publication);
    assert!(!second_version.is_user_protected);
}

#[tokio::test]
async fn application_public_api_publish_compiles_workflow_application_as_workflow() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());

    let publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .expect("workflow application should publish through workflow compiler");
    let compiled_plan = repository
        .get_compiled_plan(publication.compiled_plan_id)
        .await
        .unwrap()
        .expect("publish should persist workflow compiled plan");

    assert_eq!(
        compiled_plan.plan["nodes"]["node-workflow-start"]["node_type"],
        serde_json::json!("workflow_start")
    );
    assert_eq!(
        compiled_plan.plan["nodes"]["node-workflow-end"]["node_type"],
        serde_json::json!("workflow_end")
    );
}

#[tokio::test]
async fn application_public_api_publish_rejects_duplicate_extension_slug() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow A");
    let second = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow B");
    let service = ApplicationPublicationService::new(harness.repository());

    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: first.id,
            mapping: workflow_extension_mapping("open-ticket-publish-conflict"),
            api_enabled: true,
        })
        .await
        .unwrap();
    let error = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: second.id,
            mapping: workflow_extension_mapping("open-ticket-publish-conflict"),
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("extension_slug"));
}

#[tokio::test]
async fn application_public_api_publish_rejects_extension_slug_used_by_saved_mapping() {
    let harness = ApplicationPublicApiTestHarness::new();
    let draft = harness.seed_application(actor_user_id(), "Draft Workflow");
    let published = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow");
    let repository = harness.repository();

    ApplicationApiMappingService::new(repository.clone())
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: draft.id,
            mapping: workflow_extension_mapping("open-ticket-cross-table-publish"),
        })
        .await
        .unwrap();
    let error = ApplicationPublicationService::new(repository)
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: published.id,
            mapping: workflow_extension_mapping("open-ticket-cross-table-publish"),
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("extension_slug"));
}
