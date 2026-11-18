use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

async fn resource_rows(pool: &PgPool, table: &str, after_upgrade: bool) -> Vec<Value> {
    // Table names are exclusively the fixed migration-owned resources below.
    let projection = if after_upgrade && table == "workflow_event_triggers" {
        "to_jsonb(resource) - 'scope_id' - 'created_at'"
    } else if after_upgrade {
        "to_jsonb(resource) - 'scope_id'"
    } else {
        "to_jsonb(resource)"
    };
    let identity = if table == "event_catalog" {
        "identity"
    } else {
        "id"
    };
    let sql = format!("SELECT {projection} FROM {table} resource ORDER BY {identity}");
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn workspace_schema_readiness_preserves_history_and_derives_scope_for_new_writes() {
    let (pool, run) =
        super::provider_protocol_capsule_store_tests::seeded_flow_run_before(Some(20261117010000))
            .await;
    let (workspace, owner, application): (Uuid, Uuid, Uuid) =
        sqlx::query_as("SELECT scope_id,created_by,application_id FROM flow_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO custom_frontend_blocks(id,workspace_id,provider_code,contribution_code,title,source,language,created_by,updated_by) VALUES($1,$2,'custom','fixture','Preserved block','export default () => null','tsx',$3,$3)")
        .bind(Uuid::now_v7()).bind(workspace).bind(owner).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO event_definitions(id,workspace_id,contract_id,name,description,payload_schema,enabled,created_by,created_at,updated_at) VALUES($1,$2,'custom.readiness','Preserved event','','{}',true,$3,'2026-01-02T03:04:05Z','2026-01-03T03:04:05Z')")
        .bind(Uuid::now_v7()).bind(workspace).bind(owner).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO plugin_record_permission_policies(id,workspace_id,publisher_namespace,plugin_code,resource_code,record_id,mode,revision,created_by,updated_by) VALUES($1,$2,'fixture','readiness','records','preserved','inherit',1,$3,$3)")
        .bind(Uuid::now_v7()).bind(workspace).bind(owner).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO workflow_event_triggers(id,workspace_id,application_id,enabled,contract_id,trigger_phase,input_mapping,updated_by,updated_at) VALUES($1,$2,$3,true,'custom.readiness','published','{}',$4,'2026-01-04T03:04:05Z')")
        .bind(Uuid::now_v7()).bind(workspace).bind(application).bind(owner).execute(&pool).await.unwrap();
    let tables = [
        "custom_frontend_blocks",
        "event_definitions",
        "plugin_record_permission_policies",
        "workflow_event_triggers",
    ];
    let mut historical_rows = Vec::new();
    for table in tables {
        historical_rows.push(resource_rows(&pool, table, false).await);
    }
    let catalog_before = resource_rows(&pool, "event_catalog", false).await;
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    for (position, table) in tables.iter().enumerate() {
        assert_eq!(
            historical_rows[position],
            resource_rows(&pool, table, true).await
        );
        let sql = format!("SELECT scope_id,workspace_id FROM {table}");
        for row in sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(&pool)
            .await
            .unwrap()
        {
            assert_eq!(
                row.get::<Uuid, _>("scope_id"),
                row.get::<Uuid, _>("workspace_id")
            );
        }
        let indexes: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_indexes WHERE schemaname=current_schema() AND tablename=$1 AND indexdef LIKE '%(scope_id, created_at, id)%'")
            .bind(table).fetch_one(&pool).await.unwrap();
        assert_eq!(indexes, 1, "{table}: scoped chronology index");
        // A caller cannot supply a scope different from workspace_id.
        let sql = format!("UPDATE {table} SET scope_id=$1");
        let error = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(Uuid::now_v7())
            .execute(&pool)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("428C9")
        );
    }
    assert_eq!(
        catalog_before,
        resource_rows(&pool, "event_catalog", false).await
    );
    let (created, updated): (time::OffsetDateTime, time::OffsetDateTime) = sqlx::query_as(
        "SELECT created_at,updated_at FROM workflow_event_triggers WHERE application_id=$1",
    )
    .bind(application)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        created, updated,
        "historical creation time uses the last known timestamp"
    );
    sqlx::query(
        "UPDATE workflow_event_triggers SET enabled=false,updated_at=now() WHERE application_id=$1",
    )
    .bind(application)
    .execute(&pool)
    .await
    .unwrap();
    let preserved: time::OffsetDateTime = sqlx::query_scalar(
        "SELECT created_at FROM workflow_event_triggers WHERE application_id=$1",
    )
    .bind(application)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(preserved, created);
    sqlx::query("DELETE FROM workflow_event_triggers WHERE application_id=$1")
        .bind(application)
        .execute(&pool)
        .await
        .unwrap();
    let (scope, inserted, modified): (Uuid, time::OffsetDateTime, time::OffsetDateTime) = sqlx::query_as("INSERT INTO workflow_event_triggers(id,workspace_id,application_id,enabled,contract_id,trigger_phase,input_mapping,updated_by,updated_at) VALUES($1,$2,$3,true,'custom.readiness','published','{}',$4,now()) RETURNING scope_id,created_at,updated_at")
        .bind(Uuid::now_v7()).bind(workspace).bind(application).bind(owner)
        .fetch_one(&pool).await.unwrap();
    assert_eq!(scope, workspace);
    assert_eq!(
        inserted, modified,
        "the existing insert contract gets a creation timestamp"
    );
}
