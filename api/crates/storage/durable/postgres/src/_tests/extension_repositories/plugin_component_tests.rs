//! Durable installation/catalog fixture: production commit transaction, no fake repositories.
use super::*;
use control_plane_contracts::ports::{
    CommitPluginFamilyUninstallInput, CreateUiComponentRecordInput,
    FrontendBlockCatalogRegistryInput, FrontendBlockCatalogRepository, ListUiComponentRecordsQuery,
    UiComponentRecordPatch,
};
use domain::{UiComponentRecordOrigin, UiComponentRecordUpstream};

fn components(actor: Uuid, version: &str) -> CommitPluginInstallationInput {
    let mut command = input(actor, version, "export default () => null");
    command.settings_templates.clear();
    command.installation.category = domain::ExtensionCategory::RuntimeExtensions;
    command.installation.contract_version = "1flowbase.extension-bus/v1".into();
    command.installation.protocol = "stdio_json_multiplex_v1".into();
    command.frontend_blocks.plugin_id = command.installation.plugin_id.clone();
    command.node_contributions.plugin_id = command.installation.plugin_id.clone();
    command.node_contributions.plugin_version = version.into();
    command.js_dependencies.plugin_id = command.installation.plugin_id.clone();
    command.js_dependencies.plugin_version = version.into();
    command.frontend_blocks.plugin_version = version.into();
    command.frontend_blocks.entries = (0..6)
        .map(|index| FrontendBlockCatalogRegistryInput {
            contribution_code: format!("view-{index}"),
            title: format!("View {index}"),
            runtime: "native_react".into(),
            entry: format!("View{index}"),
            code_template: Some(format!(
                "export default function View{index}() {{ return <p>{version}</p>; }}"
            )),
            code_template_version: Some(version.into()),
            code_template_language: Some("tsx".into()),
            // A complete source may need no imports; Plugin is the only origin allowing this.
            code_modules: Vec::new(),
            context_contract: domain::FrontendBlockContextContract {
                primitives: Vec::new(),
                input_schema: json!({}),
            },
            permissions: domain::FrontendBlockPermissions {
                network: "none".into(),
                storage: "none".into(),
                secrets: "none".into(),
            },
            ui_capabilities: Vec::new(),
        })
        .collect();
    command
}

#[tokio::test]
async fn installation_projects_six_owned_components_idempotently_and_upgrades_in_place() {
    let store = store().await;
    let actor = actor(&store).await;
    let first = components(actor, "1.0.0");
    let installed = store.commit_plugin_installation(&first).await.unwrap();
    let records = store.list_ui_component_records().await.unwrap();
    assert_eq!(records.len(), 6);
    for record in &records {
        assert_eq!(record.origin, UiComponentRecordOrigin::Plugin);
        assert_eq!(
            record.owner_plugin_id.as_deref(),
            Some("test/northwind.settings-page")
        );
        assert_eq!(record.owner_installation_id, Some(installed.id));
        assert!(record.import_code.starts_with("// Plugin component: "));
        assert!(record
            .source_code
            .starts_with("export default function View"));
        assert_eq!(record.version, "1.0.0");
    }
    store.commit_plugin_installation(&first).await.unwrap();
    let retry = store.list_ui_component_records().await.unwrap();
    assert_eq!(
        retry.iter().map(|record| record.id).collect::<Vec<_>>(),
        records.iter().map(|record| record.id).collect::<Vec<_>>()
    );

    let mut upgrade = components(actor, "2.0.0");
    upgrade.frontend_blocks.entries[0]
        .code_modules
        .push(domain::FrontendBlockCodeModule {
            source: "@fixture/views".into(),
            version: "2.0.0".into(),
            exports: vec!["View0".into()],
            binding: domain::FrontendModuleBinding::Host,
            assets: Vec::new(),
            type_declarations: String::new(),
        });
    let installed_v2 = store.commit_plugin_installation(&upgrade).await.unwrap();
    let upgraded = store.list_ui_component_records().await.unwrap();
    assert_eq!(upgraded.len(), 6);
    for (before, after) in records.iter().zip(&upgraded) {
        assert_eq!(before.id, after.id);
        assert_eq!(before.component_code, after.component_code);
        assert_eq!(after.owner_installation_id, Some(installed_v2.id));
        assert_eq!(after.version, "2.0.0");
        assert!(after.source_code.contains("2.0.0"));
    }
    assert!(upgraded[0].import_code.starts_with("// Plugin component: "));
    assert!(!upgraded[0].import_code.contains("import {"));
    upgrade.frontend_blocks.entries.remove(0);
    store.commit_plugin_installation(&upgrade).await.unwrap();
    let remaining = store.list_ui_component_records().await.unwrap();
    assert_eq!(remaining.len(), 5);
    assert!(!remaining.iter().any(|record| record.id == records[0].id));
    // Catalog replacement is also an atomic projection owner (not just initial installation).
    upgrade.frontend_blocks.entries.clear();
    upgrade.frontend_blocks.installation_id = installed_v2.id;
    store
        .replace_installation_frontend_blocks(&upgrade.frontend_blocks)
        .await
        .unwrap();
    assert!(store.list_ui_component_records().await.unwrap().is_empty());
}

#[tokio::test]
async fn plugin_components_are_read_only_and_workspace_page_filters_exact_contributions() {
    let store = store().await;
    let actor = actor(&store).await;
    let installed = store
        .commit_plugin_installation(&components(actor, "1.0.0"))
        .await
        .unwrap();
    let records = store.list_ui_component_records().await.unwrap();
    let record = &records[0];
    assert!(store
        .update_ui_component_record(
            record.id,
            &UiComponentRecordPatch {
                name: "Hijacked".into(),
                description: record.description.clone(),
                import_code: "import X from 'x';".into(),
                source_code: "<X />".into(),
                source: record.source.clone(),
                group: record.group.clone(),
                upstream: record.upstream.clone(),
                version: record.version.clone(),
                keywords: record.keywords.clone(),
                actor_user_id: actor,
            }
        )
        .await
        .is_err());
    assert!(!store.delete_ui_component_record(record.id).await.unwrap());
    let page = |ids| ListUiComponentRecordsQuery {
        plugin_contribution_ids: Some(ids),
        query: None,
        offset: 0,
        limit: 20,
    };
    assert_eq!(
        store
            .list_ui_component_records_page(page(Vec::new()))
            .await
            .unwrap()
            .total,
        0
    );
    let allowed = format!(
        "{}:{}",
        installed.id,
        record.owner_contribution_code.as_deref().unwrap()
    );
    let visible = store
        .list_ui_component_records_page(page(vec![allowed]))
        .await
        .unwrap();
    assert_eq!(visible.total, 1);
    assert_eq!(visible.items[0].id, record.id);
    assert!(!visible.has_more);
    desired(&store, installed.id, actor, PluginDesiredState::Disabled)
        .await
        .unwrap();
    // Management retains disabled read-only ownership. Workspace access supplies an empty
    // resolver-authorized set; the service fixture covers deriving that set from lifecycle.
    assert_eq!(store.list_ui_component_records().await.unwrap().len(), 6);
    assert_eq!(
        store
            .list_ui_component_records_page(page(Vec::new()))
            .await
            .unwrap()
            .total,
        0
    );
}

#[tokio::test]
async fn custom_and_official_owner_conflicts_roll_back_installation_and_catalog() {
    let store = store().await;
    let actor = actor(&store).await;
    let original = components(actor, "1.0.0");
    let installed = store.commit_plugin_installation(&original).await.unwrap();
    let record = store.list_ui_component_records().await.unwrap().remove(0);
    let mut empty = original.frontend_blocks.clone();
    empty.installation_id = installed.id;
    empty.entries.clear();
    store
        .replace_installation_frontend_blocks(&empty)
        .await
        .unwrap();
    let custom = store
        .create_ui_component_record(&CreateUiComponentRecordInput {
            component_code: record.component_code,
            name: "Custom owner".into(),
            description: "Must survive".into(),
            import_code: "import Own from 'own';".into(),
            source_code: "<Own />".into(),
            source: "local".into(),
            group: "custom".into(),
            upstream: UiComponentRecordUpstream {
                identity: "own".into(),
                version: "1.0.0".into(),
            },
            version: "1.0.0".into(),
            keywords: Vec::new(),
            actor_user_id: actor,
        })
        .await
        .unwrap();
    for origin in ["custom", "official"] {
        // Official setup is fixture-only; real projection must reject either pre-existing owner.
        sqlx::query("update ui_component_records set origin=$1 where id=$2")
            .bind(origin)
            .bind(custom.id)
            .execute(store.pool())
            .await
            .unwrap();
        let before = store
            .get_ui_component_record(custom.id)
            .await
            .unwrap()
            .unwrap();
        let rejected = components(actor, "2.0.0");
        assert!(store.commit_plugin_installation(&rejected).await.is_err());
        assert!(store
            .get_installation(rejected.installation.installation_id)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            store.list_ui_component_records().await.unwrap(),
            vec![before]
        );
        let count: i64 = sqlx::query_scalar(
            "select count(*) from frontend_block_catalog where installation_id=$1",
        )
        .bind(rejected.installation.installation_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(count, 0);
    }
}

#[tokio::test]
async fn uninstall_revokes_projection_but_retains_installation_history() {
    let store = store().await;
    let actor = actor(&store).await;
    let mut command = components(actor, "1.0.0");
    let installed = store.commit_plugin_installation(&command).await.unwrap();
    command.artifact_instance.installation_id = installed.id;
    command.artifact_instance.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
    command.artifact_instance.availability_status =
        domain::PluginAvailabilityStatus::ArtifactMissing;
    command.artifact_instance.local_path = None;
    store
        .commit_plugin_family_uninstall(&CommitPluginFamilyUninstallInput {
            artifact_instances: vec![command.artifact_instance],
            artifact_cleanups: Vec::new(),
            audit_log: domain::AuditLogRecord {
                id: Uuid::now_v7(),
                workspace_id: None,
                actor_user_id: Some(actor),
                target_type: "plugin_family".into(),
                target_id: None,
                event_code: "plugin.family_uninstalled".into(),
                payload: json!({}),
                created_at: time::OffsetDateTime::now_utc(),
            },
        })
        .await
        .unwrap();
    assert!(store.list_ui_component_records().await.unwrap().is_empty());
    assert!(store
        .get_installation(installed.id)
        .await
        .unwrap()
        .is_some());
    let count: i64 =
        sqlx::query_scalar("select count(*) from frontend_block_catalog where installation_id=$1")
            .bind(installed.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(count, 0);
}

async fn uninstall_components(
    store: &PgControlPlaneStore,
    mut command: CommitPluginInstallationInput,
    installation_id: Uuid,
    actor: Uuid,
) {
    command.artifact_instance.installation_id = installation_id;
    command.artifact_instance.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
    command.artifact_instance.availability_status =
        domain::PluginAvailabilityStatus::ArtifactMissing;
    command.artifact_instance.local_path = None;
    store
        .commit_plugin_family_uninstall(&CommitPluginFamilyUninstallInput {
            artifact_instances: vec![command.artifact_instance],
            artifact_cleanups: Vec::new(),
            audit_log: domain::AuditLogRecord {
                id: Uuid::now_v7(),
                workspace_id: None,
                actor_user_id: Some(actor),
                target_type: "plugin_family".into(),
                target_id: None,
                event_code: "plugin.family_uninstalled".into(),
                payload: json!({}),
                created_at: time::OffsetDateTime::now_utc(),
            },
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn publishers_with_same_artifact_and_different_versions_cannot_overwrite_or_delete_each_other(
) {
    let store = store().await;
    let actor = actor(&store).await;
    let mut publisher_a = components(actor, "1.0.0");
    publisher_a.installation.organization = "publisher-a".into();
    let installed_a = store
        .commit_plugin_installation(&publisher_a)
        .await
        .unwrap();
    let before_a = store.list_ui_component_records().await.unwrap();
    assert_eq!(before_a.len(), 6);

    // Different versions deliberately avoid the unrelated same-version checksum admission gate.
    let mut publisher_b = components(actor, "2.0.0");
    publisher_b.installation.organization = "publisher-b".into();
    publisher_b.frontend_blocks.entries.truncate(3);
    let installed_b = store
        .commit_plugin_installation(&publisher_b)
        .await
        .unwrap();
    let both = store.list_ui_component_records().await.unwrap();
    assert_eq!(both.len(), 9);
    for before in &before_a {
        assert_eq!(both.iter().find(|record| record.id == before.id), Some(before),
            "another publisher must not transfer ownership, overwrite source or delete missing contributions");
    }
    let before_b: Vec<_> = both
        .into_iter()
        .filter(|record| record.owner_installation_id == Some(installed_b.id))
        .collect();
    assert_eq!(before_b.len(), 3);
    for record in &before_b {
        assert_eq!(
            record.owner_plugin_id.as_deref(),
            Some("publisher-b/northwind.settings-page")
        );
        assert!(!before_a
            .iter()
            .any(|other| other.component_code == record.component_code));
    }

    let mut upgrade_a = components(actor, "3.0.0");
    upgrade_a.installation.organization = "publisher-a".into();
    upgrade_a.frontend_blocks.entries.pop();
    let installed_a3 = store.commit_plugin_installation(&upgrade_a).await.unwrap();
    let upgraded = store.list_ui_component_records().await.unwrap();
    assert_eq!(upgraded.len(), 8);
    for before in &before_a[..5] {
        let after = upgraded
            .iter()
            .find(|record| record.id == before.id)
            .unwrap();
        assert_eq!(after.component_code, before.component_code);
        assert_eq!(
            after.owner_plugin_id.as_deref(),
            Some("publisher-a/northwind.settings-page")
        );
        assert_eq!(after.owner_installation_id, Some(installed_a3.id));
        assert_eq!(after.version, "3.0.0");
    }
    for before in &before_b {
        assert_eq!(
            upgraded.iter().find(|record| record.id == before.id),
            Some(before)
        );
    }
    let mut empty_b = publisher_b.frontend_blocks.clone();
    empty_b.installation_id = installed_b.id;
    empty_b.entries.clear();
    store
        .replace_installation_frontend_blocks(&empty_b)
        .await
        .unwrap();
    let remaining_a = store.list_ui_component_records().await.unwrap();
    assert_eq!(remaining_a.len(), 5);
    assert!(remaining_a
        .iter()
        .all(|record| record.owner_installation_id == Some(installed_a3.id)));
    store
        .commit_plugin_installation(&publisher_b)
        .await
        .unwrap();
    // Uninstalling an old version cannot delete its replacement or another publisher's records.
    uninstall_components(&store, publisher_a, installed_a.id, actor).await;
    assert_eq!(store.list_ui_component_records().await.unwrap().len(), 8);
    uninstall_components(&store, upgrade_a, installed_a3.id, actor).await;
    let remaining_b = store.list_ui_component_records().await.unwrap();
    assert_eq!(remaining_b.len(), 3);
    assert!(remaining_b
        .iter()
        .all(|record| record.owner_installation_id == Some(installed_b.id)));
}

#[tokio::test]
async fn publisher_owner_migration_preserves_record_ids_and_matches_install_projection_keys() {
    use sha2::{Digest, Sha256};
    let store = store().await;
    let actor = actor(&store).await;
    let command = components(actor, "1.0.0");
    let installed = store.commit_plugin_installation(&command).await.unwrap();
    let expected = store.list_ui_component_records().await.unwrap();
    for record in &expected {
        let contribution = record.owner_contribution_code.as_deref().unwrap();
        let identity = serde_json::to_vec(&("northwind.settings-page", contribution)).unwrap();
        let legacy_code = format!(
            "plugin.{:x}",
            base16ct::HexDisplay(&Sha256::digest(identity))
        );
        sqlx::query("update ui_component_records set owner_plugin_id='northwind.settings-page', component_code=$1 where id=$2")
            .bind(legacy_code).bind(record.id).execute(store.pool()).await.unwrap();
    }
    sqlx::raw_sql(include_str!(
        "../../../migrations/20261101100000_publisher_owned_ui_components.sql"
    ))
    .execute(store.pool())
    .await
    .unwrap();
    let migrated = store.list_ui_component_records().await.unwrap();
    assert_eq!(migrated, expected);
    store.commit_plugin_installation(&command).await.unwrap();
    let retried = store.list_ui_component_records().await.unwrap();
    assert_eq!(retried.len(), 6);
    for before in &expected {
        let after = retried
            .iter()
            .find(|record| record.id == before.id)
            .unwrap();
        assert_eq!(after.component_code, before.component_code);
        assert_eq!(after.owner_installation_id, Some(installed.id));
    }
}
