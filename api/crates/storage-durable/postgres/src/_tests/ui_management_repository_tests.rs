use control_plane::ports::{
    CreateUiCodeTemplateInput, CreateUiComponentRecordInput, OfficialUiComponentCatalogRecord,
    ReviseUiCodeTemplateInput, UiComponentCatalogRepository, UiComponentRecordPatch,
    UiManagementRepository,
};
use domain::{UiCodeTemplateLanguage, UiComponentRecordOrigin, UiComponentRecordUpstream};
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn store() -> PgControlPlaneStore {
    let database = postgres_test_support::PostgresTestSchema::create(&database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    PgControlPlaneStore::new(pool)
}

#[tokio::test]
async fn ac_002_template_revisions_publish_and_default_are_transactional() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let created = store
        .create_ui_code_template(&CreateUiCodeTemplateInput {
            provider_code: "1flowbase".into(),
            contribution_code: "frontstage.js-ui-block".into(),
            name: "Dashboard".into(),
            source: "export default function Block() { return null }".into(),
            language: UiCodeTemplateLanguage::Tsx,
            actor_user_id: actor,
        })
        .await
        .unwrap();
    assert_eq!(created.latest_revision.revision, 1);
    assert!(store
        .set_ui_code_template_default(created.id, actor)
        .await
        .is_err());

    let revised = store
        .revise_ui_code_template(&ReviseUiCodeTemplateInput {
            template_id: created.id,
            name: "Dashboard".into(),
            source: "export default function Block() { return <main /> }".into(),
            language: UiCodeTemplateLanguage::Tsx,
            actor_user_id: actor,
        })
        .await
        .unwrap();
    assert_eq!(revised.latest_revision.revision, 2);
    let published = store
        .publish_ui_code_template_revision(created.id, 2, actor)
        .await
        .unwrap();
    assert_eq!(published.published_revision.unwrap().revision, 2);
    store
        .set_ui_code_template_default(created.id, actor)
        .await
        .unwrap();
    assert!(
        store
            .get_ui_code_template(created.id)
            .await
            .unwrap()
            .unwrap()
            .is_default
    );
}

#[tokio::test]
async fn wp_d2_component_records_are_system_scoped_crud_with_official_write_protection() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let created = store
        .create_ui_component_record(&CreateUiComponentRecordInput {
            component_code: "local.status-panel".into(),
            name: "Status panel".into(),
            description: "Shows a status".into(),
            import_code: "import StatusPanel from './StatusPanel';".into(),
            source_code: "<StatusPanel />".into(),
            source: "local".into(),
            group: "operations".into(),
            upstream: UiComponentRecordUpstream {
                identity: "@local/status-panel".into(),
                version: "0.1.0".into(),
            },
            version: "1.0.0".into(),
            keywords: vec!["status".into()],
            actor_user_id: actor,
        })
        .await
        .unwrap();
    assert_eq!(created.origin, UiComponentRecordOrigin::Custom);
    assert_eq!(created.scope_id, domain::SYSTEM_SCOPE_ID);

    let updated = store
        .update_ui_component_record(
            created.id,
            &UiComponentRecordPatch {
                name: "System status panel".into(),
                description: created.description.clone(),
                import_code: created.import_code.clone(),
                source_code: created.source_code.clone(),
                source: created.source.clone(),
                group: created.group.clone(),
                upstream: created.upstream.clone(),
                version: "1.1.0".into(),
                keywords: created.keywords.clone(),
                actor_user_id: actor,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.component_code, created.component_code);
    assert_eq!(updated.name, "System status panel");
    assert!(store.delete_ui_component_record(created.id).await.unwrap());
    assert!(store
        .get_ui_component_record(created.id)
        .await
        .unwrap()
        .is_none());

    let official_id = Uuid::now_v7();
    sqlx::query("insert into ui_component_records (id, scope_id, component_code, name, description, import_code, source_code, origin, source, \"group\", upstream_identity, upstream_version, version, keywords, created_by, updated_by) values ($1,$2,'official.button','Button','Official button','opaque import','opaque source','official','taichuy','ant-design-x','@ant-design/x','2.9.0','1.0.0',array['action'],$3,$3)")
        .bind(official_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(actor)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .update_ui_component_record(
            official_id,
            &UiComponentRecordPatch {
                name: "Changed".into(),
                description: "Changed".into(),
                import_code: "opaque".into(),
                source_code: "opaque".into(),
                source: "taichuy".into(),
                group: "ant-design-x".into(),
                upstream: UiComponentRecordUpstream {
                    identity: "@ant-design/x".into(),
                    version: "2.9.0".into(),
                },
                version: "1.0.1".into(),
                keywords: vec!["action".into()],
                actor_user_id: actor,
            },
        )
        .await
        .is_err());
    assert!(!store.delete_ui_component_record(official_id).await.unwrap());
}

fn official_record(code: &str, group: &str, import_code: &str) -> OfficialUiComponentCatalogRecord {
    OfficialUiComponentCatalogRecord {
        component_code: code.into(),
        name: code.into(),
        description: "Official fixture".into(),
        import_code: import_code.into(),
        source_code: "opaque source {{{".into(),
        source: "taichuy".into(),
        group: group.into(),
        upstream: UiComponentRecordUpstream {
            identity: "@not-installed/package".into(),
            version: "9.9.9".into(),
        },
        version: "1.0.0".into(),
        keywords: vec!["fixture".into()],
        catalog_updated_at: time::OffsetDateTime::parse(
            "2026-08-23T00:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap(),
        source_locator: format!("ui_components/@taichuy/{group}/{code}.json"),
        source_checksum: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .into(),
    }
}

#[tokio::test]
async fn wp_d3_authoritative_group_replace_is_atomic_and_group_scoped() {
    let store = store().await;
    let actor = Uuid::now_v7();
    store
        .replace_official_ui_component_source_group(
            "taichuy",
            "group-a",
            &[
                official_record("taichuy.group-a.keep", "group-a", "opaque import one"),
                official_record("taichuy.group-a.remove", "group-a", "opaque import two"),
            ],
            actor,
        )
        .await
        .unwrap();
    store
        .replace_official_ui_component_source_group(
            "taichuy",
            "group-b",
            &[official_record(
                "taichuy.group-b.untouched",
                "group-b",
                "opaque import",
            )],
            actor,
        )
        .await
        .unwrap();
    let custom = store
        .create_ui_component_record(&CreateUiComponentRecordInput {
            component_code: "local.group-a.custom".into(),
            name: "Custom".into(),
            description: "Custom fixture".into(),
            import_code: "custom opaque import".into(),
            source_code: "custom opaque source".into(),
            source: "taichuy".into(),
            group: "group-a".into(),
            upstream: UiComponentRecordUpstream {
                identity: "@custom/package".into(),
                version: "1.0.0".into(),
            },
            version: "1.0.0".into(),
            keywords: vec![],
            actor_user_id: actor,
        })
        .await
        .unwrap();

    store
        .replace_official_ui_component_source_group(
            "taichuy",
            "group-a",
            &[official_record(
                "taichuy.group-a.keep",
                "group-a",
                "import Missing from '@never-installed/new-version';",
            )],
            actor,
        )
        .await
        .unwrap();

    let records = store.list_ui_component_records().await.unwrap();
    assert!(records.iter().any(|record| record.id == custom.id));
    assert!(records
        .iter()
        .any(|record| record.component_code == "taichuy.group-b.untouched"));
    assert!(!records
        .iter()
        .any(|record| record.component_code == "taichuy.group-a.remove"));
    let kept = records
        .iter()
        .find(|record| record.component_code == "taichuy.group-a.keep")
        .unwrap();
    assert_eq!(
        kept.import_code,
        "import Missing from '@never-installed/new-version';"
    );
}

#[tokio::test]
async fn wp_d3_rejects_mixed_identity_before_authoritative_replace() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let before = store.list_ui_component_records().await.unwrap();
    let error = store
        .replace_official_ui_component_source_group(
            "taichuy",
            "group-a",
            &[official_record(
                "taichuy.group-b.invalid",
                "group-b",
                "opaque",
            )],
            actor,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("source/group"));
    assert_eq!(store.list_ui_component_records().await.unwrap(), before);
}
