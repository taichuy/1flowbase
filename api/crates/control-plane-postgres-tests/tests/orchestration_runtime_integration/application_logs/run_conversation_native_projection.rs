use super::*;
use control_plane_contracts::application_public_runtime::ApplicationPublishedFlowRunRepository;
use control_plane_contracts::ports::ApplicationRunLogContext;

// #2090: the native (client protocol) projection path. A native call keeps its
// request facts in `log_context`, so these tests exercise the projection that
// serves a running call rather than the retained-payload path.

#[tokio::test]
async fn native_running_run_projects_prompt_context_and_completed_output_items() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-26 09:00:00 UTC);
    let run = seed_native_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "native-running",
        started_at,
        json!({
            "__native_model_prompt_context": {
                "system": [{ "text": "Codex desktop context." }],
                "messages": []
            },
            "node-start": {
                "system": "Codex desktop context.",
                "query": "old question",
                "model": "gpt-native"
            }
        }),
        Some(json!({
            "role": "user",
            "type": "message",
            "content": [{ "text": "current question" }]
        })),
    )
    .await;
    append_provider_output_item(
        &store,
        run.id,
        json!({
            "type": "custom_tool_call",
            "call_id": "call-1",
            "name": "exec",
            "input": "ls"
        }),
    )
    .await;
    append_provider_output_item(
        &store,
        run.id,
        json!({
            "type": "message",
            "id": "message-1",
            "role": "assistant",
            "phase": "final_answer",
            "content": [{ "type": "output_text", "text": "done" }]
        }),
    )
    .await;

    // #2090 AC-001/AC-005: a running call already exposes its retained input,
    // its completed output items and the context in force.
    let page = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(page.total_count, 3);
    assert_eq!(
        page.items
            .iter()
            .map(|item| (
                item.role.as_deref(),
                item.content.as_deref(),
                item.output_source.as_deref(),
                item.status.as_str(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (Some("user"), Some("current question"), None, "running"),
            (
                Some("assistant"),
                Some("exec"),
                Some("provider_output_item"),
                "running"
            ),
            (
                Some("assistant"),
                Some("done"),
                Some("provider_output_item"),
                "running"
            ),
        ]
    );
    assert_eq!(
        page.contexts
            .iter()
            .map(|context| (
                context.role.as_str(),
                context.context_source.as_str(),
                context.content.as_str(),
            ))
            .collect::<Vec<_>>(),
        vec![("system", "client_request", "Codex desktop context.")],
        "identical client and application context is reported once, from its earliest source"
    );
    let output_state = page.output_state.as_ref().unwrap();
    assert_eq!(output_state.status, "running");
    assert_eq!(output_state.output_source, "provider_output_item");
    assert_eq!(output_state.output_item_count, 2);
    assert_eq!(page.newest_sequence, Some(2));

    // #2090 AC-001: the task row carries the same observed prompt while running,
    // without waiting for the run projection to be rebuilt.
    let task = store
        .get_application_run_log_task(seeded.application_id, run.id)
        .await
        .unwrap()
        .expect("task row");
    assert_eq!(task.user_input.as_deref(), Some("current question"));
    assert_eq!(task.outcome, "in_progress");
}

#[tokio::test]
async fn native_projection_distinguishes_client_application_and_effective_context() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-26 09:30:00 UTC);
    let run = seed_native_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "native-context-sources",
        started_at,
        json!({
            "__native_model_prompt_context": {
                "system": [{ "text": "client system" }],
                "messages": [
                    {
                        "role": "developer",
                        "content": [{ "type": "input_text", "text": "developer instructions" }]
                    }
                ]
            },
            "node-start": { "system": "application system", "query": "q" }
        }),
        Some(json!({ "role": "user", "content": "q" })),
    )
    .await;
    let node_run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_node_run(
        &store,
        &CreateNodeRunInput {
            flow_run_id: run.id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "LLM".to_string(),
            status: NodeRunStatus::Running,
            input_payload: json!({
                "prompt_messages": [{ "role": "system", "content": "effective system" }]
            }),
            debug_payload: json!({}),
            started_at,
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::complete_node_run(
        &store,
        &CompleteNodeRunInput {
            node_run_id: node_run.id,
            status: NodeRunStatus::Succeeded,
            output_payload: json!({ "answer": "current answer" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({
                "llm_context": { "effective_system": "effective system" }
            }),
            finished_at: started_at + Duration::seconds(1),
        },
    )
    .await
    .unwrap();

    let page = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(
        page.contexts
            .iter()
            .map(|context| (
                context.role.as_str(),
                context.context_source.as_str(),
                context.content.as_str(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("system", "client_request", "client system"),
            ("developer", "client_request", "developer instructions"),
            ("system", "application_config", "application system"),
            ("system", "effective_prompt", "effective system"),
        ]
    );
    assert!(
        page.items.iter().all(|item| item.role.as_deref() != Some("system")
            && item.role.as_deref() != Some("developer")),
        "context never becomes part of the paged conversation stream"
    );
}

#[tokio::test]
async fn native_persisted_answer_is_projected_with_its_source_and_replaced_by_a_formal_item() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-26 10:00:00 UTC);
    let run = seed_native_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "native-persisted-answer",
        started_at,
        json!({
            "__native_model_prompt_context": { "system": [], "messages": [] },
            "node-start": { "system": "title system", "query": "title question" }
        }),
        Some(json!({ "role": "user", "content": "title question" })),
    )
    .await;
    // The title-call shape: a task anchor whose answer is persisted on the run
    // without any provider output item.
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run_payloads(
        &store,
        &UpdateFlowRunPayloadsInput {
            flow_run_id: run.id,
            input_payload: run.input_payload.clone(),
            output_payload: json!({
                "answer": "{\"title\":\"Review quality gates\"}",
                "answer_segments": [{ "kind": "message", "text": "review" }]
            }),
            error_payload: None,
        },
    )
    .await
    .unwrap();

    // #2090 AC-002: the saved answer is shown and its source is stated, while
    // the call still runs.
    let page = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(page.total_count, 2);
    assert_eq!(
        page.items
            .iter()
            .map(|item| (
                item.role.as_deref(),
                item.answer.as_deref(),
                item.content.as_deref(),
                item.output_source.as_deref(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (Some("user"), None, Some("title question"), None),
            (
                Some("assistant"),
                None,
                Some("{\"title\":\"Review quality gates\"}"),
                Some("persisted_answer")
            ),
        ]
    );
    assert_eq!(
        page.output_state.as_ref().unwrap().output_source,
        "persisted_answer"
    );
    let running_task = store
        .get_application_run_log_task(seeded.application_id, run.id)
        .await
        .unwrap()
        .expect("task row");
    assert_eq!(
        running_task.final_output.as_deref(),
        Some("{\"title\":\"Review quality gates\"}"),
        "an observed answer stays on the task row"
    );
    assert_eq!(
        running_task.outcome, "in_progress",
        "an observed answer does not claim completion while the call runs"
    );

    // #2090 AC-002: a formal output item that arrives later owns the answer, and
    // the persisted fallback is not counted twice.
    append_provider_output_item(
        &store,
        run.id,
        json!({
            "type": "message",
            "id": "message-formal",
            "role": "assistant",
            "phase": "final_answer",
            "content": [{ "type": "output_text", "text": "formal answer" }]
        }),
    )
    .await;
    let replaced = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(replaced.total_count, 2);
    assert_eq!(
        replaced
            .items
            .iter()
            .map(|item| (item.content.as_deref(), item.output_source.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (Some("title question"), None),
            (Some("formal answer"), Some("provider_output_item")),
        ],
        "a later formal item replaces the persisted fallback instead of duplicating it"
    );

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "formal answer" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(2)),
        },
    )
    .await
    .unwrap();
    let finished_task = store
        .get_application_run_log_task(seeded.application_id, run.id)
        .await
        .unwrap()
        .expect("task row");
    assert_eq!(finished_task.outcome, "final_answer_observed");
    assert_eq!(finished_task.final_output.as_deref(), Some("formal answer"));
}

#[tokio::test]
async fn native_tool_call_items_do_not_hide_a_persisted_answer() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-26 10:15:00 UTC);
    let run = seed_native_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "native-tool-call-with-answer",
        started_at,
        json!({
            "__native_model_prompt_context": { "system": [], "messages": [] },
            "node-start": { "query": "q" }
        }),
        Some(json!({ "role": "user", "content": "run it" })),
    )
    .await;
    // A tool-call round is a formal output item, but it is not the answer.
    append_provider_output_item(
        &store,
        run.id,
        json!({
            "type": "custom_tool_call",
            "call_id": "call-1",
            "name": "exec",
            "input": "date"
        }),
    )
    .await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "the final answer" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(4)),
        },
    )
    .await
    .unwrap();

    let page = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(
        page.items
            .iter()
            .map(|item| (item.content.as_deref(), item.output_source.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (Some("run it"), None),
            (Some("exec"), Some("provider_output_item")),
            (Some("the final answer"), Some("persisted_answer")),
        ],
        "a tool-call round must not hide the answer the call stored"
    );
    assert_eq!(
        page.output_state.as_ref().unwrap().output_source,
        "persisted_answer",
        "the state names where the visible answer came from"
    );
}

#[tokio::test]
async fn native_projection_refreshes_when_late_facts_arrive() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-26 10:30:00 UTC);
    let run = seed_native_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "native-late-facts",
        started_at,
        json!({
            "__native_model_prompt_context": { "system": [], "messages": [] },
            "node-start": { "query": "q" }
        }),
        None,
    )
    .await;

    let first = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(first.total_count, 1);
    assert_eq!(first.items[0].role, None);
    assert_eq!(first.items[0].output_source.as_deref(), Some("none"));

    // The client prompt and a completed output item arrive after the first read.
    sqlx::query(
        "update flow_runs set log_context = log_context || jsonb_build_object('prompt', $2::jsonb) where id = $1",
    )
    .bind(run.id)
    .bind(json!({ "role": "user", "content": "late question" }).to_string())
    .execute(store.pool())
    .await
    .unwrap();
    append_provider_output_item(
        &store,
        run.id,
        json!({
            "type": "message",
            "id": "message-late",
            "role": "assistant",
            "phase": "final_answer",
            "content": [{ "type": "output_text", "text": "late answer" }]
        }),
    )
    .await;

    let refreshed = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(
        refreshed
            .items
            .iter()
            .map(|item| (
                item.role.as_deref(),
                item.content.as_deref(),
                item.output_source.as_deref(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (Some("user"), Some("late question"), None),
            (
                Some("assistant"),
                Some("late answer"),
                Some("provider_output_item")
            ),
        ]
    );
}

#[tokio::test]
async fn run_conversation_keeps_message_ids_and_reports_updated_existing_items() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-26 11:00:00 UTC);
    let run = seed_native_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "native-updated-item",
        started_at,
        json!({
            "__native_model_prompt_context": { "system": [], "messages": [] },
            "node-start": { "query": "q" }
        }),
        Some(json!({ "role": "user", "content": "current question" })),
    )
    .await;

    let running = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    let prompt_id = running.items[0].id;
    assert_eq!(running.items[0].status, "running");

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "final answer" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();

    // #2090 AC-004: an updated item keeps its identity so a client can replace
    // the stale copy instead of showing both.
    let finished = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(finished.items[0].id, prompt_id);
    assert_eq!(finished.items[0].status, "succeeded");
    assert_eq!(
        finished
            .items
            .iter()
            .map(|item| (item.content.as_deref(), item.output_source.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (Some("current question"), None),
            (Some("final answer"), Some("persisted_answer")),
        ]
    );
}

#[tokio::test]
async fn run_conversation_after_cursor_drains_a_backlog_larger_than_one_page() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-26 11:30:00 UTC);
    let run = seed_native_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "native-cursor-backlog",
        started_at,
        json!({
            "__native_model_prompt_context": { "system": [], "messages": [] },
            "node-start": { "query": "q" }
        }),
        Some(json!({ "role": "user", "content": "current question" })),
    )
    .await;
    for index in 0..6 {
        append_provider_output_item(
            &store,
            run.id,
            json!({
                "type": "message",
                "id": format!("message-{index}"),
                "role": "assistant",
                "content": [{ "type": "output_text", "text": format!("answer-{index}") }]
            }),
        )
        .await;
    }

    let first = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(first.total_count, 7);
    assert!(first.has_before);
    assert!(!first.has_after);
    assert_eq!(first.items.len(), 5);
    let watermark = first.newest_sequence.expect("newest sequence");

    for index in 6..13 {
        append_provider_output_item(
            &store,
            run.id,
            json!({
                "type": "message",
                "id": format!("message-{index}"),
                "role": "assistant",
                "content": [{ "type": "output_text", "text": format!("answer-{index}") }]
            }),
        )
        .await;
    }

    // Reading forward from the newest known position drains the backlog in
    // pages, without a gap and without repeating an item.
    let mut collected = Vec::new();
    let mut after = Some(watermark);
    let mut guard = 0;
    while let Some(cursor) = after {
        let page =
            run_conversation_page(&store, seeded.application_id, run.id, None, Some(cursor), 5)
                .await;
        collected.extend(
            page.items
                .iter()
                .map(|item| (item.display_sequence, item.content.clone().unwrap())),
        );
        after = page.has_after.then(|| page.after_cursor).flatten();
        guard += 1;
        assert!(guard < 20, "catch-up must terminate");
    }
    assert_eq!(
        collected
            .iter()
            .map(|(_, content)| content.as_str())
            .collect::<Vec<_>>(),
        (6..13).map(|index| format!("answer-{index}")).collect::<Vec<_>>()
    );
    let sequences = collected
        .iter()
        .map(|(sequence, _)| *sequence)
        .collect::<Vec<_>>();
    let mut sorted = sequences.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sequences, sorted, "catch-up yields ordered, unique items");
}

/// A native (client protocol) run keeps its request facts in `log_context`, so
/// it exercises the native projection rather than the retained-payload path.
async fn seed_native_run_conversation_flow_run(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    debug_session_id: &str,
    started_at: OffsetDateTime,
    input_payload: serde_json::Value,
    prompt: Option<serde_json::Value>,
) -> domain::FlowRunRecord {
    let api_key_id = seed_application_api_key(store, seeded).await;
    ApplicationPublishedFlowRunRepository::create_published_flow_run(
        store,
        &CreateFlowRunInput {
            application_run_log_context: Some(ApplicationRunLogContext {
                identity_status: "identified".into(),
                protocol: Some("openai_responses".into()),
                thread_id: Some(format!("thread-{debug_session_id}")),
                turn_id: Some(format!("turn-{debug_session_id}")),
                prompt,
                ..Default::default()
            }),
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: debug_session_id.to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "native run conversation".into(),
            status: FlowRunStatus::Running,
            input_payload,
            started_at,
            api_key_id: Some(api_key_id),
            publication_version_id: Some(Uuid::now_v7()),
            assistant_conversation_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: Some("openai-responses".into()),
            idempotency_key: Some(format!("native-{debug_session_id}")),
        },
    )
    .await
    .unwrap()
    .flow_run
}

async fn append_provider_output_item(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
    item: serde_json::Value,
) {
    store
        .append_runtime_event(&AppendRuntimeEventInput {
            flow_run_id,
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
            payload: json!({ "item": item }),
        })
        .await
        .unwrap();
}

async fn run_conversation_page(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    run_id: Uuid,
    before_sequence: Option<i64>,
    after_sequence: Option<i64>,
    limit: i64,
) -> control_plane::ports::ApplicationRunConversationMessageItemsPage {
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_conversation_message_items_page(
        store,
        application_id,
        run_id,
        ListApplicationRunConversationMessageItemsPageInput {
            before_sequence,
            after_sequence,
            limit,
        },
    )
    .await
    .unwrap()
}
