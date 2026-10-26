use super::projection_cost::{event, setup};
use super::*;
use sqlx::{Column, Row, TypeInfo, ValueRef};

async fn persisted_events(store: &PgControlPlaneStore, run: Uuid) -> Vec<(Uuid, i64, Value)> {
    sqlx::query_as("select id,sequence,runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where flow_run_id=$1 order by sequence")
        .bind(run).fetch_all(store.pool()).await.unwrap()
}

async fn projection_snapshot(store: &PgControlPlaneStore, run: Uuid) -> Value {
    sqlx::query_scalar(r#"select jsonb_build_object(
        'messages',(select jsonb_agg(to_jsonb(m) order by display_sequence) from application_run_conversation_message_items m where flow_run_id=$1),
        'progress',(select to_jsonb(p) from application_run_native_projection_progress p where flow_run_id=$1),
        'summary',(select to_jsonb(s) from application_run_log_summaries s where flow_run_id=$1),
        'run',(select to_jsonb(f) from flow_runs f where id=$1))"#)
        .bind(run).fetch_one(store.pool()).await.unwrap()
}

// The archive ownership change must preserve both borrowed ordinary Values and
// transformed owned Values. Existing append_regressions remain the full-fold,
// metadata and controlled-defect oracle; this test adds exact returned bodies.
#[tokio::test]
async fn archive_ownership_preserves_exact_returns_and_projection_failure_rollback() {
    let (store, run) = setup().await;
    let mut inputs = vec![
        event(
            run.id,
            "lossless-a".into(),
            "actual\0 literal \\u0000 中文 🦀".into(),
        ),
        event(run.id, "lossless-b".into(), "full body".repeat(4096)),
    ];
    inputs[0].payload["item"]["extra"] = json!({
        "nul\0key":[null,true,false,0,-1,u64::MAX,1.25e100,{"nested":"\0\\u0000"}],
        "unknown":{"array":[],"object":{}},
    });
    for payload in [
        json!({"source":"external","missing_body":[1,null]}),
        json!({"body_ref":"already-referenced","body":{"keep":"inline"}}),
        json!({"nul\0key":true,"body":{"keep":"exceptional"}}),
        json!({"source":"external","kind":"observation","body":{"text":"archive exact\0 body","array":[null,42]}}),
    ] {
        let mut input = event(run.id, "unused".into(), String::new());
        input.event_type = "provider_protocol_observation".into();
        input.payload = payload;
        inputs.push(input);
    }
    let records = store.append_runtime_events(&inputs).await.unwrap();
    assert_eq!(records.len(), inputs.len());
    for (input, record) in inputs.iter().zip(&records) {
        assert_eq!(record.payload, input.payload);
        assert_eq!(record.flow_run_id, input.flow_run_id);
        assert_eq!(record.node_run_id, input.node_run_id);
        assert_eq!(record.span_id, input.span_id);
        assert_eq!(record.parent_span_id, input.parent_span_id);
        assert_eq!(record.event_type, input.event_type);
        assert_eq!(record.layer, input.layer);
        assert_eq!(record.source, input.source);
        assert_eq!(record.trust_level, input.trust_level);
        assert_eq!(record.item_id, input.item_id);
        assert_eq!(record.ledger_ref, input.ledger_ref);
        assert_eq!(record.visibility, input.visibility);
        assert_eq!(record.durability, input.durability);
    }
    let persisted = persisted_events(&store, run.id).await;
    assert_eq!(persisted.len(), records.len());
    for ((id, sequence, payload), record) in persisted.iter().zip(&records) {
        assert_eq!(*id, record.id);
        assert_eq!(*sequence, record.sequence);
        assert_eq!(*payload, record.payload);
    }
    for pair in records.windows(2) {
        assert!(pair[0].sequence < pair[1].sequence);
        assert_ne!(pair[0].id, pair[1].id);
    }
    let bodies: Vec<Value> = sqlx::query_scalar("select runtime_original_json(native_message,raw_json_payloads,'native_message') from application_run_conversation_message_items where flow_run_id=$1 and source_occurrence_sequence is not null order by display_sequence")
        .bind(run.id).fetch_all(store.pool()).await.unwrap();
    assert_eq!(bodies.len(), 2);
    for (index, body) in bodies.iter().enumerate() {
        assert_eq!(body["_source_item"], inputs[index].payload["item"]);
        assert_eq!(body["content"], inputs[index].payload["item"]["content"]);
        assert_eq!(body["_log_source_sequence"], records[index].sequence);
        assert_eq!(body["_log_conflicting"], false);
        assert!(body["_log_source_revision"]
            .as_str()
            .unwrap()
            .starts_with("v7:"));
    }
    let before = projection_snapshot(&store, run.id).await;
    // This is a SAME-RUN batch. The mixed-run fallback retains its existing
    // independent per-event transactions; it is not a whole-batch guarantee.
    // Fail after runtime event insertion/RETURNING, at the projection write.
    // All materialization and projection failures must still precede commit.
    sqlx::raw_sql(
        r#"
        create function history_memory_reject_progress() returns trigger language plpgsql as $$
        begin raise exception 'history-memory controlled projection failure'; end $$;
        create trigger history_memory_reject_progress before insert or update
            on application_run_native_projection_progress for each row
            execute function history_memory_reject_progress();
    "#,
    )
    .execute(store.pool())
    .await
    .unwrap();
    let failed = [event(
        run.id,
        "must-rollback".into(),
        "rollback body\0".into(),
    )];
    assert!(store.append_runtime_events(&failed).await.is_err());
    assert_eq!(persisted_events(&store, run.id).await, persisted);
    assert_eq!(projection_snapshot(&store, run.id).await, before);
    sqlx::raw_sql("drop trigger history_memory_reject_progress on application_run_native_projection_progress; drop function history_memory_reject_progress()")
        .execute(store.pool()).await.unwrap();
    let returned = store.append_runtime_events(&failed).await.unwrap();
    assert_eq!(returned[0].payload, failed[0].payload);
    assert_eq!(returned[0].sequence, records.last().unwrap().sequence + 1);
}

// Mixed-run batches retain independent per-event transactions: an error leaves
// the committed prefix visible and must not execute the remaining inputs.
#[tokio::test]
async fn mixed_run_append_failure_keeps_committed_prefix_and_skips_tail() {
    let (store, run) = setup().await;
    assert!(persisted_events(&store, run.id).await.is_empty());
    let missing_run = Uuid::now_v7();
    let exists: bool = sqlx::query_scalar("select exists(select 1 from flow_runs where id=$1)")
        .bind(missing_run)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert!(!exists);
    let inputs = [
        event(run.id, "mixed-prefix".into(), "committed exact\0".into()),
        event(missing_run, "mixed-failure".into(), "missing run".into()),
        event(run.id, "mixed-tail".into(), "later literal \\u0000".into()),
    ];

    assert!(store.append_runtime_events(&inputs).await.is_err());
    let committed = persisted_events(&store, run.id).await;
    assert_eq!(committed.len(), 1, "only the successful prefix may persist");
    assert_eq!(committed[0].2, inputs[0].payload);
    let bodies: Vec<Value> = sqlx::query_scalar(
        "select runtime_original_json(native_message,raw_json_payloads,'native_message') from application_run_conversation_message_items where flow_run_id=$1 and source_occurrence_sequence is not null order by display_sequence",
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        bodies.len(),
        1,
        "the prefix projection must commit with its event"
    );
    assert_eq!(bodies[0]["_source_item"], inputs[0].payload["item"]);

    let tail = store.append_runtime_events(&inputs[2..]).await.unwrap();
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].payload, inputs[2].payload);
    assert_eq!(tail[0].sequence, committed[0].1 + 1);
    let after = persisted_events(&store, run.id).await;
    assert_eq!(after.len(), 2);
    assert_eq!(after[0], committed[0]);
    assert_eq!(after[1].2, inputs[2].payload);
}

// Execute the production literals, including the historical-owner predicate;
// the old restore function remains the independent value/error oracle.
fn metadata_queries() -> (String, String) {
    let completed = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../storage/durable/postgres/src/orchestration_runtime_repository/application_run_logs/run_conversation_message_item_methods.rs"));
    let canonical = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../storage/durable/postgres/src/orchestration_runtime_repository/application_run_logs/projection/native_canonical_projection.rs"));
    let literal = |source: &str, anchor: &str, call: &str| {
        let source = source.split_once(anchor).unwrap().1;
        let source = source.split_once(call).unwrap().1.trim_start();
        serde_json::Deserializer::from_str(source)
            .into_iter::<String>()
            .next()
            .unwrap()
            .unwrap()
    };
    (
        literal(
            completed,
            "async fn fetch_application_run_projection_flow_run(",
            "sqlx::query(",
        ),
        literal(
            canonical,
            "let mut context: Option<Value> = if owner == run.id {",
            "sqlx::query_scalar(",
        ),
    )
}

#[tokio::test]
async fn metadata_owner_query_preserves_named_restored_fields_and_sql_nulls() {
    let (store, run) = setup().await;
    let (query, _) = metadata_queries();
    // Frozen pre-change wildcard query: duplicated payload names deliberately
    // distinguish SQLx's named lookup from the physical JSONB columns.
    let baseline = "select f.*,runtime_original_json(f.input_payload,f.raw_json_payloads,'input_payload') as input_payload,runtime_original_json(f.output_payload,f.raw_json_payloads,'output_payload') as output_payload,runtime_original_json(f.error_payload,f.raw_json_payloads,'error_payload') as error_payload,(select account from users where id=f.created_by) as authorized_account from flow_runs f where id=$1";
    let fields = [
        "id",
        "application_id",
        "flow_id",
        "flow_draft_id",
        "compiled_plan_id",
        "debug_session_id",
        "flow_schema_version",
        "document_hash",
        "run_mode",
        "target_node_id",
        "title",
        "status",
        "input_payload",
        "output_payload",
        "error_payload",
        "created_by",
        "authorized_account",
        "api_key_id",
        "publication_version_id",
        "external_user",
        "external_conversation_id",
        "external_trace_id",
        "compatibility_mode",
        "idempotency_key",
        "started_at",
        "finished_at",
        "created_at",
        "updated_at",
    ];
    let input = json!({"sentinel":"restored\0 and literal \\u0000"});
    let output = json!([null, true, 42, {"restored":"output\0"}]);
    let error = json!({"message":"restored error\0"});
    let raw_input = serde_json::to_string(&input).unwrap();
    for (physical_output, physical_error, raw, expected_output, expected_error) in [
        (
            Some(json!({"sentinel":"physical output"})),
            Some(json!({"sentinel":"physical error"})),
            json!({"input_payload":raw_input,"output_payload":serde_json::to_string(&output).unwrap(),"error_payload":serde_json::to_string(&error).unwrap()}),
            Some(output),
            Some(error),
        ),
        (
            Some(Value::Null),
            None,
            json!({"input_payload":raw_input}),
            Some(Value::Null),
            None,
        ),
        (
            Some(json!({"must":"not replace SQL NULL"})),
            Some(json!({"physical":true})),
            json!({"input_payload":raw_input,"output_payload":null,"error_payload":"null"}),
            None,
            Some(Value::Null),
        ),
    ] {
        sqlx::query("update flow_runs set input_payload=$2,output_payload=$3,error_payload=$4,raw_json_payloads=$5 where id=$1")
            .bind(run.id).bind(json!({"sentinel":"physical input"}))
            .bind(physical_output).bind(physical_error).bind(raw)
            .execute(store.pool()).await.unwrap();
        let old = sqlx::query(baseline)
            .bind(run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
        let actual = sqlx::query(sqlx::AssertSqlSafe(query.clone()))
            .bind(run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(
            actual
                .columns()
                .iter()
                .map(|column| column.name())
                .collect::<Vec<_>>(),
            fields
        );
        for field in fields {
            assert_eq!(
                actual.column(field).type_info().name(),
                old.column(field).type_info().name(),
                "{field}"
            );
            let left = actual.try_get_raw(field).unwrap();
            let right = old.try_get_raw(field).unwrap();
            assert_eq!(left.is_null(), right.is_null(), "{field}");
            if !left.is_null() {
                assert_eq!(
                    left.as_bytes().unwrap(),
                    right.as_bytes().unwrap(),
                    "{field}"
                );
            }
        }
        assert_eq!(actual.try_get::<Value, _>("input_payload").unwrap(), input);
        assert_eq!(
            actual
                .try_get::<Option<Value>, _>("output_payload")
                .unwrap(),
            expected_output
        );
        assert_eq!(
            actual.try_get::<Option<Value>, _>("error_payload").unwrap(),
            expected_error
        );
        for field in ["input_payload", "output_payload", "error_payload"] {
            let indices = old
                .columns()
                .iter()
                .enumerate()
                .filter_map(|(index, column)| (column.name() == field).then_some(index))
                .collect::<Vec<_>>();
            assert_eq!(indices.len(), 2);
            assert_eq!(old.columns()[indices[0]].type_info().name(), "JSONB");
            assert_eq!(old.columns()[indices[1]].type_info().name(), "JSON");
            assert_eq!(
                old.try_get::<Option<Value>, _>(field).unwrap(),
                old.try_get::<Option<Value>, _>(indices[1]).unwrap()
            );
        }
        let physical = old
            .columns()
            .iter()
            .position(|column| column.name() == "input_payload")
            .unwrap();
        assert_ne!(old.try_get::<Value, _>(physical).unwrap(), input);
    }
}

#[tokio::test]
async fn metadata_context_query_preserves_restore_fallback_and_reduces_wire() {
    let (store, run) = setup().await;
    let (_, query) = metadata_queries();
    // Shadow only the relation so SQL NULL and non-object inputs can exercise
    // the exact production CASE without changing storage constraints.
    let query = format!("with flow_runs as (select $1::uuid as id,$2::jsonb as log_context,$3::jsonb as raw_json_payloads) {query}");
    let metadata = json!({"keep":"literal \\u0000","nested":{"tool_results":[1]}});
    let mut context = metadata.clone();
    context["tool_results"] = json!([{"output":"x".repeat(4096)}]);
    let restored =
        json!({"keep":"actual\0 and literal \\u0000","tool_results":[{"output":"restored\0"}]});
    let sidecar = json!({"log_context":serde_json::to_string(&restored).unwrap()});
    let cases = [
        (
            "object-no-sidecar",
            Some(context.clone()),
            Some(json!({})),
            Some(metadata.clone()),
        ),
        (
            "SQL-null-sidecar",
            Some(context.clone()),
            None,
            Some(metadata.clone()),
        ),
        (
            "JSON-null-sidecar",
            Some(context.clone()),
            Some(Value::Null),
            Some(metadata.clone()),
        ),
        (
            "present-original",
            Some(context.clone()),
            Some(sidecar.clone()),
            Some(restored.clone()),
        ),
        (
            "present-JSON-null",
            Some(context.clone()),
            Some(json!({"log_context":null})),
            None,
        ),
        (
            "restored-JSON-null",
            Some(context.clone()),
            Some(json!({"log_context":"null"})),
            Some(Value::Null),
        ),
        (
            "array-fallback",
            Some(json!([{"tool_results":[1]}])),
            Some(json!({})),
            Some(json!([{"tool_results":[1]}])),
        ),
        (
            "string-fallback",
            Some(json!("literal \\u0000")),
            Some(json!({})),
            Some(json!("literal \\u0000")),
        ),
        (
            "JSON-null-context",
            Some(Value::Null),
            Some(json!({})),
            Some(Value::Null),
        ),
        ("SQL-null-context", None, Some(json!({})), None),
        (
            "non-object-original",
            Some(json!([1])),
            Some(sidecar.clone()),
            Some(restored.clone()),
        ),
        (
            "SQL-null-with-original",
            None,
            Some(sidecar),
            Some(restored),
        ),
    ];
    for (label, context, raw, expected) in cases {
        let actual = sqlx::query(sqlx::AssertSqlSafe(query.clone()))
            .bind(run.id)
            .bind(&context)
            .bind(&raw)
            .fetch_one(store.pool())
            .await
            .unwrap();
        let old = sqlx::query("select runtime_original_json($1::jsonb,$2::jsonb,'log_context')")
            .bind(&context)
            .bind(&raw)
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(actual.column(0).type_info().name(), "JSON", "{label}");
        let mut actual_value: Option<Value> = actual.try_get(0).unwrap();
        assert_eq!(actual_value, expected, "{label}");
        let mut old_value: Option<Value> = old.try_get(0).unwrap();
        // This is the unchanged client-side remove, applied to both wire values.
        for value in [&mut actual_value, &mut old_value] {
            if let Some(object) = value.as_mut().and_then(Value::as_object_mut) {
                object.remove("tool_results");
            }
        }
        assert_eq!(actual_value, old_value, "{label}");
        if label == "object-no-sidecar" {
            let old_bytes = old.try_get_raw(0).unwrap().as_bytes().unwrap().len();
            let new_bytes = actual.try_get_raw(0).unwrap().as_bytes().unwrap().len();
            assert!(
                old_bytes >= new_bytes + 4096,
                "metadata must omit the result body on the wire"
            );
        }
    }
    let invalid = json!({"log_context":"{invalid"});
    let actual = sqlx::query(sqlx::AssertSqlSafe(query))
        .bind(run.id)
        .bind(&context)
        .bind(&invalid)
        .fetch_one(store.pool())
        .await
        .unwrap_err();
    let old = sqlx::query("select runtime_original_json($1::jsonb,$2::jsonb,'log_context')")
        .bind(&context)
        .bind(&invalid)
        .fetch_one(store.pool())
        .await
        .unwrap_err();
    assert_eq!(
        actual.as_database_error().unwrap().code(),
        old.as_database_error().unwrap().code()
    );
    assert_eq!(
        actual.as_database_error().unwrap().code().as_deref(),
        Some("22P02")
    );
}

#[tokio::test]
async fn metadata_rebuild_preserves_historical_owner_payloads_prompts_and_results() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let mut historical = Vec::new();
    for (index, text) in ["literal \\u0000", "actual\0 and literal \\u0000"]
        .into_iter()
        .enumerate()
    {
        let result = json!({"type":"function_call_output","call_id":format!("metadata-{index}"),"output":text});
        let run = native_member(
            &store,
            &seeded,
            &compiled,
            key,
            index,
            "metadata-rebuild",
            vec![result.clone()],
        )
        .await;
        store.update_flow_run_payloads(&UpdateFlowRunPayloadsInput {
            flow_run_id: run.id,
            input_payload: json!({"__native_model_prompt_context":{"system":[{"text":format!("system-{index}\0")}],"messages":[]}}),
            output_payload: json!({"answer":format!("answer-{index}\0")}),
            error_payload: None,
        }).await.unwrap();
        let has_original: bool = sqlx::query_scalar(
            "select raw_json_payloads ? 'log_context' from flow_runs where id=$1",
        )
        .bind(run.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(has_original, index == 1);
        let original: Value = sqlx::query_scalar("select runtime_original_json(log_context,raw_json_payloads,'log_context') from flow_runs where id=$1")
            .bind(run.id).fetch_one(store.pool()).await.unwrap();
        historical.push((run, result, original));
    }
    let current = native_member(
        &store,
        &seeded,
        &compiled,
        key,
        2,
        "metadata-rebuild",
        vec![],
    )
    .await;
    let ids = historical
        .iter()
        .map(|(run, _, _)| run.id)
        .chain([current.id])
        .collect::<Vec<_>>();
    sqlx::query("update application_run_native_projection_progress set invalidated=true where flow_run_id=any($1)")
        .bind(&ids).execute(store.pool()).await.unwrap();
    // Real completed-output entry -> shared fetch -> full canonical rebuild,
    // exercising both the guarded and sidecar fallback historical-owner reads.
    store
        .append_runtime_events(&[event(
            current.id,
            "metadata-current".into(),
            "current body".into(),
        )])
        .await
        .unwrap();
    for (index, (run, result, original)) in historical.into_iter().enumerate() {
        let fact = canonical(&store, run.id, &format!("result:metadata-{index}"))
            .await
            .unwrap();
        assert_eq!(fact["_source_item"], result);
        let messages: Vec<Value> = sqlx::query_scalar("select runtime_original_json(native_message,raw_json_payloads,'native_message') from application_run_conversation_message_items where flow_run_id=$1 order by display_sequence")
            .bind(run.id).fetch_all(store.pool()).await.unwrap();
        for expected in [
            format!("system-{index}\0"),
            format!("question-{index}"),
            format!("answer-{index}\0"),
        ] {
            assert_eq!(
                messages
                    .iter()
                    .filter(|message| message["content"] == expected)
                    .count(),
                1,
                "owner {index}: {expected:?}"
            );
        }
        let retained: Value = sqlx::query_scalar("select runtime_original_json(log_context,raw_json_payloads,'log_context') from flow_runs where id=$1")
            .bind(run.id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(
            retained, original,
            "metadata reads must not mutate retained facts"
        );
    }
    let read_contexts: i64 = sqlx::query_scalar("select last_read_context_count from application_run_native_projection_progress where flow_run_id=$1")
        .bind(current.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        read_contexts, 5,
        "three first-pass facts plus two historical metadata reads"
    );
}
