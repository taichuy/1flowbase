use control_plane_contracts::{application_public_runtime::*, ports::*};
use serde_json::{json, Value};
use storage_durable_postgres::PgControlPlaneStore;
use time::OffsetDateTime;
use uuid::Uuid;

fn exceptional() -> Value {
    json!({"nul\0key": ["before\0after", "literal\\u0000", "\\", null], "nul\\u0000key": "distinct"})
}

async fn seed() -> (PgControlPlaneStore, Uuid, Uuid, Uuid) {
    let (pool, run) = super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let (app, scope): (Uuid, Uuid) =
        sqlx::query_as("select application_id, scope_id from flow_runs where id=$1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    (PgControlPlaneStore::new(pool), run, app, scope)
}

#[tokio::test]
async fn runtime_json_repository_round_trip_and_replacement() {
    let (store, run, app, scope) = seed().await;
    let original = exceptional();
    let input = UpdateFlowRunPayloadsInput {
        flow_run_id: run,
        input_payload: original.clone(),
        output_payload: original.clone(),
        error_payload: Some(original.clone()),
    };
    let saved = store.update_flow_run_payloads(&input).await.unwrap();
    assert_eq!(saved.input_payload, original);
    let reread = store.get_flow_run(app, run).await.unwrap().unwrap();
    assert_eq!(reread.output_payload, original);
    assert_eq!(reread.error_payload, Some(original.clone()));
    let node = store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run,
            node_id: "node".into(),
            node_type: "llm".into(),
            node_alias: "Node".into(),
            status: domain::NodeRunStatus::Running,
            input_payload: original.clone(),
            debug_payload: original.clone(),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    assert_eq!(node.input_payload, original);
    let failed = store
        .update_node_run(&UpdateNodeRunInput {
            node_run_id: node.id,
            status: domain::NodeRunStatus::Failed,
            output_payload: original.clone(),
            error_payload: Some(original.clone()),
            metrics_payload: json!({}),
            debug_payload: original.clone(),
            finished_at: None,
        })
        .await
        .unwrap();
    assert_eq!(failed.error_payload, Some(original.clone()));
    // A missing error on an already-failed node preserves both projection and original.
    let retained = store
        .update_node_run_payloads(&UpdateNodeRunPayloadsInput {
            node_run_id: node.id,
            input_payload: original.clone(),
            output_payload: original.clone(),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({}),
        })
        .await
        .unwrap();
    assert_eq!(retained.error_payload, Some(original.clone()));
    let checkpoint = store
        .create_checkpoint(&CreateCheckpointInput {
            flow_run_id: run,
            node_run_id: Some(node.id),
            status: "waiting".into(),
            reason: "callback".into(),
            locator_payload: original.clone(),
            variable_snapshot: original.clone(),
            external_ref_payload: Some(Value::Null),
        })
        .await
        .unwrap();
    assert_eq!(checkpoint.variable_snapshot, original);
    assert_eq!(
        store
            .get_checkpoint(run, checkpoint.id)
            .await
            .unwrap()
            .unwrap()
            .locator_payload,
        original
    );
    let canonical_input = PutCanonicalRuntimeContentInput {
        scope_id: scope,
        application_id: app,
        content: original.clone(),
    };
    let canonical = store
        .put_canonical_runtime_content(&canonical_input)
        .await
        .unwrap();
    assert_eq!(canonical.content, original);
    assert_eq!(
        store
            .put_canonical_runtime_content(&canonical_input)
            .await
            .unwrap()
            .id,
        canonical.id
    );
    let events = vec![
        AppendRunEventInput {
            flow_run_id: run,
            node_run_id: Some(node.id),
            event_type: "fixture".into(),
            payload: original.clone(),
        },
        AppendRunEventInput {
            flow_run_id: run,
            node_run_id: None,
            event_type: "fixture".into(),
            payload: json!({"ordinary":true}),
        },
    ];
    let records = store.append_run_events(&events).await.unwrap();
    assert_eq!(records[0].payload, original);
    // Normal replacement removes only its sidecar slot; SQL NULL remains distinct from JSON null.
    for error in [Some(Value::Null), None] {
        let value = store
            .update_flow_run_payloads(&UpdateFlowRunPayloadsInput {
                flow_run_id: run,
                input_payload: json!({"literal":"\\u0000"}),
                output_payload: Value::Null,
                error_payload: error.clone(),
            })
            .await
            .unwrap();
        assert_eq!(value.error_payload, error);
        assert_eq!(value.output_payload, Value::Null);
    }
    let sidecar: Value = sqlx::query_scalar("select raw_json_payloads from flow_runs where id=$1")
        .bind(run)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(sidecar, json!({}));
}

#[tokio::test]
async fn runtime_json_callback_reservation_claim_replay_and_inbox() {
    let (store, run, app, scope) = seed().await;
    let original = exceptional();
    let node = store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run,
            node_id: "callback".into(),
            node_type: "llm".into(),
            node_alias: "Callback".into(),
            status: domain::NodeRunStatus::WaitingCallback,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let checkpoint = store
        .create_checkpoint(&CreateCheckpointInput {
            flow_run_id: run,
            node_run_id: Some(node.id),
            status: "waiting".into(),
            reason: "callback".into(),
            locator_payload: json!({}),
            variable_snapshot: original.clone(),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    let callback = store
        .create_callback_task(&CreateCallbackTaskInput {
            flow_run_id: run,
            node_run_id: node.id,
            callback_kind: "fixture".into(),
            request_payload: original.clone(),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    let reserve = RecordFlowRunCallbackResumeAttemptInput {
        flow_run_id: run,
        callback_task_id: callback.id,
        source: "openai_responses".into(),
        response_payload: original.clone(),
        idempotency_key: callback.id.to_string(),
    };
    let attempt = store
        .record_flow_run_callback_resume_attempt(&reserve)
        .await
        .unwrap();
    assert_eq!(attempt.attempt.response_payload, original);
    assert!(
        !store
            .record_flow_run_callback_resume_attempt(&reserve)
            .await
            .unwrap()
            .inserted
    );
    let complete = CompleteCallbackTaskInput {
        callback_task_id: callback.id,
        response_payload: original.clone(),
        completed_at: OffsetDateTime::now_utc(),
    };
    assert_eq!(
        store
            .complete_callback_task(&complete)
            .await
            .unwrap()
            .response_payload,
        Some(original.clone())
    );
    assert_eq!(
        store
            .complete_callback_task(&complete)
            .await
            .unwrap()
            .response_payload,
        Some(original.clone())
    );
    let mismatch = CompleteCallbackTaskInput {
        response_payload: json!({"literal":"\\u0000"}),
        ..complete
    };
    assert!(store.complete_callback_task(&mismatch).await.is_err());
    sqlx::query("update flow_runs set status='waiting_callback' where id=$1")
        .bind(run)
        .execute(store.pool())
        .await
        .unwrap();
    let tools=store.create_callback_task(&CreateCallbackTaskInput {flow_run_id:run,node_run_id:node.id,callback_kind:"llm_tool_calls".into(),request_payload:json!({"tool_calls":[{"id":"call_nul","name":"fixture","arguments":{}}]}),external_ref_payload:None}).await.unwrap();
    sqlx::query("insert into flow_run_tool_callback_inbox (id,scope_id,application_id,flow_run_id,callback_task_id,tool_call_id,tool_ordinal) values ($1,$2,$3,$4,$5,'call_nul',0)").bind(Uuid::now_v7()).bind(scope).bind(app).bind(run).bind(tools.id).execute(store.pool()).await.unwrap();
    let result =
        ToolCallbackResultInput::from_payload(json!({"tool_call_id":"call_nul","output":original}))
            .unwrap();
    let command = CommitToolCallbackResultsInput {
        scope_id: scope,
        application_id: app,
        flow_run_id: run,
        checkpoint_id: checkpoint.id,
        callback_task_id: tools.id,
        results: vec![result.clone()],
    };
    let committed = store.commit_tool_callback_results(&command).await.unwrap();
    let expected = json!({"tool_results":[result.result_payload]});
    assert_eq!(
        committed.callback_task.response_payload,
        Some(expected.clone())
    );
    assert_eq!(committed.claim.as_ref().unwrap().request_payload, expected);
    assert_eq!(
        store
            .commit_tool_callback_results(&command)
            .await
            .unwrap()
            .callback_task
            .response_payload,
        Some(expected.clone())
    );
    let claim = AcquireResumeClaimInput {
        scope_id: scope,
        application_id: app,
        flow_run_id: run,
        checkpoint_id: checkpoint.id,
        callback_task_id: Some(tools.id),
        kind: ResumeClaimKind::Callback,
        request_payload: expected,
    };
    assert_eq!(
        store
            .acquire_resume_claim(&claim)
            .await
            .unwrap()
            .claim
            .request_payload,
        claim.request_payload
    );
    let wrong = AcquireResumeClaimInput {
        request_payload: json!({"tool_results":"different"}),
        ..claim
    };
    assert!(store.acquire_resume_claim(&wrong).await.is_err());
}

#[tokio::test]
async fn runtime_json_public_history_and_trace_restore_originals() {
    let (store, run, app, scope) = seed().await;
    let key = Uuid::now_v7();
    sqlx::query("insert into api_keys (id,name,token_hash,token_prefix,creator_user_id,tenant_id,scope_kind,scope_id,key_kind,application_id) select $1,'NUL fixture',$1::text,'fixture',a.created_by,w.tenant_id,'workspace',w.id,'application_api_key',a.id from applications a join workspaces w on w.id=a.workspace_id where a.id=$2")
        .bind(key).bind(app).execute(store.pool()).await.unwrap();
    sqlx::query("update flow_runs set api_key_id=$2,external_user='nul-user',external_conversation_id='nul-history' where id=$1")
        .bind(run).bind(key).execute(store.pool()).await.unwrap();
    let query = "prompt\0and literal\\u0000";
    let answer = "answer\0and literal\\u0000";
    store
        .update_flow_run_payloads(&UpdateFlowRunPayloadsInput {
            flow_run_id: run,
            input_payload: json!({"query":query}),
            output_payload: json!({}),
            error_payload: None,
        })
        .await
        .unwrap();
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run,
            status: domain::FlowRunStatus::Succeeded,
            output_payload: json!({"answer":answer}),
            error_payload: None,
            finished_at: Some(OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
    let history = store
        .list_application_public_conversation_messages(
            &ListApplicationPublicConversationMessagesInput {
                application_id: app,
                api_key_id: key,
                external_user: "nul-user".into(),
                external_conversation_id: "nul-history".into(),
                limit: 20,
            },
        )
        .await
        .unwrap();
    assert!(history
        .iter()
        .any(|m| m.role == "user" && m.content == query));
    assert!(history
        .iter()
        .any(|m| m.role == "assistant" && m.content == answer));
    let sidecar:Value=sqlx::query_scalar("select raw_json_payloads from application_conversation_messages where flow_run_id=$1 and role='user'").bind(run).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        serde_json::from_str::<String>(sidecar["content"].as_str().unwrap()).unwrap(),
        query
    );
    let trace = Uuid::now_v7();
    let original = exceptional();
    store
        .replace_application_run_trace_projection(&ReplaceApplicationRunTraceProjectionInput {
            flow_run_id: run,
            projection_version: 1,
            source_watermark: "nul-fixture".into(),
            nodes: vec![ApplicationRunTraceNodeProjectionInput {
                trace_node_id: trace,
                parent_trace_node_id: None,
                stable_locator: "run:nul".into(),
                node_kind: "node_run".into(),
                owner_kind: Some("node_run".into()),
                owner_id: Some(run.to_string()),
                order_key: "1".into(),
                node_id: None,
                node_type: Some("llm".into()),
                node_mode: None,
                node_alias: "NUL fixture".into(),
                status: "succeeded".into(),
                started_at: OffsetDateTime::now_utc(),
                finished_at: None,
                duration_ms: None,
                metrics_payload: original.clone(),
                has_children: false,
                child_count: 0,
                has_content: true,
                content_ref: None,
                source_flow_run_id: None,
                source_trace_node_id: None,
                parent_callback_task_id: None,
                parent_tool_call_id: None,
                trace_relation_kind: None,
            }],
            contents: vec![ApplicationRunTraceNodeContentProjectionInput {
                trace_node_id: trace,
                content_kind: "json".into(),
                payload: original.clone(),
                source_refs: json!([]),
            }],
        })
        .await
        .unwrap();
    assert_eq!(
        store
            .get_application_run_trace_node_content(run, trace)
            .await
            .unwrap()
            .unwrap()
            .payload,
        original
    );
    let _ = scope;
}

#[tokio::test]
async fn runtime_json_native_facts_distinguish_nul_from_literal_and_keep_order() {
    let (store, seed_run, app, _scope) = seed().await;
    let base = store.get_flow_run(app, seed_run).await.unwrap().unwrap();
    let key = Uuid::now_v7();
    sqlx::query("insert into api_keys (id,name,token_hash,token_prefix,creator_user_id,tenant_id,scope_kind,scope_id,key_kind,application_id) select $1,'NUL native fixture',$1::text,'fixture',a.created_by,w.tenant_id,'workspace',w.id,'application_api_key',a.id from applications a join workspaces w on w.id=a.workspace_id where a.id=$2")
        .bind(key).bind(app).execute(store.pool()).await.unwrap();
    let log_context = ApplicationRunLogContext {
        identity_status: "identified".into(),
        protocol: Some("openai_responses".into()),
        thread_id: Some("nul-thread".into()),
        turn_id: Some("nul-turn".into()),
        session_id: Some("session-retained".into()),
        prompt: Some(json!({"type":"message","role":"user","content":"prompt\0exact"})),
        tool_results: vec![
            json!({"type":"function_call_output","call_id":"result_nul","output":"tool\0exact"}),
        ],
        ..Default::default()
    };
    let created = store
        .create_published_flow_run(&CreateFlowRunInput {
            application_run_log_context: Some(log_context),
            actor_user_id: base.created_by,
            application_id: app,
            flow_id: base.flow_id,
            flow_draft_id: base.draft_id,
            compiled_plan_id: base.compiled_plan_id.unwrap(),
            debug_session_id: String::new(),
            flow_schema_version: base.flow_schema_version,
            document_hash: base.document_hash,
            run_mode: domain::FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "NUL facts".into(),
            status: domain::FlowRunStatus::Running,
            input_payload: json!({"query":"prompt\0exact"}),
            started_at: OffsetDateTime::now_utc(),
            api_key_id: Some(key),
            publication_version_id: None,
            assistant_conversation_id: None,
            external_user: Some("facts-user".into()),
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: Some("openai-responses-v1".into()),
            idempotency_key: None,
        })
        .await
        .unwrap();
    let run = created.flow_run.id;
    let log:Value=sqlx::query_scalar("select runtime_original_json(log_context,raw_json_payloads,'log_context') from flow_runs where id=$1").bind(run).fetch_one(store.pool()).await.unwrap();
    assert_eq!(log["prompt"]["content"], json!("prompt\0exact"));
    assert_eq!(log["session_id"], json!("session-retained"));
    assert!(log["log_conversation_id"].is_string());
    let event = |id: &str, text: &str| AppendRuntimeEventInput {
        flow_run_id: run,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        event_type: "provider_output_item_done".into(),
        layer: domain::RuntimeEventLayer::ProviderRaw,
        source: domain::RuntimeEventSource::ProviderPlugin,
        trust_level: domain::RuntimeTrustLevel::ExternalOpaque,
        item_id: None,
        ledger_ref: None,
        payload: json!({"item":{"type":"message","id":id,"role":"assistant","content":[{"type":"output_text","text":text}]}}),
        visibility: domain::RuntimeEventVisibility::User,
        durability: domain::RuntimeEventDurability::Durable,
    };
    let events = vec![
        event("nul", "a\0b"),
        event("nul", "a\\u0000b"),
        event("ordinary", "same"),
        event("ordinary", "same"),
    ];
    let records = store.append_runtime_events(&events).await.unwrap();
    assert_eq!(records[0].payload, events[0].payload);
    let detail = store
        .get_application_run_detail(app, run)
        .await
        .unwrap()
        .unwrap();
    let nul = detail
        .native_messages
        .iter()
        .find(|message| message["_source_item"]["id"] == "nul")
        .unwrap();
    assert_eq!(nul["_source_item"]["content"][0]["text"], json!("a\0b"));
    assert_eq!(nul["_log_conflicting"], json!(true));
    let ordinary = detail
        .native_messages
        .iter()
        .find(|message| message["_source_item"]["id"] == "ordinary")
        .unwrap();
    assert_eq!(ordinary["_log_conflicting"], json!(false));
    let ids = detail
        .native_messages
        .iter()
        .filter_map(|message| message["_source_item"]["id"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["nul", "ordinary"]);
}
