use super::*;
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyActor,
    compat::openai::{
        translate_response_request_with_context_and_previous, OpenAiPreviousResponseContext,
        OpenAiResponsesEnvelope, OpenAiResponsesRequestContext,
    },
    native::ApplicationNativeRunService,
    native_tool_resume::correlate_semantic_responses_callback,
    protocol_translation::TranslationProtocol,
};
use control_plane_contracts::{
    application_public_api::ApplicationApiMappingConfig,
    application_public_runtime::ApplicationPublishedRunControlRepository,
    ports::{
        ApplicationPublicationRepository, CreateApplicationPublicationVersionInput,
        ProviderProtocolContextSlotId, ProviderProtocolContextValue, ProviderTransportStore,
    },
};
use plugin_framework::provider_contract::{
    ProviderFinishReason, ProviderInvocationResult, ProviderToolCall,
};

async fn actor(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    key: Uuid,
) -> ApplicationApiKeyActor {
    ApplicationApiKeyActor {
        api_key_id: key,
        application_id: seeded.application_id,
        creator_user_id: seeded.actor_user_id,
        tenant_id: root_tenant_id(store).await,
        workspace_id: seeded.workspace_id,
        actor: store
            .load_actor_context_for_user(seeded.actor_user_id)
            .await
            .unwrap(),
    }
}

// The full producer chain is intentional: a seeded responses_round would miss the bug.
#[tokio::test]
async fn semantic_cursor_producer_persists_discoverable_tool_round() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let (runtime, provider, _package) =
        semantic_runtime::seed_runtime_consumer(&store, &seeded, &compiled, None).await;
    // Use the actual mapped user query as the LLM prompt, rather than a checkpoint fixture.
    sqlx::query("update flow_compiled_plans set plan=jsonb_set(plan, '{nodes,node-llm,bindings,prompt_messages}', $2) where id=$1")
        .bind(compiled.id).bind(json!({"kind":"prompt_messages", "selector_paths":[["node-start","query"]],
            "raw_value":[{"id":"user","role":"user","content":{"kind":"templated_text","value":"{{node-start.query}}"}}]}))
        .execute(store.pool()).await.unwrap();
    let version = Uuid::now_v7();
    sqlx::query("insert into flow_versions (id,flow_id,sequence,trigger,change_kind,summary,document,created_by) select $1,$2,coalesce(max(sequence),0)+1,'autosave','logical','round fixture','{}'::jsonb,$3 from flow_versions where flow_id=$2")
        .bind(version).bind(seeded.flow_id).bind(seeded.actor_user_id).execute(store.pool()).await.unwrap();
    store.create_active_application_publication_version(&CreateApplicationPublicationVersionInput {
        actor_user_id: seeded.actor_user_id, application_id: seeded.application_id,
        mapping_snapshot: ApplicationApiMappingConfig::default_native(), extension_slug: None, api_enabled: true,
        compiled_plan_id: compiled.id, flow_id: seeded.flow_id, flow_version_id: version,
        flow_schema_version: "1flowbase.flow/v2".into(), document_hash: "round-fixture".into(),
        document_snapshot: json!({"graph":{"nodes":[{"id":"node-start","type":"start","config":{"model_list":["1flowbase"]}}]}}),
        runtime_profile_snapshot: json!({}), output_selector: json!({}), dependency_snapshot: vec![],
    }).await.unwrap();
    let key = seed_application_api_key(&store, &seeded).await;
    let actor = actor(&store, &seeded, key).await;
    let native = ApplicationNativeRunService::new(store.clone());
    let tools = json!([{"type":"function","name":"Bash","parameters":{"type":"object"}}]);
    let first = translate_response_request_with_context_and_previous(
        json!({"model":"1flowbase","input":[{"role":"user","content":"first"}],"tools":tools}),
        OpenAiResponsesRequestContext::responses(),
        None,
    )
    .unwrap();
    let first_protocol_context = first.request.client_protocol_envelope.clone();
    let first_run = native
        .create_native_run_for_actor(
            actor.clone(),
            first.request,
            TranslationProtocol::OpenAiResponses,
        )
        .await
        .unwrap();
    if let Some(envelope) = first_protocol_context {
        provider
            .transport_store
            .put_protocol_context(
                ProviderProtocolContextSlotId::for_original_flow_run(first_run.id),
                ProviderProtocolContextValue::from_envelope(envelope).unwrap(),
            )
            .await
            .unwrap();
    }
    provider.release.add_permits(1);
    let result = runtime
        .start_published_flow_run(
            control_plane::orchestration_runtime::StartPublishedFlowRunCommand {
                application_id: seeded.application_id,
                flow_run_id: first_run.id,
                provider_transport_slot: None,
                transport_connection_scope: None,
                observation_context: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(result.flow_run.status, FlowRunStatus::Succeeded);
    let first_id = format!("resp_{}", first_run.id);
    let mut contexts = store
        .find_published_responses_contexts(
            seeded.workspace_id,
            seeded.application_id,
            key,
            seeded.actor_user_id,
            &first_id,
        )
        .await
        .unwrap();
    let context = contexts.pop().unwrap();
    let original_proof = context
        .history
        .clone()
        .expect("full producer persisted compact proof");
    let second = translate_response_request_with_context_and_previous(
        json!({"model":"1flowbase","previous_response_id":first_id,"input":[{"role":"user","content":"use child"}],"tools":tools}),
        OpenAiResponsesRequestContext::responses(),
        Some(OpenAiPreviousResponseContext {
            history: context.history, provider_continuation: None, response_id: first_id,
            external_user: context.external_user, external_conversation_id: context.external_conversation_id,
            answer: context.answer,
        }),
    ).unwrap();
    let second_protocol_context = second.request.client_protocol_envelope.clone();
    let second_run = native
        .create_native_run_for_actor(
            actor.clone(),
            second.request,
            TranslationProtocol::OpenAiResponses,
        )
        .await
        .unwrap();
    if let Some(envelope) = second_protocol_context {
        provider
            .transport_store
            .put_protocol_context(
                ProviderProtocolContextSlotId::for_original_flow_run(second_run.id),
                ProviderProtocolContextValue::from_envelope(envelope).unwrap(),
            )
            .await
            .unwrap();
    }
    provider
        .results
        .lock()
        .unwrap()
        .push_back(ProviderInvocationResult {
            finish_reason: Some(ProviderFinishReason::ToolCall),
            tool_calls: vec![ProviderToolCall {
                id: "call-child-original".into(),
                name: "Bash".into(),
                arguments: json!({}),
                provider_metadata: json!({}),
            }],
            ..Default::default()
        });
    provider.release.add_permits(1);
    let waiting = runtime
        .start_published_flow_run(
            control_plane::orchestration_runtime::StartPublishedFlowRunCommand {
                application_id: seeded.application_id,
                flow_run_id: second_run.id,
                provider_transport_slot: None,
                transport_connection_scope: None,
                observation_context: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(waiting.flow_run.status, FlowRunStatus::WaitingCallback);
    let callbacks = store
        .find_semantic_responses_callbacks_by_call_ids(
            seeded.workspace_id,
            seeded.application_id,
            key,
            seeded.actor_user_id,
            &["call-child-original".into()],
        )
        .await
        .unwrap();
    assert_eq!(callbacks.len(), 1);
    let callback = &callbacks[0];
    let proof = &callback.request_payload["responses_round"]["history"];
    assert_eq!(proof["version"], 3);
    assert_eq!(
        proof["item_count"].as_u64().unwrap(),
        original_proof.item_count as u64 + 2
    );
    assert_eq!(proof["digest"].as_str().unwrap().len(), 64);
    let envelope = OpenAiResponsesEnvelope::capture(json!({"model":"1flowbase","previous_response_id":format!("resp_{}",second_run.id),
        "input":[{"type":"function_call_output","call_id":"call-child-original","output":"child started"}],"tools":tools})).unwrap();
    let grant = correlate_semantic_responses_callback(&store, &actor, &envelope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(grant.callback_task_id(), callback.id);
    assert!(provider
        .inputs
        .lock()
        .unwrap()
        .iter()
        .all(|input| input.native_transport.is_none()));
}

#[tokio::test]
async fn compact_previous_context_reads_exact_round_without_large_payloads() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        OffsetDateTime::now_utc(),
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let key = seed_application_api_key(&store, &seeded).await;
    let response_id = format!("resp_{}", run.id);
    let proof = current_callback_history_fixture();
    let answer = "preserve\0answer";
    let large_output = json!({"answer":answer,"responses_round":{"response_id":response_id,"history":proof,"output":[{"text":"z".repeat(2*1024*1024)}]},"irrelevant":"x".repeat(2*1024*1024)});
    let mut projection = large_output.clone();
    projection["answer"] = json!("lossy projection");
    sqlx::query("update flow_runs set api_key_id=$2,status='succeeded',output_payload=$3,raw_json_payloads=$4 where id=$1")
        .bind(run.id).bind(key).bind(projection).bind(json!({"output_payload":large_output.to_string()})).execute(store.pool()).await.unwrap();
    let lookup = || {
        store.find_published_responses_contexts(
            seeded.workspace_id,
            seeded.application_id,
            key,
            seeded.actor_user_id,
            &response_id,
        )
    };
    let contexts = lookup().await.unwrap();
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].answer.as_deref(), Some(answer));
    // EXPLAIN the adapter's actual statement. Public IDs must not execute historical
    // provider/node/span locator discovery, regardless of application history size.
    let adapter = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../storage/durable/postgres/src/orchestration_runtime_repository/repository_ports/responses_context.rs"));
    let statement = adapter
        .split_once("sqlx::query(r#\"")
        .unwrap()
        .1
        .split_once("\"#)")
        .unwrap()
        .0;
    // Audited: statement comes only from compile-time adapter source; values stay bound.
    let plan: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "explain (analyze, format json) {statement}"
    )))
    .bind(seeded.workspace_id)
    .bind(seeded.application_id)
    .bind(key)
    .bind(seeded.actor_user_id)
    .bind(&response_id)
    .bind(Some(run.id))
    .fetch_one(store.pool())
    .await
    .unwrap();
    fn assert_provider_fallback_skipped(value: &Value) -> bool {
        if value.get("Subplan Name").and_then(Value::as_str) == Some("CTE provider_run_ids") {
            assert_eq!(value["Actual Rows"].as_f64(), Some(0.0));
            fn assert_no_historical_reads(node: &Value) {
                if node
                    .get("Relation Name")
                    .and_then(Value::as_str)
                    .is_some_and(|relation| {
                        matches!(
                            relation,
                            "flow_runs" | "node_runs" | "node_run_details" | "runtime_spans"
                        )
                    })
                {
                    assert_eq!(
                        node["Actual Loops"].as_u64(),
                        Some(0),
                        "a scoped public PK match must skip historical provider discovery: {node}"
                    );
                }
                if let Some(children) = node.get("Plans").and_then(Value::as_array) {
                    for child in children {
                        assert_no_historical_reads(child);
                    }
                }
            }
            assert_no_historical_reads(value);
            return true;
        }
        match value {
            Value::Object(fields) => fields.values().any(assert_provider_fallback_skipped),
            Value::Array(items) => items.iter().any(assert_provider_fallback_skipped),
            _ => false,
        }
    }
    assert!(
        assert_provider_fallback_skipped(&plan),
        "EXPLAIN must include the actual fallback branch"
    );
    assert_eq!(
        serde_json::to_value(contexts[0].history.as_ref().unwrap()).unwrap(),
        proof
    );
    assert!(
        format!("{:?}", contexts).len() < 1024,
        "only identity, answer scalar and three-field proof cross the repository boundary"
    );
    assert!(store
        .find_published_responses_contexts(
            seeded.workspace_id,
            seeded.application_id,
            Uuid::now_v7(),
            seeded.actor_user_id,
            &response_id
        )
        .await
        .unwrap()
        .is_empty());
    assert!(store
        .find_published_responses_contexts(
            Uuid::now_v7(),
            seeded.application_id,
            key,
            seeded.actor_user_id,
            &response_id
        )
        .await
        .unwrap()
        .is_empty());
    assert!(store
        .find_published_responses_contexts(
            seeded.workspace_id,
            seeded.application_id,
            key,
            Uuid::now_v7(),
            &response_id
        )
        .await
        .unwrap()
        .is_empty());
    assert!(store
        .find_published_responses_contexts(
            seeded.workspace_id,
            seeded.application_id,
            key,
            seeded.actor_user_id,
            "resp_missing"
        )
        .await
        .unwrap()
        .is_empty());
    // Legacy rows stay explicit missing evidence; no reconstruction from the huge output.
    sqlx::query(
        "update flow_runs set output_payload=output_payload #- '{responses_round}' where id=$1",
    )
    .bind(run.id)
    .execute(store.pool())
    .await
    .unwrap();
    assert!(lookup().await.unwrap()[0].history.is_none());
    // Native terminal proof is selected only for this exact public round.
    let node = seed_node_run(&store, &run, OffsetDateTime::now_utc()).await;
    sqlx::query("update node_run_records set output_payload=$2 where id=$1").bind(node.id)
        .bind(json!({"provider_metadata":{"native_response":{"response_id":response_id,"history":proof}}})).execute(store.pool()).await.unwrap();
    let native_context = lookup().await.unwrap().pop().unwrap();
    assert!(native_context.history.is_some());
    translate_response_request_with_context_and_previous(
        json!({"model":"1flowbase","previous_response_id":response_id,"input":[{"role":"user","content":"semantic successor"}]}),
        OpenAiResponsesRequestContext::responses(),
        Some(OpenAiPreviousResponseContext {
            history: native_context.history, provider_continuation: None, response_id: response_id.clone(),
            external_user: None, external_conversation_id: None, answer: native_context.answer,
        }),
    ).expect("a native predecessor's compact public proof also supports the semantic successor");
    // Historical callback proof takes precedence over mutable flow/native output.
    let callback_id = Uuid::now_v7();
    sqlx::query("insert into flow_run_callback_tasks(id,flow_run_id,node_run_id,callback_kind,status,request_payload) values($1,$2,$3,'llm_tool_calls','pending',$4)")
        .bind(callback_id).bind(run.id).bind(node.id).bind(json!({"responses_round":{"response_id":response_id,"history":proof,"output":[]}})).execute(store.pool()).await.unwrap();
    sqlx::query("update node_run_records set output_payload='{}'::jsonb where id=$1")
        .bind(node.id)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(lookup().await.unwrap()[0].history.as_ref().unwrap()).unwrap(),
        proof
    );
    sqlx::query("update flow_run_callback_tasks set request_payload=jsonb_set(request_payload, '{responses_round,history,version}', '2'::jsonb) where id=$1")
        .bind(callback_id).execute(store.pool()).await.unwrap();
    assert!(
        lookup().await.is_err(),
        "invalid proof versions must not enter the translator"
    );
    sqlx::query("update flow_run_callback_tasks set request_payload=jsonb_set(request_payload, '{responses_round,history,version}', '3'::jsonb) where id=$1")
        .bind(callback_id).execute(store.pool()).await.unwrap();
    sqlx::query("insert into flow_run_callback_tasks(id,flow_run_id,node_run_id,callback_kind,status,request_payload) select $2,flow_run_id,node_run_id,callback_kind,status,request_payload from flow_run_callback_tasks where id=$1")
        .bind(callback_id).bind(Uuid::now_v7()).execute(store.pool()).await.unwrap();
    assert_eq!(
        lookup().await.unwrap().len(),
        2,
        "ambiguity must reach the admission owner, not disappear behind LIMIT 1"
    );
}
