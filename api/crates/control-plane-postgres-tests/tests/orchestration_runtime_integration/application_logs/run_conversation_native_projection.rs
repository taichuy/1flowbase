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

    // Running tasks expose the current input and context; completed provider
    // facts remain available in output state without becoming extra turns.
    let page = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(page.total_count, 1);
    assert_eq!(page.items[0].query.as_deref(), Some("current question"));
    assert_eq!(page.items[0].model.as_deref(), Some("gpt-native"));
    assert_eq!(page.items[0].status, "running");
    assert!(
        page.items[0].answer.is_none(),
        "a running task does not claim a final answer"
    );
    assert_eq!(page.items[0].detail_run_id, Some(run.id));
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
    assert_eq!(page.newest_sequence, Some(0));

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

    let persisted_effective: Option<String> = sqlx::query_scalar(
        "select content from application_run_conversation_message_items where flow_run_id=$1 and context_source='effective_prompt'")
        .bind(run.id).fetch_optional(store.pool()).await.unwrap();
    assert_eq!(
        persisted_effective.as_deref(),
        Some("effective system"),
        "node completion writes the effective context before any GET"
    );

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
        page.items
            .iter()
            .all(|item| item.role.as_deref() != Some("system")
                && item.role.as_deref() != Some("developer")),
        "context never becomes part of the paged conversation stream"
    );
}

#[tokio::test]
async fn native_node_updates_write_effective_context_without_get_repair() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-26 09:45:00 UTC);
    let run = seed_native_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "node-context-writes",
        started_at,
        json!({"node-start": {"query": "q"}}),
        Some(json!({"role": "user", "content": "q"})),
    )
    .await;
    let node = store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run.id,
            node_id: "node-llm".into(),
            node_type: "llm".into(),
            node_alias: "LLM".into(),
            status: NodeRunStatus::Running,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at,
        })
        .await
        .unwrap();
    // Identical explicit timestamps must not hide changed node debug facts.
    for effective in ["first effective system", "second effective system"] {
        store
            .update_node_run(&UpdateNodeRunInput {
                node_run_id: node.id,
                status: NodeRunStatus::Running,
                output_payload: json!({}),
                error_payload: None,
                metrics_payload: json!({}),
                debug_payload: json!({"llm_context": {"effective_system": effective}}),
                finished_at: Some(started_at),
            })
            .await
            .unwrap();
        let persisted: String = sqlx::query_scalar("select content from application_run_conversation_message_items where flow_run_id=$1 and context_source='effective_prompt'")
            .bind(run.id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(persisted, effective);
    }
    store.complete_node_run(&CompleteNodeRunInput {
        node_run_id: node.id, status: NodeRunStatus::Succeeded,
        output_payload: json!({}), error_payload: None, metrics_payload: json!({}),
        debug_payload: json!({"llm_context": {"effective_system": "completed effective system"}}),
        finished_at: started_at,
    }).await.unwrap();
    let persisted: String = sqlx::query_scalar("select content from application_run_conversation_message_items where flow_run_id=$1 and context_source='effective_prompt'")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(persisted, "completed effective system");
    let page = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert!(page
        .contexts
        .iter()
        .any(|context| context.context_source == "effective_prompt"
            && context.content == "completed effective system"));
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

    // An in-flight payload stays in the original record, not the visible answer projection.
    let page = run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(page.total_count, 1);
    assert_eq!(page.items[0].query.as_deref(), Some("title question"));
    assert!(
        page.items[0].answer.is_none(),
        "a persisted in-flight payload is not a completed business answer"
    );
    let retained: serde_json::Value =
        sqlx::query_scalar("select output_payload from flow_runs where id=$1")
            .bind(run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(retained["answer"], "{\"title\":\"Review quality gates\"}");
    assert_eq!(
        page.output_state.as_ref().unwrap().output_source,
        "none",
        "a retained in-flight payload is not the source of a visible final answer"
    );
    let running_task = store
        .get_application_run_log_task(seeded.application_id, run.id)
        .await
        .unwrap()
        .expect("task row");
    assert!(
        running_task.final_output.is_none(),
        "persisted output requires successful completion"
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
    let replaced =
        run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(replaced.total_count, 1);
    assert_eq!(replaced.items[0].id, page.items[0].id);
    assert!(replaced.items[0].answer.is_none());
    assert_eq!(replaced.output_state.as_ref().unwrap().output_item_count, 1);

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
    let final_page =
        run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(final_page.items[0].id, page.items[0].id);
    assert_eq!(final_page.items[0].answer.as_deref(), Some("formal answer"));
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
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].query.as_deref(), Some("run it"));
    assert_eq!(page.items[0].answer.as_deref(), Some("the final answer"));
    let tool_name: String = sqlx::query_scalar("select payload#>>'{item,name}' from runtime_events where flow_run_id=$1 and event_type='provider_output_item_done'")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        tool_name, "exec",
        "tool execution remains retained outside chat"
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

    let refreshed =
        run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(refreshed.items.len(), 1);
    assert_eq!(refreshed.items[0].id, first.items[0].id);
    assert_eq!(refreshed.items[0].query.as_deref(), Some("late question"));
    assert!(refreshed.items[0].answer.is_none());
    let task = store
        .get_application_run_log_task(seeded.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task.final_output.as_deref(), Some("late answer"));
    assert_eq!(task.outcome, "in_progress");
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({"answer":"late answer"}),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(2)),
        })
        .await
        .unwrap();
    let complete =
        run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(complete.items[0].id, first.items[0].id);
    assert_eq!(complete.items[0].answer.as_deref(), Some("late answer"));
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
    let finished =
        run_conversation_page(&store, seeded.application_id, run.id, None, None, 5).await;
    assert_eq!(finished.items[0].id, prompt_id);
    assert_eq!(finished.items[0].status, "succeeded");
    assert_eq!(finished.items.len(), 1);
    assert_eq!(finished.items[0].query.as_deref(), Some("current question"));
    assert_eq!(finished.items[0].answer.as_deref(), Some("final answer"));
    assert_eq!(
        finished.items[0].output_source.as_deref(),
        Some("persisted_answer")
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
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let mut ids = Vec::new();
    let mut conversation_id = None;
    let mut watermark = None;
    for index in 0..13 {
        let debug_session_id = format!("native-backlog-{index}");
        let run = ApplicationPublishedFlowRunRepository::create_published_flow_run(
            &store,
            &CreateFlowRunInput {
                application_run_log_context: Some(ApplicationRunLogContext {
                    identity_status: "identified".into(),
                    protocol: Some("openai_responses".into()),
                    thread_id: Some("thread-native-cursor-backlog".into()),
                    turn_id: Some(format!("turn-{debug_session_id}")),
                    prompt: Some(json!({"role": "user", "content": format!("question-{index}")})),
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
                input_payload: json!({"node-start": {"query": format!("question-{index}")}}),
                started_at: started_at + Duration::seconds(index * 2),
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
        .flow_run;
        append_provider_output_item(&store, run.id, json!({
            "type": "custom_tool_call", "call_id": format!("call-{index}"), "name": "exec", "input": "ls"
        })).await;
        store
            .update_flow_run(&UpdateFlowRunInput {
                flow_run_id: run.id,
                status: FlowRunStatus::Succeeded,
                output_payload: json!({"answer": format!("answer-{index}")}),
                error_payload: None,
                finished_at: Some(started_at + Duration::seconds(index * 2 + 1)),
            })
            .await
            .unwrap();
        ids.push(run.id);
        if index == 0 {
            conversation_id = store
                .get_application_run_log_task(seeded.application_id, run.id)
                .await
                .unwrap()
                .unwrap()
                .log_conversation_id;
        }
        if index == 5 {
            let first = store
                .list_application_conversation_runs_page(
                    seeded.application_id,
                    ListApplicationConversationRunsPageInput {
                        external_conversation_id: conversation_id.unwrap().to_string(),
                        around_run_id: None,
                        before_run_id: None,
                        after_run_id: None,
                        limit: 5,
                    },
                )
                .await
                .unwrap();
            assert_eq!(
                first.items.iter().map(|item| item.id).collect::<Vec<_>>(),
                ids[1..]
            );
            assert!(first.has_before);
            assert!(!first.has_after);
            watermark = first.after_cursor;
        }
    }
    let mut collected = Vec::new();
    let mut after = Some(watermark.expect("latest business turn cursor"));
    let mut pages = 0;
    while let Some(cursor) = after {
        let page = store
            .list_application_conversation_runs_page(
                seeded.application_id,
                ListApplicationConversationRunsPageInput {
                    external_conversation_id: conversation_id.unwrap().to_string(),
                    around_run_id: None,
                    before_run_id: None,
                    after_run_id: Some(cursor),
                    limit: 5,
                },
            )
            .await
            .unwrap();
        for item in &page.items {
            let index = ids.iter().position(|id| *id == item.id).unwrap();
            assert_eq!(
                item.query.as_deref(),
                Some(format!("question-{index}").as_str())
            );
            assert_eq!(
                item.answer.as_deref(),
                Some(format!("answer-{index}").as_str())
            );
        }
        collected.extend(page.items.iter().map(|item| item.id));
        after = page.has_after.then_some(page.after_cursor).flatten();
        pages += 1;
        assert!(pages <= 2, "catch-up must terminate after two pages");
    }
    assert_eq!(pages, 2);
    assert_eq!(
        collected,
        ids[6..],
        "catch-up yields every new task exactly once, in order"
    );
    let wrong_scope = store
        .list_application_conversation_runs_page(
            Uuid::now_v7(),
            ListApplicationConversationRunsPageInput {
                external_conversation_id: conversation_id.unwrap().to_string(),
                around_run_id: None,
                before_run_id: None,
                after_run_id: watermark,
                limit: 5,
            },
        )
        .await
        .unwrap();
    assert!(wrong_scope.items.is_empty());
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
