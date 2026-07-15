use control_plane::{
    application_public_api::publications::UnpublishApplicationCommand,
    errors::ControlPlaneError, ports::ApplicationPublicationRepository,
};

use super::*;

fn unpublish_command(application_id: Uuid) -> UnpublishApplicationCommand {
    UnpublishApplicationCommand {
        actor_user_id: actor_user_id(),
        application_id,
    }
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
