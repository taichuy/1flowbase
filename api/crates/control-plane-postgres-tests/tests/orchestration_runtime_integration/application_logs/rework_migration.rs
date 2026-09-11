use super::*;
use sqlx::migrate::Migrator;
use std::borrow::Cow;

// AC-012/013: populated old schema, exact facts, forward migration, original
// read/rebuild, and retirement. Fresh installation is covered by rework.rs.
#[tokio::test]
async fn issue_2032_rework_migrates_populated_five_tables_into_original_logs() {
    let pool = isolated_database().await.connect().await.unwrap();
    let prior = Migrator {
        migrations: Cow::Owned(
            sqlx::migrate!("../storage/durable/postgres/migrations")
                .iter()
                .filter(|m| m.version < 20260911210000)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    prior.run(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let run = Uuid::now_v7();
    let conversation = Uuid::now_v7();
    let turn = Uuid::now_v7();
    sqlx::query("insert into flow_runs(id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,input_payload,created_by,api_key_id,finished_at) values($1,$2,$3,$4,$5,'published_api_run','succeeded',$6,$7,$8,now())")
        .bind(run).bind(seeded.application_id).bind(seeded.flow_id).bind(seeded.draft_id).bind(compiled.id).bind(json!({"query":"preserve input","history":[]})).bind(seeded.actor_user_id).bind(key).execute(store.pool()).await.unwrap();
    sqlx::query("insert into application_run_log_summaries(flow_run_id,scope_id,application_id,run_mode,status,title,input_payload,api_key_id,started_at,finished_at,created_at,updated_at) select id,scope_id,application_id,run_mode,status,'migration fixture',input_payload,api_key_id,started_at,finished_at,created_at,updated_at from flow_runs where id=$1")
        .bind(run).execute(store.pool()).await.unwrap();
    sqlx::query("insert into gateway_log_conversations(id,scope_id,application_id,api_key_id,external_user,protocol,thread_id,created_at,updated_at) select $1,scope_id,application_id,api_key_id,'','openai_responses','retained-thread',created_at,updated_at from flow_runs where id=$2")
        .bind(conversation).bind(run).execute(store.pool()).await.unwrap();
    sqlx::query("insert into gateway_log_turns(id,conversation_id,client_turn_id,created_at,updated_at) values($1,$2,'retained-turn',now(),now())")
        .bind(turn).bind(conversation).execute(store.pool()).await.unwrap();
    let output = json!({"type":"custom_tool_call","call_id":"retained-call","name":"exec","input":"console.log(42)"});
    let result = json!({"type":"custom_tool_call_output","call_id":"retained-call","output":"42"});
    let context = json!({"identity_status":"identified","thread_id":"retained-thread","turn_id":"retained-turn","tool_results":[result]});
    sqlx::query("insert into gateway_log_invocations(flow_run_id,scope_id,application_id,api_key_id,conversation_id,turn_id,identity_status,context,created_at) select id,scope_id,application_id,api_key_id,$2,$3,'identified',$4,created_at from flow_runs where id=$1")
        .bind(run).bind(conversation).bind(turn).bind(&context).execute(store.pool()).await.unwrap();
    sqlx::query("insert into runtime_events(id,flow_run_id,sequence,event_type,layer,source,trust_level,payload,visibility,durability) values($1,$2,1,'provider_output_item_done','agent_transition','host','host_fact',$3,'workspace','durable')")
        .bind(Uuid::now_v7()).bind(run).bind(json!({"item":output})).execute(store.pool()).await.unwrap();
    sqlx::query("insert into gateway_log_output_items(owner_id,item_key,conversation_id,turn_id,flow_run_id,sequence,observed_at,item,conflicting) values($1,'tool:retained-call',$1,$2,$3,1,now(),$4,true)")
        .bind(conversation).bind(turn).bind(run).bind(&output).execute(store.pool()).await.unwrap();
    sqlx::query("insert into gateway_log_tool_results(conversation_id,call_id,flow_run_id,received_at,result,conflicting) values($1,'retained-call',$2,now(),$3,true)")
        .bind(conversation).bind(run).bind(&result).execute(store.pool()).await.unwrap();
    run_migrations(store.pool()).await.unwrap();
    for table in [
        "gateway_log_conversations",
        "gateway_log_turns",
        "gateway_log_invocations",
        "gateway_log_output_items",
        "gateway_log_tool_results",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, Option<String>>("select to_regclass($1)::text")
                .bind(table)
                .fetch_one(store.pool())
                .await
                .unwrap(),
            None
        );
    }
    let retained: serde_json::Value =
        sqlx::query_scalar("select log_context from flow_runs where id=$1")
            .bind(run)
            .fetch_one(store.pool())
            .await
            .unwrap();
    for (field, value) in context.as_object().unwrap() {
        assert_eq!(&retained[field], value);
    }
    assert_eq!(retained["log_conversation_id"], conversation.to_string());
    assert_eq!(retained["log_task_run_id"], run.to_string());
    let original_input: serde_json::Value =
        sqlx::query_scalar("select input_payload from flow_runs where id=$1")
            .bind(run)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(
        original_input,
        json!({"query":"preserve input","history":[]})
    );
    let page = store
        .list_application_run_conversation_message_items_page(
            seeded.application_id,
            run,
            ListApplicationRunConversationMessageItemsPageInput {
                limit: 50,
                before_sequence: None,
                after_sequence: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
    let first = page.items.iter().map(|item| item.id).collect::<Vec<_>>();
    let facts:Vec<serde_json::Value>=sqlx::query_scalar("select native_message from application_run_conversation_message_items where flow_run_id=$1 order by display_sequence").bind(run).fetch_all(store.pool()).await.unwrap();
    assert!(facts
        .iter()
        .any(|m| m["_source_item"] == output && m["_log_conflicting"] == true));
    assert!(facts
        .iter()
        .any(|m| m["_source_item"] == result && m["_log_conflicting"] == true));
    sqlx::query("delete from application_run_conversation_message_items where flow_run_id=$1")
        .bind(run)
        .execute(store.pool())
        .await
        .unwrap();
    let rebuilt = store
        .list_application_run_conversation_message_items_page(
            seeded.application_id,
            run,
            ListApplicationRunConversationMessageItemsPageInput {
                limit: 50,
                before_sequence: None,
                after_sequence: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        first,
        rebuilt.items.iter().map(|item| item.id).collect::<Vec<_>>()
    );
}
