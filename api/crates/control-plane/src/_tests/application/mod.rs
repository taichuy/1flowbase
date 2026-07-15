use control_plane::application::{
    ApplicationNonCrudConsoleOperation, ApplicationService, CreateApplicationCommand,
    CreateApplicationTagCommand, DeleteApplicationCommand,
    ReplaceApplicationEnvironmentVariablesCommand, UpdateApplicationCommand,
};
use control_plane::ports::{
    ApplicationEnvironmentVariableInput, ApplicationManagementQuery,
    ApplicationManagementSortDirection, ApplicationManagementSortField,
};
use domain::ApplicationType;
use uuid::Uuid;

fn applications_group() -> domain::ConsolePolicyGroup {
    domain::ConsolePolicyGroup::settings_feature(
        access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID,
    )
    .expect("applications settings feature id must be valid")
}

fn operation_id(value: &str) -> domain::ConsoleOperationId {
    domain::ConsoleOperationId::try_from(value).expect("application operation id must be valid")
}

fn custom_policy(operations: Vec<domain::ConsoleOperationPolicy>) -> domain::RoleConsolePolicy {
    domain::RoleConsolePolicy::new(
        Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::custom(
            applications_group(),
            operations,
        )],
    )
}

fn create_command(actor_user_id: Uuid, name: &str) -> CreateApplicationCommand {
    CreateApplicationCommand {
        workflow_trigger_config: None,
        actor_user_id,
        application_type: ApplicationType::AgentFlow,
        workflow_trigger_type: None,
        name: name.into(),
        description: name.into(),
        icon: None,
        icon_type: None,
        icon_background: None,
    }
}

fn environment_variable(name: &str) -> ApplicationEnvironmentVariableInput {
    ApplicationEnvironmentVariableInput {
        name: name.into(),
        value_type: "string".into(),
        value: serde_json::json!("value"),
        description: String::new(),
    }
}

fn management_query() -> ApplicationManagementQuery {
    ApplicationManagementQuery {
        filter: domain::ResourceFilterExpr::all(Vec::new()),
        sort_field: ApplicationManagementSortField::UpdatedAt,
        sort_direction: ApplicationManagementSortDirection::Desc,
        page: 1,
        page_size: 20,
    }
}

#[tokio::test]
async fn ac_011_application_management_policy_only_allows_without_legacy_feature_grant() {
    let policy_only = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![domain::ConsoleOperationPolicy::simple(
            operation_id(access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION),
            true,
        )])],
    );

    let page = policy_only
        .list_application_management(Uuid::nil(), management_query())
        .await
        .unwrap();
    assert_eq!(page.total, 0);
}

#[tokio::test]
async fn ac_011_application_management_legacy_feature_grant_does_not_authorize() {
    let legacy_only = ApplicationService::for_tests_with_console_policies(
        vec![access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION],
        Vec::new(),
    );
    assert!(legacy_only
        .list_application_management(Uuid::nil(), management_query())
        .await
        .is_err());
}

#[tokio::test]
async fn ac_005_console_policy_defaults_to_deny_and_does_not_read_legacy_create_grant() {
    let service = ApplicationService::for_tests_with_console_policies(
        vec!["application.create.all"],
        Vec::new(),
    );

    let error = service
        .create_application(create_command(Uuid::nil(), "Blocked legacy grant"))
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn ac_005_console_policy_create_allows_without_legacy_permission_and_stamps_actor_scope() {
    let actor_user_id = Uuid::now_v7();
    let service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![domain::ConsoleOperationPolicy::simple(
            operation_id(access_control::APPLICATIONS_CREATE_OPERATION_ID),
            true,
        )])],
    );

    let created = service
        .create_application(create_command(actor_user_id, "Console policy create"))
        .await
        .unwrap();

    assert_eq!(created.workspace_id, Uuid::nil());
    assert_eq!(created.created_by, actor_user_id);
}

#[tokio::test]
async fn ac_005_console_policy_custom_simple_uses_allow_union_and_full_grants_group_profile() {
    let actor_user_id = Uuid::nil();
    let union_service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![
            custom_policy(vec![domain::ConsoleOperationPolicy::simple(
                operation_id(access_control::APPLICATIONS_CREATE_OPERATION_ID),
                false,
            )]),
            custom_policy(vec![domain::ConsoleOperationPolicy::simple(
                operation_id(access_control::APPLICATIONS_CREATE_OPERATION_ID),
                true,
            )]),
        ],
    );
    union_service
        .create_application(create_command(actor_user_id, "Union create"))
        .await
        .unwrap();

    let full_service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![domain::RoleConsolePolicy::new(
            Uuid::now_v7(),
            vec![domain::RoleConsoleGroupPolicy::full(applications_group())],
        )],
    );
    full_service.seed_foreign_application("Visible through full");
    assert_eq!(
        full_service
            .list_applications(actor_user_id)
            .await
            .unwrap()
            .len(),
        1
    );
    full_service
        .create_application(create_command(actor_user_id, "Full create"))
        .await
        .unwrap();
}

#[tokio::test]
async fn ac_005_console_policy_own_view_hides_same_workspace_other_owner() {
    let actor_user_id = Uuid::nil();
    let service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![domain::ConsoleOperationPolicy::row(
            operation_id(access_control::APPLICATIONS_VIEW_OPERATION_ID),
            domain::ConsoleOperationRowScope::Own,
        )])],
    );
    let mine = service.seed_application_for_actor(actor_user_id, "Mine");
    service.seed_foreign_application("Other App");

    let visible = service.list_applications(actor_user_id).await.unwrap();

    assert_eq!(
        visible
            .iter()
            .map(|application| application.id)
            .collect::<Vec<_>>(),
        vec![mine.id]
    );
}

#[tokio::test]
async fn ac_005_console_policy_multi_role_union_promotes_own_to_scope_all() {
    let actor_user_id = Uuid::nil();
    let service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![
            custom_policy(vec![domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_VIEW_OPERATION_ID),
                domain::ConsoleOperationRowScope::Own,
            )]),
            custom_policy(vec![domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_VIEW_OPERATION_ID),
                domain::ConsoleOperationRowScope::ScopeAll,
            )]),
        ],
    );
    service.seed_application_for_actor(actor_user_id, "Mine");
    service.seed_foreign_application("Other App");

    let visible = service.list_applications(actor_user_id).await.unwrap();

    assert_eq!(visible.len(), 2);
}

#[tokio::test]
async fn ac_005_console_policy_update_and_delete_use_real_owner() {
    let actor_user_id = Uuid::nil();
    let service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_UPDATE_OPERATION_ID),
                domain::ConsoleOperationRowScope::Own,
            ),
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_DELETE_OPERATION_ID),
                domain::ConsoleOperationRowScope::Own,
            ),
        ])],
    );
    let mine = service.seed_application_for_actor(actor_user_id, "Mine");
    let other = service.seed_foreign_application("Other App");

    let updated = service
        .update_application(UpdateApplicationCommand {
            actor_user_id,
            application_id: mine.id,
            name: "Updated".into(),
            description: "updated".into(),
            tag_ids: Vec::new(),
        })
        .await
        .unwrap();
    assert_eq!(updated.name, "Updated");

    let update_error = service
        .update_application(UpdateApplicationCommand {
            actor_user_id,
            application_id: other.id,
            name: "Forbidden".into(),
            description: "forbidden".into(),
            tag_ids: Vec::new(),
        })
        .await
        .unwrap_err();
    assert!(update_error.to_string().contains("permission_denied"));

    let delete_error = service
        .delete_application(DeleteApplicationCommand {
            actor_user_id,
            application_id: other.id,
        })
        .await
        .unwrap_err();
    assert!(delete_error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn ac_006_console_policy_root_keeps_application_crud_bypass() {
    let actor_user_id = Uuid::nil();
    let service = ApplicationService::for_tests_as_root();

    let created = service
        .create_application(create_command(actor_user_id, "Root application"))
        .await
        .unwrap();
    service
        .delete_application(DeleteApplicationCommand {
            actor_user_id,
            application_id: created.id,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn ac_006_console_policy_scope_all_cannot_cross_workspace() {
    let actor_user_id = Uuid::nil();
    let service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_VIEW_OPERATION_ID),
                domain::ConsoleOperationRowScope::ScopeAll,
            ),
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_UPDATE_OPERATION_ID),
                domain::ConsoleOperationRowScope::ScopeAll,
            ),
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_DELETE_OPERATION_ID),
                domain::ConsoleOperationRowScope::ScopeAll,
            ),
        ])],
    );
    let foreign =
        service.seed_application_in_workspace(Uuid::now_v7(), actor_user_id, "Other workspace");

    let error = service
        .get_application(actor_user_id, foreign.id)
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("resource not found: application"));

    let update_error = service
        .update_application(UpdateApplicationCommand {
            actor_user_id,
            application_id: foreign.id,
            name: "Blocked".into(),
            description: "blocked".into(),
            tag_ids: Vec::new(),
        })
        .await
        .unwrap_err();
    assert!(update_error
        .to_string()
        .contains("resource not found: application"));
    let delete_error = service
        .delete_application(DeleteApplicationCommand {
            actor_user_id,
            application_id: foreign.id,
        })
        .await
        .unwrap_err();
    assert!(delete_error
        .to_string()
        .contains("resource not found: application"));
}

#[tokio::test]
async fn ac_006_ac_007_non_crud_operations_ignore_crud_scope_but_keep_workspace_boundary() {
    let actor_user_id = Uuid::now_v7();
    let service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![
            domain::ConsoleOperationPolicy::simple(
                operation_id(access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID),
                true,
            ),
            domain::ConsoleOperationPolicy::simple(
                operation_id(access_control::APPLICATIONS_PUBLISH_OPERATION_ID),
                true,
            ),
            domain::ConsoleOperationPolicy::simple(
                operation_id(access_control::APPLICATIONS_RUN_OPERATION_ID),
                true,
            ),
            domain::ConsoleOperationPolicy::simple(
                operation_id(access_control::APPLICATIONS_LOGS_EXPORT_OPERATION_ID),
                true,
            ),
            domain::ConsoleOperationPolicy::simple(
                operation_id(access_control::APPLICATIONS_LOGS_IMPORT_OPERATION_ID),
                true,
            ),
            domain::ConsoleOperationPolicy::simple(
                operation_id(
                    access_control::APPLICATIONS_ORCHESTRATION_TEMPLATE_EXPORT_OPERATION_ID,
                ),
                true,
            ),
            domain::ConsoleOperationPolicy::simple(
                operation_id(
                    access_control::APPLICATIONS_ORCHESTRATION_VERSION_RESTORE_OPERATION_ID,
                ),
                true,
            ),
        ])],
    );
    let mine = service.seed_application_for_actor(actor_user_id, "Owned application");
    let peer = service.seed_foreign_application("Peer application");
    let cross_workspace =
        service.seed_application_in_workspace(Uuid::now_v7(), actor_user_id, "Foreign workspace");

    for operation in [
        ApplicationNonCrudConsoleOperation::ApiSetEnabled,
        ApplicationNonCrudConsoleOperation::Publish,
        ApplicationNonCrudConsoleOperation::Run,
        ApplicationNonCrudConsoleOperation::LogsExport,
        ApplicationNonCrudConsoleOperation::LogsImport,
        ApplicationNonCrudConsoleOperation::OrchestrationTemplateExport,
        ApplicationNonCrudConsoleOperation::OrchestrationVersionRestore,
    ] {
        service
            .load_application_for_non_crud_console_operation(actor_user_id, mine.id, operation)
            .await
            .expect("non-CRUD operation must allow an existing same-workspace application");

        service
            .load_application_for_non_crud_console_operation(actor_user_id, peer.id, operation)
            .await
            .expect("non-CRUD operation must not depend on CRUD owner scope");

        let cross_workspace_error = service
            .load_application_for_non_crud_console_operation(
                actor_user_id,
                cross_workspace.id,
                operation,
            )
            .await
            .expect_err("non-CRUD operation must not cross the current workspace");
        assert!(cross_workspace_error
            .to_string()
            .contains("resource not found: application"));
    }
}

#[tokio::test]
async fn ac_005_application_catalog_and_tags_use_create_policy_without_legacy_fallback() {
    let actor_user_id = Uuid::nil();
    let allowed = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![domain::ConsoleOperationPolicy::simple(
            operation_id(access_control::APPLICATIONS_CREATE_OPERATION_ID),
            true,
        )])],
    );

    let tag = allowed
        .create_application_tag(CreateApplicationTagCommand {
            actor_user_id,
            name: "Allowed tag".into(),
        })
        .await
        .unwrap();
    let catalog = allowed.list_application_tags(actor_user_id).await.unwrap();
    assert_eq!(catalog[0].id, tag.id);

    let legacy_only = ApplicationService::for_tests_with_console_policies(
        vec![
            "application.create.all",
            "application.view.all",
            "application.edit.all",
        ],
        Vec::new(),
    );
    assert!(legacy_only
        .list_application_tags(actor_user_id)
        .await
        .unwrap_err()
        .to_string()
        .contains("permission_denied"));
    assert!(legacy_only
        .create_application_tag(CreateApplicationTagCommand {
            actor_user_id,
            name: "Blocked legacy tag".into(),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("permission_denied"));
}

#[tokio::test]
async fn ac_005_application_environment_variables_enforce_real_owner_and_workspace() {
    let actor_user_id = Uuid::nil();
    let service = ApplicationService::for_tests_with_console_policies(
        vec!["application.view.all", "application.edit.all"],
        vec![custom_policy(vec![
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_VIEW_OPERATION_ID),
                domain::ConsoleOperationRowScope::Own,
            ),
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_UPDATE_OPERATION_ID),
                domain::ConsoleOperationRowScope::Own,
            ),
        ])],
    );
    let mine = service.seed_application_for_actor(actor_user_id, "Mine");
    let other = service.seed_foreign_application("Other owner");
    let cross_workspace =
        service.seed_application_in_workspace(Uuid::now_v7(), actor_user_id, "Other workspace");

    service
        .replace_application_environment_variables(ReplaceApplicationEnvironmentVariablesCommand {
            actor_user_id,
            application_id: mine.id,
            variables: vec![environment_variable("OwnValue")],
        })
        .await
        .unwrap();
    assert_eq!(
        service
            .list_application_environment_variables(actor_user_id, mine.id)
            .await
            .unwrap()
            .len(),
        1
    );

    for application_id in [other.id] {
        assert!(service
            .list_application_environment_variables(actor_user_id, application_id)
            .await
            .unwrap_err()
            .to_string()
            .contains("resource not found: application"));
        assert!(service
            .replace_application_environment_variables(
                ReplaceApplicationEnvironmentVariablesCommand {
                    actor_user_id,
                    application_id,
                    variables: vec![environment_variable("BlockedValue")],
                },
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("permission_denied"));
    }

    for result in [
        service
            .list_application_environment_variables(actor_user_id, cross_workspace.id)
            .await
            .map(|_| ()),
        service
            .replace_application_environment_variables(
                ReplaceApplicationEnvironmentVariablesCommand {
                    actor_user_id,
                    application_id: cross_workspace.id,
                    variables: vec![environment_variable("SpoofedScope")],
                },
            )
            .await
            .map(|_| ()),
    ] {
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("resource not found: application"));
    }
}

#[tokio::test]
async fn ac_005_application_environment_variables_scope_all_disabled_and_root_behave_consistently()
{
    let actor_user_id = Uuid::nil();
    let scope_all = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_VIEW_OPERATION_ID),
                domain::ConsoleOperationRowScope::ScopeAll,
            ),
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_UPDATE_OPERATION_ID),
                domain::ConsoleOperationRowScope::ScopeAll,
            ),
        ])],
    );
    let other = scope_all.seed_foreign_application("Other owner");
    scope_all
        .replace_application_environment_variables(ReplaceApplicationEnvironmentVariablesCommand {
            actor_user_id,
            application_id: other.id,
            variables: vec![environment_variable("ScopeAllValue")],
        })
        .await
        .unwrap();
    assert_eq!(
        scope_all
            .list_application_environment_variables(actor_user_id, other.id)
            .await
            .unwrap()
            .len(),
        1
    );

    let disabled = ApplicationService::for_tests_with_console_policies(
        vec!["application.view.all", "application.edit.all"],
        Vec::new(),
    );
    let disabled_application = disabled.seed_application_for_actor(actor_user_id, "Disabled");
    assert!(disabled
        .list_application_environment_variables(actor_user_id, disabled_application.id)
        .await
        .unwrap_err()
        .to_string()
        .contains("permission_denied"));
    assert!(disabled
        .replace_application_environment_variables(ReplaceApplicationEnvironmentVariablesCommand {
            actor_user_id,
            application_id: disabled_application.id,
            variables: vec![environment_variable("DisabledValue")],
        },)
        .await
        .unwrap_err()
        .to_string()
        .contains("permission_denied"));

    let root = ApplicationService::for_tests_as_root();
    let root_application = root.seed_foreign_application("Root bypass");
    root.replace_application_environment_variables(ReplaceApplicationEnvironmentVariablesCommand {
        actor_user_id,
        application_id: root_application.id,
        variables: vec![environment_variable("RootValue")],
    })
    .await
    .unwrap();
    assert_eq!(
        root.list_application_environment_variables(actor_user_id, root_application.id)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn create_application_requires_console_create_operation() {
    let service = ApplicationService::for_tests_with_permissions(vec!["application.view.own"]);

    let error = service
        .create_application(CreateApplicationCommand {
            workflow_trigger_config: None,
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
    let service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![
            domain::ConsoleOperationPolicy::simple(
                operation_id(access_control::APPLICATIONS_CREATE_OPERATION_ID),
                true,
            ),
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_VIEW_OPERATION_ID),
                domain::ConsoleOperationRowScope::Own,
            ),
        ])],
    );
    let mine = service
        .create_application(CreateApplicationCommand {
            workflow_trigger_config: None,
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
            workflow_trigger_config: None,
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
async fn update_application_requires_console_update_operation() {
    let service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![domain::ConsoleOperationPolicy::simple(
            operation_id(access_control::APPLICATIONS_CREATE_OPERATION_ID),
            true,
        )])],
    );
    let created = service
        .create_application(CreateApplicationCommand {
            workflow_trigger_config: None,
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
    let service = ApplicationService::for_tests_with_console_policies(
        vec!["application.edit.own"],
        vec![custom_policy(vec![
            domain::ConsoleOperationPolicy::simple(
                operation_id(access_control::APPLICATIONS_CREATE_OPERATION_ID),
                true,
            ),
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_UPDATE_OPERATION_ID),
                domain::ConsoleOperationRowScope::Own,
            ),
        ])],
    );
    let created = service
        .create_application(CreateApplicationCommand {
            workflow_trigger_config: None,
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
async fn delete_application_requires_console_delete_operation() {
    let service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![domain::ConsoleOperationPolicy::simple(
            operation_id(access_control::APPLICATIONS_CREATE_OPERATION_ID),
            true,
        )])],
    );
    let created = service
        .create_application(CreateApplicationCommand {
            workflow_trigger_config: None,
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
    let service = ApplicationService::for_tests_with_console_policies(
        Vec::new(),
        vec![custom_policy(vec![
            domain::ConsoleOperationPolicy::simple(
                operation_id(access_control::APPLICATIONS_CREATE_OPERATION_ID),
                true,
            ),
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_VIEW_OPERATION_ID),
                domain::ConsoleOperationRowScope::Own,
            ),
            domain::ConsoleOperationPolicy::row(
                operation_id(access_control::APPLICATIONS_DELETE_OPERATION_ID),
                domain::ConsoleOperationRowScope::Own,
            ),
        ])],
    );
    let created = service
        .create_application(CreateApplicationCommand {
            workflow_trigger_config: None,
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
