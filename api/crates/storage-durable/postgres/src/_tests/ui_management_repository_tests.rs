use control_plane::ports::{
    CreateUiCodeTemplateInput, CreateUiComponentRecordInput, ReviseUiCodeTemplateInput,
    UiComponentRecordPatch, UiManagementRepository,
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
