use super::*;

async fn native_member(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    key: Uuid,
    index: usize,
    thread: &str,
    results: Vec<Value>,
) -> domain::FlowRunRecord {
    ApplicationPublishedFlowRunRepository::create_published_flow_run(
        store,
        &CreateFlowRunInput {
            application_run_log_context: Some(ApplicationRunLogContext {
                identity_status: "identified".into(),
                protocol: Some("openai_responses".into()),
                thread_id: Some(thread.into()),
                turn_id: Some(format!("turn-{index}")),
                prompt: Some(json!({"role":"user","content":format!("question-{index}")})),
                tool_results: results,
                ..Default::default()
            }),
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: format!("incremental-{thread}-{index}-{key}"),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "incremental native projection".into(),
            status: FlowRunStatus::Running,
            input_payload: json!({"node-start":{"query":format!("question-{index}")}}),
            started_at: datetime!(2026-10-09 00:00:00 UTC) + Duration::seconds(index as i64),
            api_key_id: Some(key),
            publication_version_id: Some(Uuid::now_v7()),
            assistant_conversation_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: Some("openai-responses".into()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap()
    .flow_run
}

async fn canonical(store: &PgControlPlaneStore, owner: Uuid, key: &str) -> Option<Value> {
    sqlx::query_scalar("select runtime_original_json(native_message,raw_json_payloads,'native_message') from application_run_conversation_message_items where flow_run_id=$1 and source_item_key=$2")
        .bind(owner).bind(key).fetch_optional(store.pool()).await.unwrap()
}

#[tokio::test]
async fn incremental_duplicate_history_reads_only_current_context_and_new_events() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let original = json!({"type":"function_call_output","call_id":"same-result","output":"real\u{0}NUL and literal \\u0000"});
    let mut first = None;
    let mut first_revision = None;
    for index in 0..12 {
        let run = native_member(
            &store,
            &seeded,
            &compiled,
            key,
            index,
            "duplicate-history",
            vec![original.clone()],
        )
        .await;
        let page =
            run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
        assert_eq!(
            page.items[0].query.as_deref(),
            Some(format!("question-{index}").as_str())
        );
        let (contexts,events):(i64,i64)=sqlx::query_as("select last_read_context_count,last_read_event_count from application_run_native_projection_progress where flow_run_id=$1")
            .bind(run.id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(
            (contexts, events),
            (1, 0),
            "new calls cannot decode accumulated duplicate contexts"
        );
        if let Some(owner) = first {
            let revision:i64=sqlx::query_scalar("select source_revision from application_run_native_projection_progress where flow_run_id=$1")
                .bind(owner).fetch_one(store.pool()).await.unwrap();
            assert_eq!(
                Some(revision),
                first_revision,
                "unchanged historical progress is untouched"
            );
            assert!(canonical(&store, run.id, "result:same-result")
                .await
                .is_none());
        } else {
            first = Some(run.id);
            first_revision=Some(sqlx::query_scalar("select source_revision from application_run_native_projection_progress where flow_run_id=$1").bind(run.id).fetch_one(store.pool()).await.unwrap());
        }
        append_provider_output_item(
            &store,
            run.id,
            json!({"type":"custom_tool_call","call_id":format!("unique-{index}"),"input":"x"}),
        )
        .await;
        let counts:(i64,i64)=sqlx::query_as("select last_read_context_count,last_read_event_count from application_run_native_projection_progress where flow_run_id=$1")
            .bind(run.id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(counts, (1, 1), "only the new completed event is returned");
    }
    let kept = canonical(&store, first.unwrap(), "result:same-result")
        .await
        .unwrap();
    assert_eq!(kept["_source_item"], original);
    assert_eq!(kept["_log_conflicting"], false);
}

#[tokio::test]
async fn incremental_late_earlier_owner_moves_exact_nul_fact_and_refreshes_both_tasks() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let early = native_member(&store, &seeded, &compiled, key, 0, "late-owner", vec![]).await;
    let late = native_member(&store, &seeded, &compiled, key, 1, "late-owner", vec![]).await;
    assert!(early.id < late.id);
    let literal = json!({"type":"message","id":"same","role":"assistant","phase":"final_answer","content":"literal \\u0000"});
    let actual = json!({"type":"message","id":"same","role":"assistant","phase":"final_answer","content":"actual \u{0}"});
    append_provider_output_item(&store, late.id, literal.clone()).await;
    append_provider_output_item(&store, early.id, actual.clone()).await;
    let kept = canonical(&store, early.id, "output:message:same")
        .await
        .unwrap();
    assert_eq!(kept["_source_item"], actual);
    assert_eq!(kept["_log_conflicting"], true);
    assert!(canonical(&store, late.id, "output:message:same")
        .await
        .is_none());
    for run in [&early, &late] {
        let task = store
            .get_application_run_log_task(seeded.application_id, run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(task.member_run_ids, vec![run.id]);
        let page =
            run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
        assert_eq!(page.items.len(), 1);
    }
    // Deleting the earliest occurrence invalidates the complete authorized
    // group. Rebuild moves ownership back but retains the observed conflict.
    sqlx::query("delete from runtime_events where flow_run_id=$1 and event_type='provider_output_item_done'")
        .bind(early.id).execute(store.pool()).await.unwrap();
    store
        .update_flow_run_payloads(&UpdateFlowRunPayloadsInput {
            flow_run_id: early.id,
            input_payload: early.input_payload.clone(),
            output_payload: json!({}),
            error_payload: None,
        })
        .await
        .unwrap();
    let kept = canonical(&store, late.id, "output:message:same")
        .await
        .unwrap();
    assert_eq!(kept["_source_item"], literal);
    assert_eq!(kept["_log_conflicting"], true);
    assert!(canonical(&store, early.id, "output:message:same")
        .await
        .is_none());
}

#[tokio::test]
async fn incremental_legacy_rebuild_and_authorization_domains_keep_exact_originals() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let first_key = seed_application_api_key(&store, &seeded).await;
    let other_key = seed_application_api_key(&store, &seeded).await;
    let result = json!({"type":"function_call_output","call_id":"isolated","output":"x\u{0}"});
    let first = native_member(
        &store,
        &seeded,
        &compiled,
        first_key,
        0,
        "same-thread",
        vec![result.clone()],
    )
    .await;
    let other = native_member(
        &store,
        &seeded,
        &compiled,
        other_key,
        1,
        "same-thread",
        vec![json!({"type":"function_call_output","call_id":"isolated","output":"other"})],
    )
    .await;
    assert_eq!(
        canonical(&store, first.id, "result:isolated")
            .await
            .unwrap()["_log_conflicting"],
        false
    );
    assert_eq!(
        canonical(&store, other.id, "result:isolated")
            .await
            .unwrap()["_source_item"]["output"],
        "other"
    );
    sqlx::query("delete from application_run_native_projection_progress where flow_run_id=$1")
        .bind(first.id)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("update application_run_conversation_message_items set projection_version=6,source_occurrence_sequence=null where flow_run_id=$1").bind(first.id).execute(store.pool()).await.unwrap();
    store
        .update_flow_run_payloads(&UpdateFlowRunPayloadsInput {
            flow_run_id: first.id,
            input_payload: first.input_payload.clone(),
            output_payload: json!({}),
            error_payload: None,
        })
        .await
        .unwrap();
    assert_eq!(
        canonical(&store, first.id, "result:isolated")
            .await
            .unwrap()["_source_item"],
        result
    );
    assert_eq!(
        canonical(&store, other.id, "result:isolated")
            .await
            .unwrap()["_log_conflicting"],
        false
    );
    let foreign_conversation: String =
        sqlx::query_scalar("select log_context->>'log_conversation_id' from flow_runs where id=$1")
            .bind(other.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    sqlx::query("update flow_runs set log_context=jsonb_set(log_context,'{log_conversation_id}',to_jsonb($2::text)) where id=$1")
        .bind(first.id).bind(foreign_conversation).execute(store.pool()).await.unwrap();
    assert!(
        store
            .update_flow_run_payloads(&UpdateFlowRunPayloadsInput {
                flow_run_id: first.id,
                input_payload: first.input_payload.clone(),
                output_payload: json!({}),
                error_payload: None,
            })
            .await
            .is_err(),
        "a retained pointer cannot bypass the authenticated conversation domain"
    );
    assert_eq!(
        canonical(&store, other.id, "result:isolated")
            .await
            .unwrap()["_source_item"]["output"],
        "other"
    );
}

#[tokio::test]
async fn incremental_projection_failure_rolls_back_fact_and_progress_then_retry_succeeds() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let run = native_member(&store, &seeded, &compiled, key, 0, "rollback", vec![]).await;
    let before:(i64,i64)=sqlx::query_as("select output_sequence,source_revision from application_run_native_projection_progress where flow_run_id=$1").bind(run.id).fetch_one(store.pool()).await.unwrap();
    sqlx::query("create function reject_native_fixture() returns trigger language plpgsql as $$ begin raise exception 'fixture projection write failure'; end $$").execute(store.pool()).await.unwrap();
    sqlx::query("create trigger reject_native_fixture before insert on application_run_conversation_message_items for each row execute function reject_native_fixture()")
        .execute(store.pool()).await.unwrap();
    let input = AppendRuntimeEventInput {
        flow_run_id: run.id,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        event_type: "provider_output_item_done".into(),
        layer: domain::RuntimeEventLayer::AgentTransition,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
        payload: json!({"item":{"type":"message","id":"retry","content":"done"}}),
    };
    assert!(store.append_runtime_event(&input).await.is_err());
    let after:(i64,i64)=sqlx::query_as("select output_sequence,source_revision from application_run_native_projection_progress where flow_run_id=$1").bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(before, after);
    let count:i64=sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1 and event_type='provider_output_item_done'").bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(count, 0);
    sqlx::query("drop trigger reject_native_fixture on application_run_conversation_message_items")
        .execute(store.pool())
        .await
        .unwrap();
    store.append_runtime_event(&input).await.unwrap();
    assert_eq!(
        canonical(&store, run.id, "output:message:retry")
            .await
            .unwrap()["_source_item"]["content"],
        "done"
    );
}

#[tokio::test]
async fn incremental_nonappend_result_correction_and_owner_deletion_rebuild_survivors() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let result = json!({"type":"function_call_output","call_id":"corrected","output":"first"});
    let first = native_member(
        &store,
        &seeded,
        &compiled,
        key,
        0,
        "nonappend",
        vec![result.clone()],
    )
    .await;
    let second = native_member(
        &store,
        &seeded,
        &compiled,
        key,
        1,
        "nonappend",
        vec![result.clone()],
    )
    .await;
    let appended = json!({"type":"function_call_output","call_id":"appended","output":"next"});
    sqlx::query("update flow_runs set log_context=jsonb_set(log_context,'{tool_results}',$2::jsonb) where id=$1")
        .bind(second.id).bind(json!([result.clone(),appended.clone()])).execute(store.pool()).await.unwrap();
    store
        .update_flow_run_payloads(&UpdateFlowRunPayloadsInput {
            flow_run_id: second.id,
            input_payload: second.input_payload.clone(),
            output_payload: json!({}),
            error_payload: None,
        })
        .await
        .unwrap();
    let counts:(i64,i64,i64)=sqlx::query_as("select result_count,last_read_context_count,last_read_event_count from application_run_native_projection_progress where flow_run_id=$1")
        .bind(second.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        counts,
        (2, 1, 0),
        "append-only results do not rebuild historical contexts"
    );
    assert_eq!(
        canonical(&store, second.id, "result:appended")
            .await
            .unwrap()["_source_item"],
        appended
    );
    assert!(canonical(&store, second.id, "result:corrected")
        .await
        .is_none());
    let correction =
        json!({"type":"function_call_output","call_id":"corrected","output":"changed"});
    sqlx::query("update flow_runs set log_context=jsonb_set(log_context,'{tool_results}',$2::jsonb),raw_json_payloads=raw_json_payloads-'log_context' where id=$1")
        .bind(first.id).bind(json!([correction.clone()])).execute(store.pool()).await.unwrap();
    let invalidated: bool = sqlx::query_scalar(
        "select invalidated from application_run_native_projection_progress where flow_run_id=$1",
    )
    .bind(second.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(invalidated, "a correction invalidates surviving members");
    store
        .update_flow_run_payloads(&UpdateFlowRunPayloadsInput {
            flow_run_id: first.id,
            input_payload: first.input_payload.clone(),
            output_payload: json!({}),
            error_payload: None,
        })
        .await
        .unwrap();
    let kept = canonical(&store, first.id, "result:corrected")
        .await
        .unwrap();
    assert_eq!(kept["_source_item"], correction);
    assert_eq!(kept["_log_conflicting"], true);
    // Removing the body-owning run must not lose the historical conflict
    // observation when its projection and progress cascade away.
    sqlx::query("delete from flow_runs where id=$1")
        .bind(first.id)
        .execute(store.pool())
        .await
        .unwrap();
    store
        .update_flow_run_payloads(&UpdateFlowRunPayloadsInput {
            flow_run_id: second.id,
            input_payload: second.input_payload.clone(),
            output_payload: json!({}),
            error_payload: None,
        })
        .await
        .unwrap();
    let kept = canonical(&store, second.id, "result:corrected")
        .await
        .unwrap();
    assert_eq!(kept["_source_item"], result);
    assert_eq!(
        kept["_log_conflicting"], true,
        "deleting an owner cannot erase an observed conflict"
    );
}

#[tokio::test]
async fn incremental_opposing_completed_fact_writers_settle_late_owner_without_fk_deadlock() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let early = native_member(
        &store,
        &seeded,
        &compiled,
        key,
        0,
        "opposing-writers",
        vec![],
    )
    .await;
    let late = native_member(
        &store,
        &seeded,
        &compiled,
        key,
        1,
        "opposing-writers",
        vec![],
    )
    .await;
    for index in 0..6 {
        let item_key = format!("opposing-{index}");
        append_provider_output_item(
            &store,
            late.id,
            json!({"type":"message","id":item_key,"content":"original later owner"}),
        )
        .await;
        let earliest = json!({"type":"message","id":item_key,"content":"earliest owner"});
        let another = json!({"type":"message","id":item_key,"content":"another later observation"});
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            tokio::join!(
                append_provider_output_item(&store, early.id, earliest.clone()),
                append_provider_output_item(&store, late.id, another)
            );
        })
        .await
        .expect("opposing writers must settle without a conversation/FK lock inversion");
        let source_key = format!("output:message:{item_key}");
        let kept = canonical(&store, early.id, &source_key).await.unwrap();
        assert_eq!(kept["_source_item"], earliest);
        assert_eq!(kept["_log_conflicting"], true);
        assert!(canonical(&store, late.id, &source_key).await.is_none());
    }
    // The weaker key lock still serializes terminal status transitions and
    // rejects subsequent completed facts through the original admission owner.
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: early.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({"answer":"done"}),
            error_payload: None,
            finished_at: Some(datetime!(2026-10-09 00:01:00 UTC)),
        })
        .await
        .unwrap();
    let before: i64 =
        sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1")
            .bind(early.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let input = AppendRuntimeEventInput {
        flow_run_id: early.id,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        event_type: "provider_output_item_done".into(),
        layer: domain::RuntimeEventLayer::AgentTransition,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
        payload: json!({"item":{"type":"message","id":"too-late","content":"forbidden"}}),
    };
    assert!(store.append_runtime_event(&input).await.is_err());
    let after: i64 = sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1")
        .bind(early.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(before, after, "terminal admission remains fenced");
}

#[tokio::test]
async fn incremental_allocator_and_internal_revision_updates_never_enter_body_trigger() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let run = native_member(
        &store,
        &seeded,
        &compiled,
        key,
        0,
        "allocator-filter",
        vec![json!({
            "type":"function_call_output","call_id":"large-context","output":"x".repeat(256*1024)
        })],
    )
    .await;
    // An unchanged revision alone would not detect the old implementation:
    // it entered the trigger and compared bodies even without changing them.
    // A controlled rejecting function observes invocation of the REAL trigger.
    sqlx::query("create or replace function revise_flow_message_projection() returns trigger language plpgsql as $$ begin raise exception 'fixture source-body trigger invoked'; end $$")
        .execute(store.pool()).await.unwrap();
    let before:(i64,i64)=sqlx::query_as("select runtime_event_sequence_high_water,message_projection_revision from flow_runs where id=$1")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    sqlx::query("update flow_runs set runtime_event_sequence_high_water=runtime_event_sequence_high_water+1 where id=$1")
        .bind(run.id).execute(store.pool()).await.expect("allocator-only updates cannot enter the source-body trigger");
    sqlx::query("update flow_runs set message_projection_revision=message_projection_revision+1 where id=$1")
        .bind(run.id).execute(store.pool()).await.expect("internal counter updates cannot recursively enter the source-body trigger");
    let after:(i64,i64)=sqlx::query_as("select runtime_event_sequence_high_water,message_projection_revision from flow_runs where id=$1")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(after, (before.0 + 1, before.1 + 1));
    let relevant = sqlx::query(
        "update flow_runs set output_payload=jsonb_build_object('answer','changed') where id=$1",
    )
    .bind(run.id)
    .execute(store.pool())
    .await
    .expect_err("real source updates must still invoke the trigger");
    assert!(relevant
        .to_string()
        .contains("fixture source-body trigger invoked"));
    let retained: Value = sqlx::query_scalar("select output_payload from flow_runs where id=$1")
        .bind(run.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(
        retained,
        json!({}),
        "the controlled failure rolls the source update back"
    );
}
