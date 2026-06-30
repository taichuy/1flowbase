use control_plane::ports::{
    CreatePluginAssignmentInput, NodeContributionRegistryInput, NodeContributionRepository,
    PluginRepository, ReplaceInstallationNodeContributionsInput, UpsertPluginInstallationInput,
};
use domain::{
    NodeContributionDependencyStatus, PluginArtifactStatus, PluginAvailabilityStatus,
    PluginDesiredState, PluginRuntimeStatus, PluginVerificationStatus,
};
use serde_json::json;
use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

async fn seed_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
) {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "1flowbase")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            name: "password-local".into(),
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root",
            "Root",
        )
        .await
        .unwrap();

    (store, workspace, actor)
}

async fn insert_installation(
    store: &PgControlPlaneStore,
    actor_id: Uuid,
    provider_code: &str,
    version: &str,
    enabled: bool,
) -> domain::PluginInstallationRecord {
    let desired_state = if enabled {
        PluginDesiredState::ActiveRequested
    } else {
        PluginDesiredState::Disabled
    };
    let availability_status = if enabled {
        PluginAvailabilityStatus::InstallIncomplete
    } else {
        PluginAvailabilityStatus::Disabled
    };
    PluginRepository::upsert_installation(
        store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: provider_code.into(),
            plugin_id: format!("{provider_code}@{version}"),
            plugin_version: version.into(),
            contract_version: "1flowbase.capability/v1".into(),
            protocol: "stdio_json".into(),
            display_name: format!("{provider_code} {version}"),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status,
            package_path: None,
            installed_path: format!("/tmp/plugins/{provider_code}/{version}"),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: actor_id,
        },
    )
    .await
    .unwrap()
}

async fn insert_installation_with_plugin_id(
    store: &PgControlPlaneStore,
    actor_id: Uuid,
    provider_code: &str,
    plugin_id: &str,
    version: &str,
    enabled: bool,
) -> domain::PluginInstallationRecord {
    let desired_state = if enabled {
        PluginDesiredState::ActiveRequested
    } else {
        PluginDesiredState::Disabled
    };
    let availability_status = if enabled {
        PluginAvailabilityStatus::InstallIncomplete
    } else {
        PluginAvailabilityStatus::Disabled
    };
    PluginRepository::upsert_installation(
        store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: provider_code.into(),
            plugin_id: plugin_id.into(),
            plugin_version: version.into(),
            contract_version: "1flowbase.capability/v1".into(),
            protocol: "stdio_json".into(),
            display_name: format!("{plugin_id} {version}"),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status,
            package_path: None,
            installed_path: format!("/tmp/plugins/{plugin_id}/{version}"),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: actor_id,
        },
    )
    .await
    .unwrap()
}

fn contribution_input(
    contribution_code: &str,
    version_range: &str,
) -> ReplaceInstallationNodeContributionsInput {
    ReplaceInstallationNodeContributionsInput {
        installation_id: Uuid::nil(),
        provider_code: String::new(),
        plugin_id: String::new(),
        plugin_version: String::new(),
        entries: vec![NodeContributionRegistryInput {
            plugin_unique_identifier: "prompt_pack".into(),
            package_id: "prompt_pack@0.1.0".into(),
            contribution_code: contribution_code.into(),
            node_shell: "action".into(),
            category: "ai".into(),
            title: format!("Title {contribution_code}"),
            description: format!("Description {contribution_code}"),
            icon: "spark".into(),
            schema_ui: json!({}),
            schema_version: "1flowbase.node-contribution/v2".into(),
            output_schema: json!({
                "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
            }),
            contribution_checksum: "sha256:contribution".into(),
            compiled_contribution_hash: "sha256:compiled".into(),
            output_schema_snapshot: json!({
                "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
            }),
            side_effect_policy: "external_read".into(),
            infra_contracts: vec![],
            required_auth: vec!["provider_instance".into()],
            visibility: "public".into(),
            experimental: false,
            dependency_installation_kind: "required".into(),
            dependency_plugin_version_range: version_range.into(),
        }],
    }
}

fn bind_contribution_input_to_installation(
    input: &mut ReplaceInstallationNodeContributionsInput,
    installation: &domain::PluginInstallationRecord,
) {
    input.installation_id = installation.id;
    input.provider_code = installation.provider_code.clone();
    input.plugin_id = installation.plugin_id.clone();
    input.plugin_version = installation.plugin_version.clone();

    for entry in &mut input.entries {
        entry.plugin_unique_identifier = installation.provider_code.clone();
        entry.package_id = installation.plugin_id.clone();
    }
}

#[tokio::test]
async fn node_contribution_repository_resolves_workspace_dependency_statuses() {
    let (store, workspace, actor) = seed_store().await;
    let ready = insert_installation(&store, actor.id, "ready_plugin", "0.2.0", true).await;
    let disabled = insert_installation(&store, actor.id, "disabled_plugin", "0.1.0", false).await;
    let missing = insert_installation(&store, actor.id, "missing_plugin", "0.3.0", true).await;
    let replaced_v1 = insert_installation_with_plugin_id(
        &store,
        actor.id,
        "replaced_plugin",
        "replaced_plugin_v1@0.1.0",
        "0.1.0",
        true,
    )
    .await;
    let replaced_v2 = insert_installation_with_plugin_id(
        &store,
        actor.id,
        "replaced_plugin",
        "replaced_plugin_v2@0.2.0",
        "0.2.0",
        true,
    )
    .await;

    let mut ready_input = contribution_input("ready_node", ">=0.2.0");
    bind_contribution_input_to_installation(&mut ready_input, &ready);
    NodeContributionRepository::replace_installation_node_contributions(&store, &ready_input)
        .await
        .unwrap();

    let mut disabled_input = contribution_input("disabled_node", ">=0.1.0");
    bind_contribution_input_to_installation(&mut disabled_input, &disabled);
    NodeContributionRepository::replace_installation_node_contributions(&store, &disabled_input)
        .await
        .unwrap();

    let mut missing_input = contribution_input("missing_node", ">=0.3.0");
    bind_contribution_input_to_installation(&mut missing_input, &missing);
    NodeContributionRepository::replace_installation_node_contributions(&store, &missing_input)
        .await
        .unwrap();

    let mut replaced_input = contribution_input("replaced_node", ">=0.1.0");
    bind_contribution_input_to_installation(&mut replaced_input, &replaced_v1);
    NodeContributionRepository::replace_installation_node_contributions(&store, &replaced_input)
        .await
        .unwrap();

    PluginRepository::create_assignment(
        &store,
        &CreatePluginAssignmentInput {
            installation_id: ready.id,
            workspace_id: workspace.id,
            provider_code: ready.provider_code.clone(),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();
    PluginRepository::create_assignment(
        &store,
        &CreatePluginAssignmentInput {
            installation_id: disabled.id,
            workspace_id: workspace.id,
            provider_code: disabled.provider_code.clone(),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();
    PluginRepository::create_assignment(
        &store,
        &CreatePluginAssignmentInput {
            installation_id: replaced_v2.id,
            workspace_id: workspace.id,
            provider_code: replaced_v2.provider_code.clone(),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    let entries = NodeContributionRepository::list_node_contributions(&store, workspace.id)
        .await
        .unwrap();
    let statuses = entries
        .into_iter()
        .map(|entry| (entry.contribution_code, entry.dependency_status))
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(statuses.len(), 4);
    assert_eq!(
        statuses.get("disabled_node"),
        Some(&NodeContributionDependencyStatus::DisabledPlugin)
    );
    assert_eq!(
        statuses.get("missing_node"),
        Some(&NodeContributionDependencyStatus::MissingPlugin)
    );
    assert_eq!(
        statuses.get("replaced_node"),
        Some(&NodeContributionDependencyStatus::MissingPlugin)
    );
    assert_eq!(
        statuses.get("ready_node"),
        Some(&NodeContributionDependencyStatus::Ready)
    );
}

#[tokio::test]
async fn node_contribution_registry_rejects_legacy_v2_hash_rows() {
    let (store, _workspace, actor) = seed_store().await;
    let ready = insert_installation(&store, actor.id, "ready_plugin", "0.2.0", true).await;

    let mut input = contribution_input("legacy_hash_node", ">=0.2.0");
    bind_contribution_input_to_installation(&mut input, &ready);
    input.entries[0].contribution_checksum = "sha256:legacy".into();

    let error = NodeContributionRepository::replace_installation_node_contributions(&store, &input)
        .await
        .expect_err("legacy placeholder checksums must not be accepted as v2 rows");

    assert!(error
        .to_string()
        .contains("node_contribution_registry_hash_not_legacy_check"));
}
