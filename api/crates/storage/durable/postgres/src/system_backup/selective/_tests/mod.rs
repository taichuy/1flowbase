mod fixes;

use super::*;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

async fn fixture() -> (PgPool, Uuid) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let pool = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap()
        .connect()
        .await
        .unwrap();
    crate::run_migrations(&pool).await.unwrap();
    let actor = Uuid::now_v7();
    sqlx::query("insert into users(id,account,email,password_hash,name,nickname,status) values($1,'selective','selective@example.com','fixture','Selective','Selective','active')").bind(actor).execute(&pool).await.unwrap();
    (pool, actor)
}
fn select(feature: &str, structure: bool, data: bool) -> Vec<SelectiveBackupSelection> {
    vec![SelectiveBackupSelection {
        feature_id: format!("system.{feature}"),
        structure,
        data,
    }]
}
async fn capture(
    repository: &PgSelectiveBackupRepository,
    selection: Vec<SelectiveBackupSelection>,
) -> Vec<u8> {
    let source = repository.source(selection).await.unwrap();
    let (writer, mut reader) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn(async move { source.write_to(Box::pin(writer)).await.unwrap() });
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.unwrap();
    task.await.unwrap();
    bytes
}
fn reader(bytes: Vec<u8>) -> BackupComponentReader {
    Box::pin(std::io::Cursor::new(bytes))
}
async fn pool(pool: &PgPool, actor: Uuid, id: Uuid, name: &str) {
    sqlx::query("insert into network_egress_pools(id,scope_id,display_name,selection_strategy,created_by,updated_by) values($1,'00000000-0000-0000-0000-000000000000',$2,'healthy_first',$3,$3)").bind(id).bind(name).bind(actor).execute(pool).await.unwrap();
}

#[tokio::test]
async fn selective_roundtrip_preserves_target_only_and_prepare_can_rollback() {
    let (db, actor) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let id = Uuid::now_v7();
    pool(&db, actor, id, "Original").await;
    let bytes = capture(&repo, select("network-center", true, false)).await;
    assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
    sqlx::query("update network_egress_pools set display_name='Changed' where id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let extra = Uuid::now_v7();
    pool(&db, actor, extra, "Target only").await;
    let preview = repo
        .preflight(reader(bytes.clone()), "source", "target")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    assert!(!preview
        .selected_tables
        .contains(&"network_egress_projections".into()));
    let pending = repo
        .prepare_restore(reader(bytes.clone()), "source", "target", true)
        .await
        .unwrap();
    pending.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select display_name from network_egress_pools where id=$1"
        )
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap(),
        "Changed"
    );
    repo.restore(reader(bytes), "source", "target", true)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select display_name from network_egress_pools where id=$1"
        )
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap(),
        "Original"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from network_egress_pools where id=$1")
            .bind(extra)
            .fetch_one(&db)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn selective_catalog_covers_formal_schema_and_execution_facts() {
    let (db, _) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let categories = repo.catalog().await.unwrap();
    let application = categories
        .iter()
        .find(|c| c.feature_id == "system.applications")
        .unwrap();
    for name in [
        "flow_runs",
        "provider_protocol_capsules",
        "provider_semantic_trajectory_steps",
        "client_trajectory_node_links",
        "runtime_canonical_contents",
    ] {
        assert!(
            application.data_tables.contains(&name.into()),
            "missing {name}"
        );
    }
    assert_eq!(application.label_key, "auto.application_management");
    let all = categories
        .iter()
        .flat_map(|c| c.structure_tables.iter().chain(&c.data_tables))
        .collect::<BTreeSet<_>>();
    assert!(!all.contains(&"extension_artifact_instances".to_owned()));
    assert!(!all.contains(&"extension_installation_node_inventory".to_owned()));
    assert!(all.contains(&"extension_installations".to_owned()));
    sqlx::query("create table unclassified_persistent_fixture(id uuid primary key)")
        .execute(&db)
        .await
        .unwrap();
    assert!(repo
        .catalog()
        .await
        .unwrap_err()
        .to_string()
        .contains("unclassified_persistent_fixture"));
}

#[tokio::test]
async fn selective_preflight_blocks_schema_and_natural_identity_conflicts_without_writes() {
    let (db, actor) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let id = Uuid::now_v7();
    pool(&db, actor, id, "Source name").await;
    let bytes = capture(&repo, select("network-center", true, false)).await;
    sqlx::query("update network_egress_pools set display_name='Target name' where id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    pool(&db, actor, Uuid::now_v7(), "SOURCE NAME").await;
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(
        preview
            .failures
            .iter()
            .any(|f| f.contains("unique identity conflict")),
        "{:?}",
        preview.failures
    );
    assert!(repo
        .restore(reader(bytes.clone()), "key", "key", true)
        .await
        .is_err());
    sqlx::query("alter table network_egress_pool_members add column incompatible_fixture text")
        .execute(&db)
        .await
        .unwrap();
    let preview = repo.preflight(reader(bytes), "key", "key").await.unwrap();
    assert!(preview
        .failures
        .iter()
        .any(|f| f.contains("incompatible fields: network_egress_pool_members")));
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select display_name from network_egress_pools where id=$1"
        )
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap(),
        "Target name"
    );
}

#[tokio::test]
async fn selective_plugin_data_requires_confirmation_and_never_creates_tables() {
    let (db, _) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    sqlx::query("create table selective_plugin_records(id uuid primary key,payload text not null)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("insert into plugin_schema_ownership(ownership_key,owner_id,owner_version,object_kind,logical_name,physical_table,plan_fingerprint) values('fixture:records','fixture.missing','1.0.0','owned_collection','records','selective_plugin_records','fixture')").execute(&db).await.unwrap();
    let id = Uuid::now_v7();
    sqlx::query("insert into selective_plugin_records values($1,'archive')")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let bytes = capture(&repo, select("extension-center", false, true)).await;
    sqlx::query("update selective_plugin_records set payload='target'")
        .execute(&db)
        .await
        .unwrap();
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert_eq!(preview.missing_plugins, vec!["fixture.missing"]);
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    assert!(repo
        .restore(reader(bytes.clone()), "key", "key", false)
        .await
        .is_err());
    repo.restore(reader(bytes.clone()), "key", "key", true)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("select payload from selective_plugin_records")
            .fetch_one(&db)
            .await
            .unwrap(),
        "archive"
    );
    sqlx::query("drop table selective_plugin_records")
        .execute(&db)
        .await
        .unwrap();
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(preview
        .failures
        .iter()
        .any(|f| f.contains("missing table: selective_plugin_records")));
    assert!(repo
        .restore(reader(bytes), "key", "key", true)
        .await
        .is_err());
    assert!(sqlx::query_scalar::<_, Option<String>>(
        "select to_regclass('selective_plugin_records')::text"
    )
    .fetch_one(&db)
    .await
    .unwrap()
    .is_none());
}

#[test]
fn selective_rekeys_configuration_and_aad_capsules() {
    use crate::secret_crypto::*;
    let plaintext = serde_json::json!({"token":"portable-fixture"});
    let mut row = serde_json::json!({"encrypted_secret_json":encrypt_secret_json(&plaintext,"source").unwrap()});
    validation::rekey("mcp_client_credentials", &mut row, "source", "target").unwrap();
    assert_eq!(
        decrypt_secret_json(&row["encrypted_secret_json"], "target").unwrap(),
        plaintext
    );
    assert!(decrypt_secret_json(&row["encrypted_secret_json"], "source").is_err());
    let run = Uuid::now_v7();
    let aad = format!("provider_protocol_capsule:v1\0{run}\0continuation\0slot");
    let mut capsule = serde_json::json!({"flow_run_id":run,"capsule_kind":"continuation","slot_key":"slot","encrypted_payload":encrypt_secret_json_with_aad(&plaintext,"source",aad.as_bytes()).unwrap()});
    validation::rekey(
        "provider_protocol_capsules",
        &mut capsule,
        "source",
        "target",
    )
    .unwrap();
    assert_eq!(
        decrypt_secret_json_with_aad(&capsule["encrypted_payload"], "target", aad.as_bytes())
            .unwrap(),
        plaintext
    );
}

#[tokio::test]
async fn selective_imports_required_plugin_identity_without_artifact_state() {
    let (db, actor) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let tenant = Uuid::now_v7();
    let workspace = Uuid::now_v7();
    let installation = Uuid::now_v7();
    let source_id = Uuid::now_v7();
    sqlx::query("insert into tenants(id,code,name) values($1,'selective','Selective')")
        .bind(tenant)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("insert into workspaces(id,tenant_id,name) values($1,$2,'Selective')")
        .bind(workspace)
        .bind(tenant)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("insert into extension_installations(id,category,organization,artifact_id,artifact_version,plugin_id,contract_version,protocol,display_name,source_kind,trust_level,verification_status,desired_state,signature_status,created_by) values($1,'runtime-extensions','fixture','source-plugin','1.0.0','fixture.source','1','stdio','Fixture source','uploaded','unverified','valid','active_requested','missing',$2)").bind(installation).bind(actor).execute(&db).await.unwrap();
    sqlx::query("insert into data_source_instances(id,workspace_id,installation_id,source_code,display_name,status,created_by) values($1,$2,$3,'fixture-source','Fixture source','active',$4)").bind(source_id).bind(workspace).bind(installation).bind(actor).execute(&db).await.unwrap();
    let bytes = capture(&repo, select("data-models", true, false)).await;
    sqlx::query("delete from data_source_instances where id=$1")
        .bind(source_id)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("delete from extension_installations where id=$1")
        .bind(installation)
        .execute(&db)
        .await
        .unwrap();
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    assert!(preview.missing_plugins.contains(&"fixture.source".into()));
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>(
            "select installation_id from data_source_instances where id=$1"
        )
        .bind(source_id)
        .fetch_one(&db)
        .await
        .unwrap(),
        installation
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select desired_state from extension_installations where id=$1"
        )
        .bind(installation)
        .fetch_one(&db)
        .await
        .unwrap(),
        "disabled"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from extension_artifact_instances where installation_id=$1"
        )
        .bind(installation)
        .fetch_one(&db)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn selective_preflight_reports_missing_unselected_identity_parent() {
    let (db, actor) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    pool(&db, actor, Uuid::now_v7(), "Needs actor").await;
    let bytes = capture(&repo, select("network-center", true, false)).await;
    sqlx::query("delete from network_egress_pools")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("delete from users where id=$1")
        .bind(actor)
        .execute(&db)
        .await
        .unwrap();
    let preview = repo.preflight(reader(bytes), "key", "key").await.unwrap();
    assert!(
        preview
            .failures
            .iter()
            .any(|f| f.contains("network_egress_pools -> users")),
        "{:?}",
        preview.failures
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from network_egress_pools")
            .fetch_one(&db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn selective_immutable_conflict_blocks_all_selected_changes() {
    use control_plane_contracts::ports::{ApplicationRepository, CreateApplicationInput};
    let (db, actor) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let tenant = Uuid::now_v7();
    let workspace = Uuid::now_v7();
    sqlx::query("insert into tenants(id,code,name) values($1,'immutable','Immutable')")
        .bind(tenant)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("insert into workspaces(id,tenant_id,name) values($1,$2,'Immutable')")
        .bind(workspace)
        .bind(tenant)
        .execute(&db)
        .await
        .unwrap();
    let store = crate::PgControlPlaneStore::new(db.clone());
    let application = store
        .create_application(&CreateApplicationInput {
            actor_user_id: actor,
            workspace_id: workspace,
            application_type: domain::ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Original app".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let content = Uuid::now_v7();
    sqlx::query("insert into runtime_canonical_contents(id,scope_id,application_id,content_hash,content,byte_size) values($1,$2,$3,$4,'{}',2)").bind(content).bind(workspace).bind(application.id).bind(format!("sha256:{}","0".repeat(64))).execute(&db).await.unwrap();
    let bytes = capture(&repo, select("applications", true, true)).await;
    // Controlled negative: the archive requests an update forbidden by the actual immutable trigger.
    use std::io::{Read, Write};
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes.as_slice())
        .read_to_string(&mut text)
        .unwrap();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    for (position, line) in text.lines().enumerate() {
        let mut value: serde_json::Value = serde_json::from_str(line).unwrap();
        if position > 0 && value["table"] == "runtime_canonical_contents" {
            value["row"]["content"] = serde_json::json!({"altered":true});
        }
        writeln!(encoder, "{}", value).unwrap();
    }
    let conflicting = encoder.finish().unwrap();
    sqlx::query("update applications set name='Target app' where id=$1")
        .bind(application.id)
        .execute(&db)
        .await
        .unwrap();
    let preview = repo
        .preflight(reader(conflicting.clone()), "key", "key")
        .await
        .unwrap();
    assert!(
        preview
            .failures
            .iter()
            .any(|f| f.contains("immutable row conflict")),
        "{:?}",
        preview.failures
    );
    assert!(repo
        .restore(reader(conflicting), "key", "key", true)
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, String>("select name from applications where id=$1")
            .bind(application.id)
            .fetch_one(&db)
            .await
            .unwrap(),
        "Target app"
    );
}

#[tokio::test]
async fn selective_rejects_unrestorable_dynamic_table_before_source_creation() {
    let (db, _) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    sqlx::query("create table selective_no_key_fixture(payload text)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("insert into plugin_schema_ownership(ownership_key,owner_id,owner_version,object_kind,logical_name,physical_table,plan_fingerprint) values('fixture:no-key','fixture.missing','1.0.0','owned_collection','no-key','selective_no_key_fixture','fixture')").execute(&db).await.unwrap();
    match repo.source(select("extension-center", false, true)).await {
        Err(error) => assert!(error.to_string().contains("has no primary key")),
        Ok(_) => panic!("unrestorable table must fail before source creation"),
    }
}

#[tokio::test]
async fn selective_data_mode_includes_configuration_and_records_effective_scope() {
    let (db, actor) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let id = Uuid::now_v7();
    pool(&db, actor, id, "Configuration included").await;
    let bytes = capture(&repo, select("network-center", false, true)).await;
    let scratch = archive::unpack(reader(bytes.clone())).await.unwrap();
    let header = archive::header(&mut scratch.reader().await.unwrap())
        .await
        .unwrap();
    assert!(header
        .tables
        .iter()
        .any(|entry| entry.name == "network_egress_projections"));
    sqlx::query("update network_egress_pools set display_name='Changed' where id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select display_name from network_egress_pools where id=$1"
        )
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap(),
        "Configuration included"
    );

    let ui = capture(&repo, select("ui-management", false, true)).await;
    let scratch = archive::unpack(reader(ui)).await.unwrap();
    let header = archive::header(&mut scratch.reader().await.unwrap())
        .await
        .unwrap();
    assert!(header.selection[0].structure && header.selection[0].data);
    for table in [
        "frontstage_pages",
        "frontstage_page_tabs",
        "frontstage_page_schemas",
        "frontstage_block_nodes",
        "frontstage_block_codes",
    ] {
        assert!(
            header.tables.iter().any(|entry| entry.name == table),
            "missing {table}"
        );
    }
}
