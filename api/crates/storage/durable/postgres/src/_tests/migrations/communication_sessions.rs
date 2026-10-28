use control_plane_contracts::ports::CommunicationRepository;
use domain::CommunicationConversationScope;
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn communication_session_migration_preserves_legacy_runs_history_and_admits_only_safe_pending(
) {
    let (pool, run) =
        super::provider_protocol_capsule_store_tests::seeded_flow_run_before(Some(20261028120000))
            .await;
    let workspace: Uuid = sqlx::query_scalar("SELECT scope_id FROM flow_runs WHERE id=$1")
        .bind(run)
        .fetch_one(&pool)
        .await
        .unwrap();
    let owner: Uuid = sqlx::query_scalar("SELECT created_by FROM flow_runs WHERE id=$1")
        .bind(run)
        .fetch_one(&pool)
        .await
        .unwrap();
    let instance = Uuid::now_v7();
    let conversation = Uuid::now_v7();
    sqlx::query("INSERT INTO notification_instances(id,workspace_id,type_code,display_name) VALUES($1,$2,'session.migration','Preserved channel')").bind(instance).bind(workspace).execute(&pool).await.unwrap();
    let config = json!({"application_id":Uuid::now_v7(),"model":"existing-model","reasoning_effort":null,"file_table_id":null,"mcp_instance_ids":[],"system_prompt":"custom unchanged prompt"});
    sqlx::query("INSERT INTO communication_bindings(instance_id,scope_id,actor_user_id,config_json,plugin_mcp_instance_id) VALUES($1,$2,$3,$4,'preserved-tools')").bind(instance).bind(workspace).bind(owner).bind(&config).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO communication_conversations(id,scope_id,instance_id,external_identity,external_conversation_key) VALUES($1,$2,$3,'alice','preserved-chat')").bind(conversation).bind(workspace).bind(instance).execute(&pool).await.unwrap();
    let pending = Uuid::now_v7();
    let in_flight = Uuid::now_v7();
    let uncertain = Uuid::now_v7();
    for (id, processing, send, run_id) in [
        (pending, "pending", "none", None),
        (in_flight, "processing", "none", Some(run)),
        (uncertain, "succeeded", "unknown", Some(Uuid::now_v7())),
    ] {
        sqlx::query("INSERT INTO communication_inputs(id,scope_id,instance_id,conversation_id,external_identity,external_message_key,received_at_ms,processing_status,send_status,run_id) VALUES($1,$2,$3,$4,'alice',$5,42,$6,$7,$8)")
            .bind(id).bind(workspace).bind(instance).bind(conversation).bind(id.to_string()).bind(processing).bind(send).bind(run_id).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO communication_messages(id,scope_id,instance_id,external_identity,conversation_id,input_id,type,direction,occurred_at_ms,content) VALUES($1,$2,$3,'alice',$4,$1,'text','inbound',42,$5)")
            .bind(id).bind(workspace).bind(instance).bind(conversation).bind(format!("original {id}")).execute(&pool).await.unwrap();
    }
    let before_inputs: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(m) FROM communication_inputs m ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    let before_history: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(m) FROM communication_messages m ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    let before_chat: Value =
        sqlx::query_scalar("SELECT to_jsonb(c) FROM communication_conversations c WHERE id=$1")
            .bind(conversation)
            .fetch_one(&pool)
            .await
            .unwrap();
    let before_binding: Value =
        sqlx::query_scalar("SELECT to_jsonb(b) FROM communication_bindings b WHERE instance_id=$1")
            .bind(instance)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let after_inputs: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(m)-'turn_id'-'turn_sequence'-'admission_status' FROM communication_inputs m ORDER BY id").fetch_all(&pool).await.unwrap();
    assert_eq!(
        before_inputs, after_inputs,
        "run/lease/status and original timestamps remain unchanged"
    );
    let after_history: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(m) FROM communication_messages m ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(before_history, after_history);
    let after_chat: Value = sqlx::query_scalar("SELECT to_jsonb(c)-'session_epoch'-'selected_model' FROM communication_conversations c WHERE id=$1").bind(conversation).fetch_one(&pool).await.unwrap();
    assert_eq!(before_chat, after_chat);
    let after_binding: Value =
        sqlx::query_scalar("SELECT to_jsonb(b) FROM communication_bindings b WHERE instance_id=$1")
            .bind(instance)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before_binding, after_binding);
    let store = crate::PgControlPlaneStore::new(pool.clone());
    let scope = CommunicationConversationScope {
        workspace_id: workspace,
        instance_id: instance,
        external_identity: "alice".into(),
        conversation_id: conversation,
    };
    assert_eq!(
        store
            .pending_communication_admissions(&scope)
            .await
            .unwrap(),
        vec![pending]
    );
    assert!(store
        .admit_communication_input(&scope, in_flight)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .admit_communication_input(&scope, uncertain)
        .await
        .unwrap()
        .is_none());
    let admitted = store
        .admit_communication_input(&scope, pending)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(admitted.input_id, pending);
    assert_eq!(
        admitted.turn.selected_model.as_deref(),
        Some("existing-model")
    );
    assert_ne!(admitted.session.id, conversation);
    let preserved_run: Option<Uuid> =
        sqlx::query_scalar("SELECT run_id FROM communication_inputs WHERE id=$1")
            .bind(in_flight)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(preserved_run, Some(run));
}
