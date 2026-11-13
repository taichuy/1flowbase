use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn silent_command_upgrade_preserves_history_and_rejects_invalid_completion_states() {
    let (pool, _) =
        super::provider_protocol_capsule_store_tests::seeded_flow_run_before(Some(20261112010000))
            .await;
    let workspace = Uuid::now_v7();
    let instance = Uuid::now_v7();
    let conversation = Uuid::now_v7();
    let session = Uuid::now_v7();
    let event = Uuid::now_v7();
    let request = json!({
        "command_id":"user/paipan", "arguments":"chart arguments",
        "channel_id":instance, "conversation_id":conversation,
        "session_id":session, "input_id":event
    });
    sqlx::query("INSERT INTO communication_command_events(event_id,input_id,scope_id,instance_id,external_identity,conversation_id,session_id,request_json) VALUES($1,$1,$2,$3,'upgrade-fixture',$4,$5,$6)")
        .bind(event).bind(workspace).bind(instance).bind(conversation).bind(session)
        .bind(request).execute(&pool).await.unwrap();
    // Reproduce the deployed failure against the actual pre-upgrade constraint.
    let rejected = sqlx::query("UPDATE communication_command_events SET reply_json='null'::jsonb,response_session_id=$2 WHERE event_id=$1")
        .bind(event).bind(session).execute(&pool).await.unwrap_err();
    let rejected = rejected.as_database_error().unwrap();
    assert_eq!(rejected.code().as_deref(), Some("23514"));
    assert_eq!(
        rejected.constraint(),
        Some("communication_command_events_check1")
    );
    // Existing ordinary replies and unfinished requests must not be rewritten.
    let ordinary = Uuid::now_v7();
    sqlx::query(r#"INSERT INTO communication_command_events(event_id,input_id,scope_id,instance_id,external_identity,conversation_id,session_id,request_json,reply_json,response_session_id,send_status) VALUES($1,$1,$2,$3,'upgrade-fixture',$4,$5,'{"command_id":"host/help"}'::jsonb,'{"text":"Existing help"}'::jsonb,$5,'succeeded')"#)
        .bind(ordinary).bind(workspace).bind(instance).bind(conversation).bind(session)
        .execute(&pool).await.unwrap();
    let before: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(e) FROM communication_command_events e ORDER BY event_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let after: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(e) FROM communication_command_events e ORDER BY event_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        before, after,
        "upgrading must preserve every historical row"
    );

    for failure in [Some("command_outcome_unknown"), None] {
        sqlx::query("UPDATE communication_command_events SET reply_json='null'::jsonb,response_session_id=$2,failure_code=$3 WHERE event_id=$1")
            .bind(event).bind(session).bind(failure).execute(&pool).await.unwrap();
        let state: (bool, String, Option<Uuid>) = sqlx::query_as("SELECT reply_json IS NOT NULL,send_status,outbound_message_id FROM communication_command_events WHERE event_id=$1")
            .bind(event).fetch_one(&pool).await.unwrap();
        assert_eq!(
            state,
            (true, "none".into(), None),
            "both silent success and silent failure are terminal without sending"
        );
    }
    for mutation in [
        "response_session_id=NULL",
        "send_status='pending'",
        "outbound_message_id=gen_random_uuid()",
        "request_json=jsonb_set(request_json,'{command_id}','\"host/help\"')",
        "installation_id=gen_random_uuid(),contribution_id='managed',graph_fingerprint='graph',subscriber_id='subscriber',handler_id='handler',handler_version='1'",
    ] {
        let sql = format!("UPDATE communication_command_events SET {mutation} WHERE event_id=$1");
        let rejected = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(event).execute(&pool).await.unwrap_err();
        let rejected = rejected.as_database_error().unwrap();
        assert_eq!(rejected.code().as_deref(), Some("23514"));
        assert_eq!(rejected.constraint(), Some("communication_command_completion_state"));
    }
}
