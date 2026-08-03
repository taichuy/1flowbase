use super::*;

#[tokio::test]
async fn ac_007_application_public_api_simple_operations_are_independent_from_crud() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, false),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                false,
            ),
        ]),
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, true),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                true,
            ),
        ]),
    ]);
    let application = harness.seed_application(other_user_id(), "Same-workspace support bot");
    let service = ApplicationPublicationService::new(harness.repository());

    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .expect("publish simple operation must not require applications.update");
    service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_enabled: false,
        })
        .await
        .expect("API-status simple operation must not require applications.update");
}

#[tokio::test]
async fn ac_007_application_public_api_crud_scope_changes_do_not_disable_simple_operations() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, true),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                true,
            ),
            application_row_operation(
                access_control::APPLICATIONS_UPDATE_OPERATION_ID,
                domain::ConsoleOperationRowScope::Own,
            ),
        ]),
        application_console_policy(vec![application_row_operation(
            access_control::APPLICATIONS_UPDATE_OPERATION_ID,
            domain::ConsoleOperationRowScope::ScopeAll,
        )]),
    ]);
    let peer_application = harness.seed_application(other_user_id(), "Peer support bot");
    let service = ApplicationPublicationService::new(harness.repository());

    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: peer_application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .expect("publish grant must remain independent from update scope");
    service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: peer_application.id,
            api_enabled: false,
        })
        .await
        .expect("API-status grant must remain independent from update scope");
}

#[tokio::test]
async fn ac_007_application_public_api_simple_operations_do_not_read_legacy_edit_grants() {
    let harness = ApplicationPublicApiTestHarness::new_with_permissions(vec![
        "application.view.all",
        "application.edit.all",
    ]);
    let application = harness.seed_application(actor_user_id(), "Legacy only");
    let service = ApplicationPublicationService::new(harness.repository());

    let publish_error = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap_err();
    let status_error = service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(publish_error.to_string().contains("permission_denied"));
    assert!(status_error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn ac_007_application_public_api_simple_operations_default_deny_and_honor_disabled() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, true),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                false,
            ),
        ]),
    ]);
    let application = harness.seed_application(actor_user_id(), "Disabled status");
    let service = ApplicationPublicationService::new(harness.repository());

    let status_error = service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(status_error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn ac_006_application_public_api_simple_operations_cannot_cross_workspace() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, true),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                true,
            ),
        ]),
    ]);
    let application =
        harness.seed_application_in_workspace(Uuid::now_v7(), actor_user_id(), "Other workspace");
    let service = ApplicationPublicationService::new(harness.repository());

    let publish_error = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap_err();
    let status_error = service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(publish_error
        .to_string()
        .contains("resource not found: application"));
    assert!(status_error
        .to_string()
        .contains("resource not found: application"));
}

#[tokio::test]
async fn ac_006_application_public_api_root_bypasses_policy_but_not_workspace() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(Vec::new());
    let current = harness.seed_application(root_user_id(), "Root current workspace");
    let foreign = harness.seed_application_in_workspace(
        Uuid::now_v7(),
        root_user_id(),
        "Root other workspace",
    );
    let service = ApplicationPublicationService::new(harness.repository());

    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: root_user_id(),
            application_id: current.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: root_user_id(),
            application_id: current.id,
            api_enabled: false,
        })
        .await
        .unwrap();

    for error in [
        service
            .publish_active_version(PublishApplicationCommand {
                actor_user_id: root_user_id(),
                application_id: foreign.id,
                mapping: ApplicationApiMappingConfig::default_native(),
                api_enabled: true,
            })
            .await
            .unwrap_err(),
        service
            .set_api_enabled(SetApplicationApiEnabledCommand {
                actor_user_id: root_user_id(),
                application_id: foreign.id,
                api_enabled: true,
            })
            .await
            .unwrap_err(),
    ] {
        assert!(error
            .to_string()
            .contains("resource not found: application"));
    }
}

#[tokio::test]
async fn ac_1281_application_api_helpers_use_persisted_console_row_scope() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_row_operation(
                access_control::APPLICATIONS_VIEW_OPERATION_ID,
                domain::ConsoleOperationRowScope::ScopeAll,
            ),
            application_row_operation(
                access_control::APPLICATIONS_UPDATE_OPERATION_ID,
                domain::ConsoleOperationRowScope::ScopeAll,
            ),
        ]),
    ]);
    let application = harness.seed_application(other_user_id(), "Peer helper application");
    let workflow = harness.seed_workflow_application(other_user_id(), "Peer scheduled workflow");
    let repository = harness.repository();

    let created_key = ApplicationApiKeyService::new(repository.clone())
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Policy key".into(),
            expires_at: None,
        })
        .await;
    let mapping = ApplicationApiMappingService::new(repository.clone())
        .get_mapping(GetApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await;
    let schedule = WorkflowScheduleTriggerService::new(repository)
        .get_trigger(GetWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: workflow.id,
        })
        .await;

    assert!(
        created_key.is_ok(),
        "API-key owner must use applications.update"
    );
    assert!(mapping.is_ok(), "mapping owner must use applications.view");
    assert!(
        schedule.is_ok(),
        "schedule owner must use applications.view"
    );
}

#[tokio::test]
async fn ac_1281_application_api_helpers_reject_legacy_only_grants() {
    let harness = ApplicationPublicApiTestHarness::new_with_permissions(vec![
        "application.view.all",
        "application.edit.all",
    ]);
    let application = harness.seed_application(other_user_id(), "Legacy helper application");
    let workflow = harness.seed_workflow_application(other_user_id(), "Legacy scheduled workflow");
    let repository = harness.repository();

    let created_key = ApplicationApiKeyService::new(repository.clone())
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Legacy key".into(),
            expires_at: None,
        })
        .await;
    let mapping = ApplicationApiMappingService::new(repository.clone())
        .get_mapping(GetApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await;
    let schedule = WorkflowScheduleTriggerService::new(repository)
        .get_trigger(GetWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: workflow.id,
        })
        .await;

    assert!(created_key.is_err());
    assert!(mapping.is_err());
    assert!(schedule.is_err());
}

#[tokio::test]
async fn ac_007_publish_operation_does_not_bypass_mapping_domain_validation() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![application_simple_operation(
            access_control::APPLICATIONS_PUBLISH_OPERATION_ID,
            true,
        )]),
    ]);
    let application = harness.seed_application(actor_user_id(), "Invalid mapping");
    let mut mapping = ApplicationApiMappingConfig::default_native();
    mapping.input.query_target.clear();

    let error = ApplicationPublicationService::new(harness.repository())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping,
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("invalid input: query_target"));
}

#[tokio::test]
async fn application_public_api_only_one_current_publication_exists_per_application() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationPublicationService::new(harness.repository());

    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let latest = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();

    let versions = service
        .list_publication_versions(application.id)
        .await
        .unwrap();
    assert_eq!(versions, vec![latest]);
}

#[tokio::test]
async fn application_public_api_public_lookup_returns_application_not_published_without_active_publication(
) {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationPublicationService::new(harness.repository());

    let error = service
        .load_active_publication(LoadActiveApplicationPublicationCommand {
            application_id: application.id,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("application_not_published"));
}

#[test]
fn application_public_api_mapping_validation_rejects_missing_query_target_and_invalid_selector() {
    let missing_query_target = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    };
    let invalid_selector = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "start.messages[0].content".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    };

    assert!(validate_application_api_mapping(&missing_query_target)
        .unwrap_err()
        .to_string()
        .contains("query_target"));
    assert!(validate_application_api_mapping(&invalid_selector)
        .unwrap_err()
        .to_string()
        .contains("selector"));
}

#[test]
fn application_public_api_mapping_validation_accepts_null_model_target() {
    let mapping = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    };

    validate_application_api_mapping(&mapping).unwrap();
}

#[test]
fn application_public_api_mapping_validation_rejects_invalid_workflow_extension_config() {
    let mut invalid_slug = workflow_extension_mapping("OpenTicket");
    assert!(validate_application_api_mapping(&invalid_slug)
        .unwrap_err()
        .to_string()
        .contains("extension.slug"));

    invalid_slug = workflow_extension_mapping("open-ticket/*rest");
    assert!(validate_application_api_mapping(&invalid_slug)
        .unwrap_err()
        .to_string()
        .contains("extension.slug"));
}
