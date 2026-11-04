use control_plane::flow::{FlowService, InMemoryFlowRepository, SaveFlowDraftCommand};
use control_plane::ports::FlowRepository;
use domain::{ApplicationType, FlowChangeKind};
use orchestration_runtime::compiler::FlowValidationError;
use serde_json::json;
use uuid::Uuid;

fn repository() -> InMemoryFlowRepository {
    InMemoryFlowRepository::with_console_policies(vec![domain::RoleConsolePolicy::new(
        Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::full(
            domain::ConsolePolicyGroup::settings_feature(
                access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID,
            )
            .unwrap(),
        )],
    )])
}

#[tokio::test]
async fn restoring_a_version_still_requires_restore_permission() {
    let repository =
        InMemoryFlowRepository::with_console_policies(vec![domain::RoleConsolePolicy::new(
            Uuid::now_v7(),
            vec![domain::RoleConsoleGroupPolicy::custom(
                domain::ConsolePolicyGroup::settings_feature(
                    access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID,
                )
                .unwrap(),
                vec![
                    domain::ConsoleOperationPolicy::simple(
                        domain::ConsoleOperationId::try_from("create_application").unwrap(),
                        true,
                    ),
                    domain::ConsoleOperationPolicy::row(
                        domain::ConsoleOperationId::try_from(
                            access_control::APPLICATIONS_VIEW_OPERATION_ID,
                        )
                        .unwrap(),
                        domain::ConsoleOperationRowScope::Own,
                    ),
                ],
            )],
        )]);
    let service = FlowService::new(repository);
    let actor = Uuid::now_v7();
    let app = service
        .seed_application_with_type_for_actor(
            actor,
            "Permission fixture",
            ApplicationType::Workflow,
        )
        .await
        .unwrap();
    let before = service
        .get_or_create_editor_state(actor, app.id)
        .await
        .unwrap();
    let error = service
        .restore_version(actor, app.id, before.versions[0].id)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(control_plane::errors::ControlPlaneError::PermissionDenied(
            _
        ))
    ));
    assert_eq!(
        service
            .get_or_create_editor_state(actor, app.id)
            .await
            .unwrap(),
        before
    );
}

#[tokio::test]
async fn invalid_start_outputs_do_not_change_draft_or_history() {
    for application_type in [ApplicationType::AgentFlow, ApplicationType::Workflow] {
        let service = FlowService::new(repository());
        let actor = Uuid::now_v7();
        let app = service
            .seed_application_with_type_for_actor(actor, "Contract fixture", application_type)
            .await
            .unwrap();
        let before = service
            .get_or_create_editor_state(actor, app.id)
            .await
            .unwrap();
        let mut document = before.draft.document.clone();
        document["graph"]["nodes"][0]["outputs"] =
            json!([{"key":"name","title":"Name","valueType":"string"}]);
        let error = service
            .save_draft(SaveFlowDraftCommand {
                actor_user_id: actor,
                application_id: app.id,
                document,
                change_kind: FlowChangeKind::Logical,
                summary: "invalid boundary".into(),
            })
            .await
            .unwrap_err();
        let diagnostic = &error
            .downcast_ref::<FlowValidationError>()
            .unwrap()
            .diagnostics[0];
        assert_eq!(diagnostic.field_path, "/outputs");
        assert_eq!(diagnostic.expected, Some(json!([])));
        assert_eq!(
            service
                .get_or_create_editor_state(actor, app.id)
                .await
                .unwrap(),
            before
        );
    }
}

#[tokio::test]
async fn incomplete_workflow_draft_preserves_start_inputs_and_selectors() {
    let service = FlowService::new(repository());
    let actor = Uuid::now_v7();
    let app = service
        .seed_application_with_type_for_actor(actor, "Contract fixture", ApplicationType::Workflow)
        .await
        .unwrap();
    let initial = service
        .get_or_create_editor_state(actor, app.id)
        .await
        .unwrap();
    let mut document = initial.draft.document.clone();
    document["graph"]["nodes"][0]["config"]["input_fields"] = json!([
        {"key":"name","label":"Name","inputType":"text","valueType":"string","required":true}
    ]);
    document["graph"]["nodes"][1]["bindings"]["name"] =
        json!({"kind":"selector","value":["node-workflow-start","name"]});
    document["graph"]["edges"] = json!([]);
    let state = service
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: actor,
            application_id: app.id,
            document: document.clone(),
            change_kind: FlowChangeKind::Logical,
            summary: "incomplete authoring".into(),
        })
        .await
        .unwrap();
    assert_eq!(state.draft.document, document);
    let restored = service
        .restore_version(actor, app.id, initial.versions[0].id)
        .await
        .unwrap();
    assert_eq!(restored.draft.document, initial.draft.document);
    assert_eq!(restored.versions.len(), state.versions.len() + 1);
}

#[tokio::test]
async fn restoring_legacy_invalid_boundary_outputs_is_rejected_without_writes() {
    let repository = repository();
    let service = FlowService::new(repository.clone());
    let actor = Uuid::now_v7();
    let app = service
        .seed_application_with_type_for_actor(actor, "Contract fixture", ApplicationType::Workflow)
        .await
        .unwrap();
    let clean = service
        .get_or_create_editor_state(actor, app.id)
        .await
        .unwrap();
    let mut invalid = clean.draft.document.clone();
    invalid["graph"]["nodes"][0]["outputs"] = json!([{"key":"legacy"}]);
    // Seed pre-fix history through the repository, without passing the guarded business command.
    let old = FlowRepository::save_draft(
        &repository,
        Uuid::nil(),
        app.id,
        actor,
        invalid,
        FlowChangeKind::Logical,
        "legacy fixture",
    )
    .await
    .unwrap();
    let version_id = old.versions.last().unwrap().id;
    let before = service
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: actor,
            application_id: app.id,
            document: clean.draft.document,
            change_kind: FlowChangeKind::Logical,
            summary: "repair current draft".into(),
        })
        .await
        .unwrap();
    let error = service
        .restore_version(actor, app.id, version_id)
        .await
        .unwrap_err();
    assert!(error.downcast_ref::<FlowValidationError>().is_some());
    assert_eq!(
        service
            .get_or_create_editor_state(actor, app.id)
            .await
            .unwrap(),
        before
    );
}
