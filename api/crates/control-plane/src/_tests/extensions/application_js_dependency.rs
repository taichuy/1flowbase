use control_plane::{
    application::{ApplicationService, CreateApplicationCommand},
    js_dependency::{
        ApplicationJsDependencyService, ReplaceApplicationJsDependencySelectionCommand,
    },
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

fn catalog_entry(
    installation_id: Uuid,
    package: &str,
    version: &str,
) -> domain::JsDependencyRegistryEntry {
    domain::JsDependencyRegistryEntry {
        installation_id,
        provider_code: "fixture_js_dependency_pack".into(),
        plugin_id: "fixture_js_dependency_pack@0.1.0".into(),
        plugin_version: "0.1.0".into(),
        alias: "zod".into(),
        package: package.into(),
        version: version.into(),
        target: "backend_code".into(),
        artifact_path: format!("artifacts/{package}.backend.mjs"),
        integrity: format!("sha256-{package}-{version}"),
        permissions: domain::JsDependencyPermissions {
            network: "outbound_only".into(),
            filesystem: "deny".into(),
            env: "deny".into(),
        },
    }
}

#[tokio::test]
async fn application_js_dependency_selection_snapshots_workspace_catalog_entry() {
    let app_service = ApplicationService::for_tests();
    let application = app_service
        .create_application(CreateApplicationCommand {
            workflow_trigger_config: None,
            actor_user_id: Uuid::nil(),
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Agent Support".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let installation_id = Uuid::now_v7();
    app_service.seed_js_dependency_catalog_entry(catalog_entry(installation_id, "zod", "3.24.0"));
    let service = ApplicationJsDependencyService::new(app_service.repository_for_tests());

    let selection = service
        .replace_application_js_dependency_selection(
            ReplaceApplicationJsDependencySelectionCommand {
                actor_user_id: Uuid::nil(),
                application_id: application.id,
                installation_id,
                alias: "zod".into(),
                target: "backend_code".into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(selection.application_id, application.id);
    assert_eq!(selection.alias, "zod");
    assert_eq!(selection.package, "zod");
    assert_eq!(selection.artifact_path, "artifacts/zod.backend.mjs");
    assert_eq!(selection.artifact_hash, "sha256-zod-3.24.0");
    assert_eq!(selection.integrity, "sha256-zod-3.24.0");
    assert_eq!(selection.permissions.network, "outbound_only");
}

#[tokio::test]
async fn application_js_dependency_selection_rejects_dependency_outside_workspace_catalog() {
    let app_service = ApplicationService::for_tests();
    let application = app_service
        .create_application(CreateApplicationCommand {
            workflow_trigger_config: None,
            actor_user_id: Uuid::nil(),
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Agent Support".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let service = ApplicationJsDependencyService::new(app_service.repository_for_tests());

    let error = service
        .replace_application_js_dependency_selection(
            ReplaceApplicationJsDependencySelectionCommand {
                actor_user_id: Uuid::nil(),
                application_id: application.id,
                installation_id: Uuid::now_v7(),
                alias: "zod".into(),
                target: "backend_code".into(),
            },
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("js_dependency"));
}

#[tokio::test]
async fn application_js_dependency_selection_replaces_existing_alias_target() {
    let app_service = ApplicationService::for_tests();
    let application = app_service
        .create_application(CreateApplicationCommand {
            workflow_trigger_config: None,
            actor_user_id: Uuid::nil(),
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Agent Support".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let first_installation_id = Uuid::now_v7();
    let second_installation_id = Uuid::now_v7();
    app_service.seed_js_dependency_catalog_entry(catalog_entry(
        first_installation_id,
        "zod",
        "3.24.0",
    ));
    app_service.seed_js_dependency_catalog_entry(catalog_entry(
        second_installation_id,
        "zod",
        "4.0.0",
    ));
    let service = ApplicationJsDependencyService::new(app_service.repository_for_tests());

    service
        .replace_application_js_dependency_selection(
            ReplaceApplicationJsDependencySelectionCommand {
                actor_user_id: Uuid::nil(),
                application_id: application.id,
                installation_id: first_installation_id,
                alias: "zod".into(),
                target: "backend_code".into(),
            },
        )
        .await
        .unwrap();
    service
        .replace_application_js_dependency_selection(
            ReplaceApplicationJsDependencySelectionCommand {
                actor_user_id: Uuid::nil(),
                application_id: application.id,
                installation_id: second_installation_id,
                alias: "zod".into(),
                target: "backend_code".into(),
            },
        )
        .await
        .unwrap();

    let selections = service
        .list_application_js_dependency_selections(Uuid::nil(), application.id)
        .await
        .unwrap();

    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].installation_id, second_installation_id);
    assert_eq!(selections[0].version, "4.0.0");
}

#[tokio::test]
async fn ac_005_application_js_dependencies_enforce_real_owner_and_workspace() {
    let actor_user_id = Uuid::nil();
    let app_service = ApplicationService::for_tests_with_console_policies(
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
    let mine = app_service.seed_application_for_actor(actor_user_id, "Mine");
    let other = app_service.seed_foreign_application("Other owner");
    let cross_workspace =
        app_service.seed_application_in_workspace(Uuid::now_v7(), actor_user_id, "Other workspace");
    let installation_id = Uuid::now_v7();
    app_service.seed_js_dependency_catalog_entry(catalog_entry(installation_id, "zod", "4.0.0"));
    let service = ApplicationJsDependencyService::new(app_service.repository_for_tests());

    service
        .replace_application_js_dependency_selection(
            ReplaceApplicationJsDependencySelectionCommand {
                actor_user_id,
                application_id: mine.id,
                installation_id,
                alias: "zod".into(),
                target: "backend_code".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .list_application_js_dependency_selections(actor_user_id, mine.id)
            .await
            .unwrap()
            .len(),
        1
    );

    for application_id in [other.id] {
        assert!(service
            .list_application_js_dependency_selections(actor_user_id, application_id)
            .await
            .unwrap_err()
            .to_string()
            .contains("resource not found: application"));
        assert!(service
            .replace_application_js_dependency_selection(
                ReplaceApplicationJsDependencySelectionCommand {
                    actor_user_id,
                    application_id,
                    installation_id,
                    alias: "zod".into(),
                    target: "backend_code".into(),
                },
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("permission_denied"));
    }

    for result in [
        service
            .list_application_js_dependency_selections(actor_user_id, cross_workspace.id)
            .await
            .map(|_| ()),
        service
            .replace_application_js_dependency_selection(
                ReplaceApplicationJsDependencySelectionCommand {
                    actor_user_id,
                    application_id: cross_workspace.id,
                    installation_id,
                    alias: "zod".into(),
                    target: "backend_code".into(),
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
async fn ac_005_application_js_dependencies_scope_all_disabled_and_root_behave_consistently() {
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
    let installation_id = Uuid::now_v7();
    scope_all.seed_js_dependency_catalog_entry(catalog_entry(installation_id, "zod", "4.0.0"));
    let service = ApplicationJsDependencyService::new(scope_all.repository_for_tests());
    service
        .replace_application_js_dependency_selection(
            ReplaceApplicationJsDependencySelectionCommand {
                actor_user_id,
                application_id: other.id,
                installation_id,
                alias: "zod".into(),
                target: "backend_code".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .list_application_js_dependency_selections(actor_user_id, other.id)
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
    let disabled_service = ApplicationJsDependencyService::new(disabled.repository_for_tests());
    assert!(disabled_service
        .list_application_js_dependency_selections(actor_user_id, disabled_application.id)
        .await
        .unwrap_err()
        .to_string()
        .contains("permission_denied"));

    let root = ApplicationService::for_tests_as_root();
    let root_application = root.seed_foreign_application("Root bypass");
    let root_installation_id = Uuid::now_v7();
    root.seed_js_dependency_catalog_entry(catalog_entry(root_installation_id, "zod", "4.0.0"));
    let root_service = ApplicationJsDependencyService::new(root.repository_for_tests());
    root_service
        .replace_application_js_dependency_selection(
            ReplaceApplicationJsDependencySelectionCommand {
                actor_user_id,
                application_id: root_application.id,
                installation_id: root_installation_id,
                alias: "zod".into(),
                target: "backend_code".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        root_service
            .list_application_js_dependency_selections(actor_user_id, root_application.id)
            .await
            .unwrap()
            .len(),
        1
    );
}
