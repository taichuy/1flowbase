use control_plane_contracts::ports::*;
use domain::{
    PluginDesiredState, PluginRuntimeStatus, PluginVerificationStatus, UiCodeTemplateLanguage,
};
use serde_json::json;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

async fn store() -> PgControlPlaneStore {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let database = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    PgControlPlaneStore::new(pool)
}

async fn actor(store: &PgControlPlaneStore) -> Uuid {
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "settings-fixture")
        .await
        .unwrap();
    store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "fixture",
            "Root",
            "Root",
        )
        .await
        .unwrap()
        .id
}
async fn apply(store: &PgControlPlaneStore) -> anyhow::Result<()> {
    let target = store.list_native_plugin_targets().await?.pop().unwrap();
    store.apply_native_plugin_settings_templates(&target).await
}

fn input(actor: Uuid, version: &str, source: &str) -> CommitPluginInstallationInput {
    let mut input = installation_commit_input(Uuid::now_v7(), actor, "Settings", "native_host");
    input.installation.plugin_id = format!("northwind.settings-page@{version}");
    input.installation.plugin_version = version.to_string();
    input.artifact_instance.local_version = Some(version.to_string());
    input.settings_templates = vec![PluginSettingsTemplateInput {
        feature_id: "northwind.settings-page.settings".to_string(),
        contribution_code: "settings".to_string(),
        source: source.to_string(),
        language: UiCodeTemplateLanguage::Tsx,
    }];
    input
}

async fn desired(
    store: &PgControlPlaneStore,
    id: Uuid,
    actor: Uuid,
    state: PluginDesiredState,
) -> anyhow::Result<domain::PluginInstallationRecord> {
    store
        .update_desired_state(&UpdatePluginDesiredStateInput {
            installation_id: id,
            actor_user_id: actor,
            desired_state: state,
        })
        .await
}

#[tokio::test]
async fn root_2014_ac_010_template_trigger_matrix() {
    let store = store().await;
    let actor = actor(&store).await;
    let mut v1 = input(actor, "1.0.0", "export default () => <p>one</p>");
    v1.installation.metadata_json =
        json!({"business_configuration": "keep", "credential_reference": "secret://keep"});
    let first = store.commit_plugin_installation(&v1).await.unwrap();
    assert!(store.list_ui_code_templates(true).await.unwrap().is_empty());
    desired(&store, first.id, actor, PluginDesiredState::PendingRestart)
        .await
        .unwrap();
    assert!(store
        .list_ui_code_templates(false)
        .await
        .unwrap()
        .is_empty());
    apply(&store).await.unwrap();
    let created = store.list_ui_code_templates(false).await.unwrap().remove(0);
    assert_eq!(created.applied_plugin_version.as_deref(), Some("1.0.0"));
    assert_eq!(
        created.owner_plugin_code.as_deref(),
        Some("northwind.settings-page")
    );
    store
        .revise_ui_code_template(&ReviseUiCodeTemplateInput {
            template_id: created.id,
            name: "My page".into(),
            source: "user edited source".into(),
            language: UiCodeTemplateLanguage::Tsx,
            actor_user_id: actor,
        })
        .await
        .unwrap();
    // Startup retry/reinstall and same-version disable/re-enable preserve the edited source.
    apply(&store).await.unwrap();
    store.commit_plugin_installation(&v1).await.unwrap();
    desired(&store, first.id, actor, PluginDesiredState::Disabled)
        .await
        .unwrap();
    desired(&store, first.id, actor, PluginDesiredState::PendingRestart)
        .await
        .unwrap();
    apply(&store).await.unwrap();
    assert_eq!(
        store
            .get_ui_code_template(created.id)
            .await
            .unwrap()
            .unwrap()
            .latest_revision
            .source,
        "user edited source"
    );
    let unrelated = store
        .create_ui_code_template(&CreateUiCodeTemplateInput {
            provider_code: "other".into(),
            contribution_code: "settings".into(),
            name: "My page".into(),
            source: "unrelated".into(),
            language: UiCodeTemplateLanguage::Tsx,
            actor_user_id: actor,
        })
        .await
        .unwrap();
    let v2 = input(actor, "2.0.0", "export default () => <p>two</p>");
    let second = store.commit_plugin_installation(&v2).await.unwrap();
    desired(&store, second.id, actor, PluginDesiredState::PendingRestart)
        .await
        .unwrap();
    apply(&store).await.unwrap();
    let upgraded = store
        .get_ui_code_template(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        upgraded.latest_revision.source,
        v2.settings_templates[0].source
    );
    assert_eq!(upgraded.applied_plugin_version.as_deref(), Some("2.0.0"));
    assert_eq!(
        upgraded.published_revision.unwrap().source,
        v2.settings_templates[0].source
    );
    assert_eq!(
        store
            .get_ui_code_template(unrelated.id)
            .await
            .unwrap()
            .unwrap()
            .latest_revision
            .source,
        "unrelated"
    );
    assert_eq!(
        store
            .get_installation(first.id)
            .await
            .unwrap()
            .unwrap()
            .metadata_json,
        v1.installation.metadata_json
    );
}

#[tokio::test]
async fn root_2014_ac_011_atomic_version_retry_concurrency() {
    let store = store().await;
    let actor = actor(&store).await;
    let initial = input(actor, "1.0.0", "old");
    let first = store.commit_plugin_installation(&initial).await.unwrap();
    desired(&store, first.id, actor, PluginDesiredState::PendingRestart)
        .await
        .unwrap();
    apply(&store).await.unwrap();
    let old_target = store
        .list_native_plugin_targets()
        .await
        .unwrap()
        .pop()
        .unwrap();
    let id = store.list_ui_code_templates(false).await.unwrap()[0].id;
    sqlx::query("alter table ui_code_template_revisions add constraint controlled_apply_failure check (source <> 'reject')").execute(store.pool()).await.unwrap();
    let mut next = input(actor, "2.0.0", "new");
    next.settings_templates.push(PluginSettingsTemplateInput {
        feature_id: "northwind.settings-page.zz".into(),
        contribution_code: "zz".into(),
        source: "reject".into(),
        language: UiCodeTemplateLanguage::Tsx,
    });
    let installed = store.commit_plugin_installation(&next).await.unwrap();
    desired(
        &store,
        installed.id,
        actor,
        PluginDesiredState::PendingRestart,
    )
    .await
    .unwrap();
    assert!(apply(&store).await.is_err());
    assert_eq!(
        store
            .get_ui_code_template(id)
            .await
            .unwrap()
            .unwrap()
            .latest_revision
            .source,
        "old"
    );
    let target = store
        .list_native_plugin_targets()
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert!(!store
        .native_plugin_target_is_applied(&target)
        .await
        .unwrap());
    assert_eq!(
        store
            .get_installation(installed.id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::PendingRestart
    );
    assert!(store
        .apply_native_plugin_settings_templates(&old_target)
        .await
        .is_err());
    sqlx::query("alter table ui_code_template_revisions drop constraint controlled_apply_failure")
        .execute(store.pool())
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        store.apply_native_plugin_settings_templates(&target),
        store.apply_native_plugin_settings_templates(&target)
    );
    a.unwrap();
    b.unwrap();
    let applied = store.get_ui_code_template(id).await.unwrap().unwrap();
    assert_eq!(applied.latest_revision.source, "new");
    assert_eq!(applied.latest_revision.revision, 2);
    assert!(store
        .native_plugin_target_is_applied(&target)
        .await
        .unwrap());
    store
        .revise_ui_code_template(&ReviseUiCodeTemplateInput {
            template_id: id,
            name: "edited".into(),
            source: "after commit edit".into(),
            language: UiCodeTemplateLanguage::Tsx,
            actor_user_id: actor,
        })
        .await
        .unwrap();
    store
        .apply_native_plugin_settings_templates(&target)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_ui_code_template(id)
            .await
            .unwrap()
            .unwrap()
            .latest_revision
            .source,
        "after commit edit"
    );
    assert_eq!(store.list_ui_code_templates(false).await.unwrap().len(), 2);
}

fn installation_commit_input(
    installation_id: Uuid,
    actor_user_id: Uuid,
    title: &str,
    _runtime: &str,
) -> control_plane_contracts::ports::CommitPluginInstallationInput {
    use control_plane_contracts::ports::{
        CommitPluginInstallationInput, ReplaceInstallationFrontendBlocksInput,
        ReplaceInstallationJsDependenciesInput, ReplaceInstallationNodeContributionsInput,
        UpsertPluginArtifactInstanceInput,
    };

    CommitPluginInstallationInput {
        settings_templates: Vec::new(),
        installation: UpsertPluginInstallationInput {
            installation_id,
            category: domain::ExtensionCategory::HostExtensions,
            organization: "test".to_string(),
            provider_code: "northwind.settings-page".into(),
            plugin_id: "northwind.settings-page@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.host_extension/v1".into(),
            protocol: "native_host".into(),
            display_name: title.into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::Disabled,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({"block_contributions": ["hero_banner"]}),
            is_system_reserved: false,
            actor_user_id,
        },
        artifact_instance: UpsertPluginArtifactInstanceInput {
            node_id: "test-node".into(),
            installation_id,
            local_version: Some("0.1.0".into()),
            local_checksum: None,
            local_path: Some("/tmp/fixture_frontend_blocks/0.1.0".into()),
            package_path: None,
            manifest_fingerprint: None,
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: domain::PluginAvailabilityStatus::Disabled,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: None,
            is_current: true,
        },
        package_catalog: None,
        node_contributions: ReplaceInstallationNodeContributionsInput {
            installation_id,
            provider_code: "northwind.settings-page".into(),
            plugin_id: "northwind.settings-page@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            entries: Vec::new(),
        },
        js_dependencies: ReplaceInstallationJsDependenciesInput {
            installation_id,
            provider_code: "northwind.settings-page".into(),
            plugin_id: "northwind.settings-page@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            entries: Vec::new(),
        },
        frontend_blocks: ReplaceInstallationFrontendBlocksInput {
            installation_id,
            provider_code: "northwind.settings-page".into(),
            plugin_id: "northwind.settings-page@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            entries: Vec::new(),
        },
        retained_frontend_module_assets: Vec::new(),
    }
}
