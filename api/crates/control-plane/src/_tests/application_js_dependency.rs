use control_plane::{
    application::{ApplicationService, CreateApplicationCommand},
    js_dependency::{
        ApplicationJsDependencyService, ReplaceApplicationJsDependencySelectionCommand,
    },
};
use domain::ApplicationType;
use uuid::Uuid;

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
