use super::*;
use serde_json::{json, Value};

fn archive_lines(bytes: &[u8]) -> Vec<Value> {
    use std::io::Read;
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_string(&mut text)
        .unwrap();
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
async fn model_fixture(db: &PgPool, actor: Uuid) -> domain::ModelDefinitionRecord {
    use control_plane_contracts::ports::{CreateModelDefinitionInput, ModelDefinitionRepository};
    let tenant = Uuid::now_v7();
    let workspace = Uuid::now_v7();
    sqlx::query("insert into tenants(id,code,name) values($1,'schema-fix','Schema fix')")
        .bind(tenant)
        .execute(db)
        .await
        .unwrap();
    sqlx::query("insert into workspaces(id,tenant_id,name) values($1,$2,'Schema fix')")
        .bind(workspace)
        .bind(tenant)
        .execute(db)
        .await
        .unwrap();
    let store = crate::PgControlPlaneStore::new(db.clone());
    ModelDefinitionRepository::create_model_definition(
        &store,
        &CreateModelDefinitionInput {
            actor_user_id: actor,
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id: workspace,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: "fixture".into(),
            template_code: "schema-fix".into(),
            template_version: "1".into(),
            code: "selective_schema_fixture".into(),
            title: "Original definition".into(),
            description: None,
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn structure_only_validates_physical_schema_without_exporting_or_creating_rows() {
    let (db, actor) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let model = model_fixture(&db, actor).await;
    let bytes = capture(&repo, select("data-models", true, false)).await;
    let lines = archive_lines(&bytes);
    assert!(lines[0]["schema_dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == model.physical_table_name));
    assert!(!lines
        .iter()
        .skip(1)
        .any(|r| r["table"] == model.physical_table_name));
    sqlx::query("update model_definitions set title='Target definition' where id=$1")
        .bind(model.id)
        .execute(&db)
        .await
        .unwrap();
    let table = schema::quote(&model.physical_table_name).unwrap();
    sqlx::query(&format!(
        "alter table {table} add column incompatible_fixture text"
    ))
    .execute(&db)
    .await
    .unwrap();
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(
        preview
            .failures
            .iter()
            .any(|f| f.contains("incompatible physical fields")
                && f.contains(&model.physical_table_name)),
        "{:?}",
        preview.failures
    );
    assert!(repo
        .restore(reader(bytes.clone()), "key", "key", true)
        .await
        .is_err());
    sqlx::query(&format!("drop table {table}"))
        .execute(&db)
        .await
        .unwrap();
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(
        preview
            .failures
            .iter()
            .any(|f| f.contains("missing physical table dependency")
                && f.contains(&model.physical_table_name)),
        "{:?}",
        preview.failures
    );
    assert!(repo
        .restore(reader(bytes), "key", "key", true)
        .await
        .is_err());
    assert!(
        sqlx::query_scalar::<_, Option<String>>("select to_regclass($1)::text")
            .bind(&model.physical_table_name)
            .fetch_one(&db)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("select title from model_definitions where id=$1")
            .bind(model.id)
            .fetch_one(&db)
            .await
            .unwrap(),
        "Target definition"
    );
}

#[tokio::test]
async fn generated_expression_difference_blocks_before_restoring_any_rows() {
    let (db, _) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    sqlx::query("create table selective_generated_fixture(id uuid primary key, amount integer not null, calculated integer generated always as (amount * 2) stored)").execute(&db).await.unwrap();
    sqlx::query("insert into plugin_schema_ownership(ownership_key,owner_id,owner_version,object_kind,logical_name,physical_table,plan_fingerprint) values('fixture:generated','fixture.generated','1','owned_collection','generated','selective_generated_fixture','fixture')").execute(&db).await.unwrap();
    let id = Uuid::now_v7();
    sqlx::query("insert into selective_generated_fixture(id,amount) values($1,2)")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let bytes = capture(&repo, select("extension-center", false, true)).await;
    let lines = archive_lines(&bytes);
    let table = lines[0]["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "selective_generated_fixture")
        .unwrap();
    assert!(table["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "calculated")
        .unwrap()["generated_expression"]
        .as_str()
        .unwrap()
        .contains("2"));
    sqlx::query("alter table selective_generated_fixture drop column calculated")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("alter table selective_generated_fixture add column calculated integer generated always as (amount * 3) stored").execute(&db).await.unwrap();
    sqlx::query("update selective_generated_fixture set amount=7")
        .execute(&db)
        .await
        .unwrap();
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(
        preview
            .failures
            .iter()
            .any(|f| f.contains("incompatible fields: selective_generated_fixture")),
        "{:?}",
        preview.failures
    );
    assert!(repo
        .restore(reader(bytes), "key", "key", true)
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i32>("select calculated from selective_generated_fixture")
            .fetch_one(&db)
            .await
            .unwrap(),
        21
    );
}

async fn plugin_fixture(db: &PgPool, actor: Uuid) -> (Uuid, Uuid) {
    let tenant = Uuid::now_v7();
    let workspace = Uuid::now_v7();
    let installation = Uuid::now_v7();
    let source = Uuid::now_v7();
    sqlx::query("insert into tenants(id,code,name) values($1,'identity-fix','Identity fix')")
        .bind(tenant)
        .execute(db)
        .await
        .unwrap();
    sqlx::query("insert into workspaces(id,tenant_id,name) values($1,$2,'Identity fix')")
        .bind(workspace)
        .bind(tenant)
        .execute(db)
        .await
        .unwrap();
    sqlx::query("insert into extension_installations(id,category,organization,artifact_id,artifact_version,plugin_id,contract_version,protocol,display_name,source_kind,trust_level,verification_status,desired_state,signature_status,receipt,metadata_json,created_by) values($1,'runtime-extensions','fixture','identity-plugin','1','fixture.identity','1','stdio','Source identity','official_registry','verified_official','valid','active_requested','verified','{\"path\":\"/private/source\"}','{\"local_path\":\"/private/source\"}',$2)").bind(installation).bind(actor).execute(db).await.unwrap();
    sqlx::query("insert into data_source_instances(id,workspace_id,installation_id,source_code,display_name,status,created_by) values($1,$2,$3,'identity-source','Source child','active',$4)").bind(source).bind(workspace).bind(installation).bind(actor).execute(db).await.unwrap();
    (installation, source)
}

#[tokio::test]
async fn implicit_plugin_identity_preserves_existing_target_policy_and_trust() {
    let (db, actor) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let (installation, source) = plugin_fixture(&db, actor).await;
    let bytes = capture(&repo, select("data-models", true, false)).await;
    let lines = archive_lines(&bytes);
    let identity = lines
        .iter()
        .skip(1)
        .find(|r| r["table"] == "extension_installations")
        .unwrap();
    for field in [
        "desired_state",
        "trust_level",
        "verification_status",
        "signature_status",
        "source_kind",
        "receipt",
        "metadata_json",
        "warnings",
        "is_system_reserved",
    ] {
        assert!(
            identity["row"].get(field).is_none(),
            "source installation state leaked: {field}"
        );
    }
    sqlx::query("update extension_installations set desired_state='disabled',verification_status='invalid',receipt='{\"target\":true}',metadata_json='{\"target_policy\":true}',display_name='Target identity' where id=$1").bind(installation).execute(&db).await.unwrap();
    let before: Value =
        sqlx::query_scalar("select to_jsonb(t) from extension_installations t where id=$1")
            .bind(installation)
            .fetch_one(&db)
            .await
            .unwrap();
    sqlx::query("update data_source_instances set display_name='Target child' where id=$1")
        .bind(source)
        .execute(&db)
        .await
        .unwrap();
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    let after: Value =
        sqlx::query_scalar("select to_jsonb(t) from extension_installations t where id=$1")
            .bind(installation)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(after, before);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select display_name from data_source_instances where id=$1"
        )
        .bind(source)
        .fetch_one(&db)
        .await
        .unwrap(),
        "Source child"
    );
}

#[tokio::test]
async fn implicit_plugin_identity_creates_only_disabled_unverified_target_identity() {
    let (db, actor) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let (installation, source) = plugin_fixture(&db, actor).await;
    let bytes = capture(&repo, select("data-models", true, false)).await;
    sqlx::query("delete from data_source_instances where id=$1")
        .bind(source)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("delete from extension_installations where id=$1")
        .bind(installation)
        .execute(&db)
        .await
        .unwrap();
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    let identity: Value =
        sqlx::query_scalar("select to_jsonb(t) from extension_installations t where id=$1")
            .bind(installation)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(identity["desired_state"], "disabled");
    assert_eq!(identity["trust_level"], "unverified");
    assert_eq!(identity["verification_status"], "pending");
    assert_eq!(identity["signature_status"], "missing");
    assert_eq!(identity["receipt"], json!({}));
    assert_eq!(identity["metadata_json"], json!({}));
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
