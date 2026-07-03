use control_plane::application::{
    ApplicationService, CreateApplicationCommand, CreateApplicationTagCommand,
    DeleteApplicationCommand, UpdateApplicationCommand,
};
use domain::ApplicationType;
use uuid::Uuid;

#[tokio::test]
async fn create_application_requires_application_create_all() {
    let service = ApplicationService::for_tests_with_permissions(vec!["application.view.own"]);

    let error = service
        .create_application(CreateApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Blocked".into(),
            description: "blocked".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn list_applications_uses_own_scope_when_actor_lacks_all_scope() {
    let service = ApplicationService::for_tests_with_permissions(vec![
        "application.view.own",
        "application.create.all",
    ]);
    let mine = service
        .create_application(CreateApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Mine".into(),
            description: "mine".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    service.seed_foreign_application("Other App");

    let visible = service.list_applications(Uuid::nil()).await.unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, mine.id);
}

#[tokio::test]
async fn get_application_detail_returns_public_api_template_before_configuration() {
    let service = ApplicationService::for_tests();
    let created = service
        .create_application(CreateApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Detail".into(),
            description: "detail".into(),
            icon: Some("RobotOutlined".into()),
            icon_type: Some("iconfont".into()),
            icon_background: Some("#E6F7F2".into()),
        })
        .await
        .unwrap();

    let detail = service
        .get_application(Uuid::nil(), created.id)
        .await
        .unwrap();

    assert_eq!(detail.sections.orchestration.subject_kind, "agent_flow");
    assert_eq!(
        detail.sections.api.invoke_path_template.as_deref(),
        Some("/api/agent/v1/runs")
    );
    assert_eq!(detail.sections.api.api_capability_status, "not_published");
    assert_eq!(detail.sections.api.credentials_status, "missing");
    assert_eq!(detail.sections.logs.run_object_kind, "application_run");
    assert_eq!(
        detail.sections.monitoring.metrics_object_kind,
        "application_metrics"
    );
}

#[tokio::test]
async fn update_application_requires_edit_permission() {
    let service = ApplicationService::for_tests_with_permissions(vec![
        "application.view.own",
        "application.create.all",
    ]);
    let created = service
        .create_application(CreateApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Original".into(),
            description: "original".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();

    let error = service
        .update_application(UpdateApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_id: created.id,
            name: "Updated".into(),
            description: "updated".into(),
            tag_ids: Vec::new(),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn update_application_replaces_basic_metadata_and_tags() {
    let service = ApplicationService::for_tests_with_permissions(vec![
        "application.view.own",
        "application.create.all",
        "application.edit.own",
    ]);
    let created = service
        .create_application(CreateApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Original".into(),
            description: "original".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let tag = service
        .create_application_tag(CreateApplicationTagCommand {
            actor_user_id: Uuid::nil(),
            name: "客服".into(),
        })
        .await
        .unwrap();

    let updated = service
        .update_application(UpdateApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_id: created.id,
            name: "Updated".into(),
            description: "updated".into(),
            tag_ids: vec![tag.id],
        })
        .await
        .unwrap();

    assert_eq!(updated.name, "Updated");
    assert_eq!(updated.description, "updated");
    assert_eq!(updated.tags.len(), 1);
    assert_eq!(updated.tags[0].name, "客服");
}

#[tokio::test]
async fn delete_application_requires_delete_permission() {
    let service = ApplicationService::for_tests_with_permissions(vec![
        "application.view.own",
        "application.create.all",
        "application.edit.own",
    ]);
    let created = service
        .create_application(CreateApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Original".into(),
            description: "original".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();

    let error = service
        .delete_application(DeleteApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_id: created.id,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn delete_application_removes_visible_record_and_writes_audit_log() {
    let service = ApplicationService::for_tests_with_permissions(vec![
        "application.view.own",
        "application.create.all",
        "application.delete.own",
    ]);
    let created = service
        .create_application(CreateApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Disposable".into(),
            description: "delete me".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();

    service
        .delete_application(DeleteApplicationCommand {
            actor_user_id: Uuid::nil(),
            application_id: created.id,
        })
        .await
        .unwrap();

    let visible = service.list_applications(Uuid::nil()).await.unwrap();
    assert!(visible
        .iter()
        .all(|application| application.id != created.id));
    assert!(service
        .audit_events()
        .contains(&"application.deleted".to_string()));
}
