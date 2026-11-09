use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn event_center_current_identity_migration_clears_only_new_configuration() {
    let (pool, run) =
        super::provider_protocol_capsule_store_tests::seeded_flow_run_before(Some(20261107010000))
            .await;
    let (workspace, owner, application): (Uuid, Uuid, Uuid) =
        sqlx::query_as("SELECT scope_id,created_by,application_id FROM flow_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO event_definitions(id,workspace_id,contract_id,contract_version,name,description,payload_schema,enabled,created_by,created_at,updated_at) VALUES($1,$2,'custom.draft','old','Draft','','{}',true,$3,now(),now())")
        .bind(Uuid::now_v7()).bind(workspace).bind(owner).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO workflow_event_triggers(id,workspace_id,application_id,enabled,contract_id,contract_version,trigger_phase,input_mapping,updated_by,updated_at) VALUES($1,$2,$3,true,'custom.draft','old','published','{}',$4,now())")
        .bind(Uuid::now_v7()).bind(workspace).bind(application).bind(owner).execute(&pool).await.unwrap();
    let instance = Uuid::now_v7();
    sqlx::query("INSERT INTO notification_instances(id,workspace_id,type_code,display_name) VALUES($1,$2,'event.upgrade','Preserved channel')")
        .bind(instance).bind(workspace).execute(&pool).await.unwrap();
    let retained = json!({"application_id":application,"system_prompt":"custom prompt","command_settings":[{"command_id":"host/status","enabled":false}],"mcp_instance_ids":["tools"],"extension_field":{"nested":true}});
    let mut old = retained.clone();
    old["event_commands"] = json!([{"contract_id":"custom.draft","contract_version":"old"}]);
    sqlx::query("INSERT INTO communication_bindings(instance_id,scope_id,actor_user_id,config_json,plugin_mcp_instance_id) VALUES($1,$2,$3,$4,'tools')")
        .bind(instance).bind(workspace).bind(owner).bind(old).execute(&pool).await.unwrap();
    let (before_binding, before_updated_at): (Value, time::OffsetDateTime) = sqlx::query_as(
        "SELECT to_jsonb(b)-'config_json'-'updated_at', updated_at FROM communication_bindings b WHERE instance_id=$1",
    )
    .bind(instance)
    .fetch_one(&pool)
    .await
    .unwrap();
    let before_apps: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(a) FROM applications a ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    // A real original protocol row must retain its contract/version and canonical bytes.
    let event = Uuid::now_v7();
    sqlx::query("INSERT INTO lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,graph_fingerprint,status) VALUES($1,$2,'original.fact','wire-v7',$3,now(),'original','pending')")
        .bind(event).bind(Uuid::now_v7()).bind(b"original canonical body".as_slice())
        .execute(&pool).await.unwrap();
    let before_outbox: Value =
        sqlx::query_scalar("SELECT to_jsonb(o) FROM lifecycle_outbox o WHERE event_id=$1")
            .bind(event)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    for (table, count_query) in [
        (
            "event_definitions",
            "SELECT count(*) FROM event_definitions",
        ),
        (
            "workflow_event_triggers",
            "SELECT count(*) FROM workflow_event_triggers",
        ),
    ] {
        let count: i64 = sqlx::query_scalar(count_query)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let retired: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name=$1 AND column_name='contract_version'").bind(table).fetch_one(&pool).await.unwrap();
        assert_eq!(retired, 0);
    }
    let after_config: Value =
        sqlx::query_scalar("SELECT config_json FROM communication_bindings WHERE instance_id=$1")
            .bind(instance)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after_config, retained);
    let (after_binding, after_updated_at): (Value, time::OffsetDateTime) = sqlx::query_as(
        "SELECT to_jsonb(b)-'config_json'-'updated_at', updated_at FROM communication_bindings b WHERE instance_id=$1",
    )
    .bind(instance)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before_binding, after_binding);
    // Removing event_commands is an UPDATE: retain the existing audit trigger semantics.
    assert!(after_updated_at > before_updated_at);
    let after_apps: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(a) FROM applications a ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(before_apps, after_apps);
    let after_outbox: Value =
        sqlx::query_scalar("SELECT to_jsonb(o) FROM lifecycle_outbox o WHERE event_id=$1")
            .bind(event)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before_outbox, after_outbox);
}
