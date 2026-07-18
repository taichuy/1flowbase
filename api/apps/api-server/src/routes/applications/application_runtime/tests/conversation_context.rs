use super::*;

#[test]
fn flow_run_response_exposes_query_and_model_short_fields() {
    let run = domain::FlowRunRecord {
        id: Uuid::now_v7(),
        application_id: Uuid::now_v7(),
        flow_id: Uuid::now_v7(),
        draft_id: Uuid::now_v7(),
        compiled_plan_id: None,
        debug_session_id: "debug-session".to_string(),
        flow_schema_version: "1flowbase.flow/v2".to_string(),
        document_hash: "hash".to_string(),
        run_mode: domain::FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "say hello".to_string(),
        status: domain::FlowRunStatus::Succeeded,
        input_payload: serde_json::json!({
            "node-start": {
                "query": "say hello",
                "model": "deepseek-chat"
            }
        }),
        output_payload: serde_json::json!({ "answer": "hello" }),
        error_payload: None,
        created_by: Uuid::now_v7(),
        authorized_account: Some("root".to_string()),
        api_key_id: None,
        publication_version_id: None,
        external_user: Some("user-1".to_string()),
        external_conversation_id: Some("conversation-1".to_string()),
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: None,
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };

    let response = to_flow_run_response(run);

    assert_eq!(response.query.as_deref(), Some("say hello"));
    assert_eq!(response.model.as_deref(), Some("deepseek-chat"));
    assert_eq!(
        response.external_conversation_id.as_deref(),
        Some("conversation-1")
    );
}

#[tokio::test]
async fn run_conversation_without_external_conversation_id_reads_imported_history_and_current_turn()
{
    let run_id = Uuid::now_v7();
    let run = domain::FlowRunRecord {
        id: run_id,
        application_id: Uuid::now_v7(),
        flow_id: Uuid::now_v7(),
        draft_id: Uuid::now_v7(),
        compiled_plan_id: None,
        debug_session_id: "debug-session".to_string(),
        flow_schema_version: "1flowbase.flow/v2".to_string(),
        document_hash: "hash".to_string(),
        run_mode: domain::FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "current question".to_string(),
        status: domain::FlowRunStatus::Succeeded,
        input_payload: serde_json::json!({
            "node-start": {
                "query": "current question",
                "model": "deepseek-chat",
                "history": [
                    { "role": "system", "content": "hidden" },
                    { "role": "user", "content": "old question 1" },
                    { "role": "assistant", "content": "old answer 1" },
                    { "role": "tool", "content": "tool payload" },
                    { "role": "user", "content": "old question 2" },
                    { "role": "assistant", "content": "old answer 2" }
                ]
            }
        }),
        output_payload: serde_json::json!({ "answer": "current answer" }),
        error_payload: None,
        created_by: Uuid::now_v7(),
        authorized_account: Some("root".to_string()),
        api_key_id: None,
        publication_version_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: None,
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };

    let load_debug_artifact = |_| async { None::<serde_json::Value> };
    let page = conversation_messages_from_single_run(
        &run,
        &ApplicationConversationMessagesQuery {
            around_run_id: None,
            before: None,
            after: None,
            limit: Some(2),
        },
        &load_debug_artifact,
    )
    .await;

    assert_eq!(page.items.len(), 2);
    assert!(page.page.has_before);
    assert!(!page.page.has_after);
    let before_cursor = page
        .page
        .before_cursor
        .clone()
        .expect("initial page should expose earlier context cursor");
    assert_eq!(page.items[0].role.as_deref(), Some("assistant"));
    assert_eq!(page.items[0].content.as_deref(), Some("old answer 2"));
    assert_eq!(page.items[0].query, None);
    assert_eq!(page.items[0].answer, None);
    assert!(!page.items[0].can_open_detail);
    assert_eq!(page.items[0].detail_run_id, None);
    assert_eq!(page.items[1].run_id, run_id.to_string());
    assert_eq!(page.items[1].role, None);
    assert_eq!(page.items[1].content, None);
    assert_eq!(page.items[1].query.as_deref(), Some("current question"));
    assert_eq!(page.items[1].answer.as_deref(), Some("current answer"));
    assert!(page.items[1].can_open_detail);
    let run_id_string = run_id.to_string();
    assert_eq!(
        page.items[1].detail_run_id.as_deref(),
        Some(run_id_string.as_str())
    );

    let previous_page = conversation_messages_from_single_run(
        &run,
        &ApplicationConversationMessagesQuery {
            around_run_id: None,
            before: Some(before_cursor),
            after: None,
            limit: Some(5),
        },
        &load_debug_artifact,
    )
    .await;
    assert_eq!(previous_page.items.len(), 4);
    assert!(!previous_page.page.has_before);
    assert!(previous_page.page.has_after);
    assert_eq!(previous_page.items[0].role.as_deref(), Some("system"));
    assert_eq!(previous_page.items[0].content.as_deref(), Some("hidden"));
    assert_eq!(previous_page.items[1].role.as_deref(), Some("user"));
    assert_eq!(
        previous_page.items[1].content.as_deref(),
        Some("old question 1")
    );
}

#[test]
fn run_conversation_projection_page_preserves_existing_response_shape_and_cursors() {
    let run_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let page = conversation_messages_from_projection_page(
        run_id,
        control_plane::ports::ApplicationRunConversationMessageItemsPage {
            items: vec![
                domain::ApplicationRunConversationMessageItem {
                    id: Uuid::now_v7(),
                    scope_id: Uuid::now_v7(),
                    application_id: Uuid::now_v7(),
                    flow_run_id: run_id,
                    display_sequence: 4,
                    source_kind: "imported_context".to_string(),
                    role: Some("assistant".to_string()),
                    content: Some("old answer".to_string()),
                    query: None,
                    model: Some("gpt-test".to_string()),
                    answer: None,
                    detail_run_id: None,
                    can_open_detail: false,
                    is_current: false,
                    status: "succeeded".to_string(),
                    started_at: now,
                    finished_at: Some(now),
                    projection_version: 1,
                    created_at: now,
                    updated_at: now,
                },
                domain::ApplicationRunConversationMessageItem {
                    id: Uuid::now_v7(),
                    scope_id: Uuid::now_v7(),
                    application_id: Uuid::now_v7(),
                    flow_run_id: run_id,
                    display_sequence: 5,
                    source_kind: "current_run".to_string(),
                    role: None,
                    content: None,
                    query: Some("current question".to_string()),
                    model: Some("gpt-test".to_string()),
                    answer: Some("current answer".to_string()),
                    detail_run_id: Some(run_id),
                    can_open_detail: true,
                    is_current: true,
                    status: "succeeded".to_string(),
                    started_at: now,
                    finished_at: Some(now),
                    projection_version: 1,
                    created_at: now,
                    updated_at: now,
                },
            ],
            total_count: 6,
            has_before: true,
            has_after: false,
            before_cursor: Some(4),
            after_cursor: None,
        },
    );

    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].run_id, format!("{run_id}:context:4"));
    assert_eq!(page.items[0].role.as_deref(), Some("assistant"));
    assert_eq!(page.items[0].content.as_deref(), Some("old answer"));
    assert_eq!(page.items[0].detail_run_id, None);
    assert!(!page.items[0].can_open_detail);
    assert_eq!(page.items[1].run_id, run_id.to_string());
    assert_eq!(page.items[1].query.as_deref(), Some("current question"));
    assert_eq!(page.items[1].answer.as_deref(), Some("current answer"));
    assert_eq!(
        page.items[1].detail_run_id.as_deref(),
        Some(run_id.to_string().as_str())
    );
    assert!(page.items[1].is_current);
    assert!(page.page.has_before);
    assert!(!page.page.has_after);
    assert_eq!(
        page.page.before_cursor.as_deref(),
        Some(format!("{run_id}:context:4").as_str())
    );
    assert_eq!(page.page.after_cursor, None);
}

#[tokio::test]
async fn run_conversation_preserves_marker_like_history_regardless_of_legacy_mode() {
    let run_id = Uuid::now_v7();
    let run = domain::FlowRunRecord {
        id: run_id,
        application_id: Uuid::now_v7(),
        flow_id: Uuid::now_v7(),
        draft_id: Uuid::now_v7(),
        compiled_plan_id: None,
        debug_session_id: "debug-session".to_string(),
        flow_schema_version: "1flowbase.flow/v2".to_string(),
        document_hash: "hash".to_string(),
        run_mode: domain::FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "current question".to_string(),
        status: domain::FlowRunStatus::Succeeded,
        input_payload: serde_json::json!({
            "node-start": {
                "query": "那你帮我拉一下最新代码",
                "history": [
                    {
                        "role": "user",
                        "content": "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\nSummary: hi\n\nIf you need specific details from before compaction (like exact code snippets, error messages, or content you generated), read the full transcript at: C:\\Users\\Lw\\.claude\\projects\\repo\\session.jsonl"
                    },
                    {
                        "role": "assistant",
                        "content": "已恢复上下文。"
                    },
                    { "role": "user", "content": "visible old question" },
                    { "role": "assistant", "content": "visible old answer" }
                ]
            }
        }),
        output_payload: serde_json::json!({ "answer": "current answer" }),
        error_payload: None,
        created_by: Uuid::now_v7(),
        authorized_account: Some("root".to_string()),
        api_key_id: None,
        publication_version_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: Some("anthropic-messages-v1".to_string()),
        idempotency_key: None,
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };

    let load_debug_artifact = |_| async { None::<serde_json::Value> };
    let page = conversation_messages_from_single_run(
        &run,
        &ApplicationConversationMessagesQuery {
            around_run_id: None,
            before: None,
            after: None,
            limit: Some(5),
        },
        &load_debug_artifact,
    )
    .await;

    let visible_text = page
        .items
        .iter()
        .flat_map(|item| {
            [
                item.content.clone(),
                item.query.clone(),
                item.answer.clone(),
            ]
            .into_iter()
            .flatten()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(visible_text.contains("This session is being continued"));
    assert!(visible_text.contains("已恢复上下文"));
    assert!(visible_text.contains("visible old question"));
    assert!(visible_text.contains("visible old answer"));
    assert!(visible_text.contains("那你帮我拉一下最新代码"));
    assert!(visible_text.contains("current answer"));
}

#[tokio::test]
async fn run_conversation_reads_llm_system_when_run_input_system_is_split_from_provider_messages() {
    let run_id = Uuid::now_v7();
    let application_id = Uuid::now_v7();
    let flow_run = domain::FlowRunRecord {
        id: run_id,
        application_id,
        flow_id: Uuid::now_v7(),
        draft_id: Uuid::now_v7(),
        compiled_plan_id: None,
        debug_session_id: "debug-session".to_string(),
        flow_schema_version: "1flowbase.flow/v2".to_string(),
        document_hash: "hash".to_string(),
        run_mode: domain::FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "hi ?".to_string(),
        status: domain::FlowRunStatus::Succeeded,
        input_payload: serde_json::json!({
            "node-start": {
                "query": "hi ?",
                "model": "1flowbase"
            }
        }),
        output_payload: serde_json::json!({ "answer": "Hello!" }),
        error_payload: None,
        created_by: Uuid::now_v7(),
        authorized_account: Some("root".to_string()),
        api_key_id: None,
        publication_version_id: None,
        external_user: None,
        external_conversation_id: Some("conversation-1".to_string()),
        external_trace_id: None,
        compatibility_mode: Some("anthropic-compatible".to_string()),
        idempotency_key: None,
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let detail = domain::ApplicationRunDetail {
        flow_run,
        node_runs: vec![domain::NodeRunRecord {
            id: Uuid::now_v7(),
            flow_run_id: run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "LLM".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: serde_json::json!({
                "prompt_messages": [
                    {
                        "id": "user-1",
                        "role": "user",
                        "content": "hi ?"
                    }
                ]
            }),
            output_payload: serde_json::json!({ "answer": "Hello!" }),
            error_payload: None,
            metrics_payload: serde_json::json!({}),
            debug_payload: serde_json::json!({
                "llm_context": {
                    "effective_system": "Use the image-aware system policy.",
                    "provider_messages": [
                        { "role": "user", "content": "hi ?" }
                    ]
                }
            }),
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        }],
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };

    let load_debug_artifact = |_| async { None::<serde_json::Value> };
    let page = conversation_messages_from_run_detail(
        &detail,
        &ApplicationConversationMessagesQuery {
            around_run_id: None,
            before: None,
            after: None,
            limit: Some(5),
        },
        &load_debug_artifact,
    )
    .await;

    assert_eq!(page.items.len(), 2);
    assert!(!page.page.has_before);
    assert!(!page.page.has_after);
    assert_eq!(page.items[0].role.as_deref(), Some("system"));
    assert_eq!(
        page.items[0].content.as_deref(),
        Some("Use the image-aware system policy.")
    );
    assert!(!page.items[0].can_open_detail);
    assert_eq!(page.items[1].run_id, run_id.to_string());
    assert_eq!(page.items[1].query.as_deref(), Some("hi ?"));
    assert_eq!(page.items[1].answer.as_deref(), Some("Hello!"));
}

#[tokio::test]
async fn llm_prompt_messages_system_content_reads_system_prompt_message() {
    let payload = serde_json::json!({
        "prompt_messages": [
            {
                "id": "system-1",
                "role": "system",
                "content": "Use the node policy."
            },
            {
                "id": "user-1",
                "role": "user",
                "content": "hi ?"
            }
        ]
    });
    let load_debug_artifact = |_| async { None::<serde_json::Value> };

    assert_eq!(
        llm_prompt_messages_system_content(&payload, &load_debug_artifact)
            .await
            .as_deref(),
        Some("Use the node policy.")
    );
}

#[tokio::test]
async fn run_conversation_without_external_conversation_id_reads_artifact_backed_imported_history()
{
    let run_id = Uuid::now_v7();
    let artifact_id = Uuid::now_v7();
    let run = domain::FlowRunRecord {
        id: run_id,
        application_id: Uuid::now_v7(),
        flow_id: Uuid::now_v7(),
        draft_id: Uuid::now_v7(),
        compiled_plan_id: None,
        debug_session_id: "debug-session".to_string(),
        flow_schema_version: "1flowbase.flow/v2".to_string(),
        document_hash: "hash".to_string(),
        run_mode: domain::FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "current question".to_string(),
        status: domain::FlowRunStatus::Succeeded,
        input_payload: serde_json::json!({
            "__runtime_debug_artifact": true,
            "artifact_ref": artifact_id.to_string(),
            "is_truncated": true,
            "query": "current question",
            "model": "deepseek-chat",
            "preview": "{\"node-start\":{\"query\":\"current question\""
        }),
        output_payload: serde_json::json!({ "answer": "current answer" }),
        error_payload: None,
        created_by: Uuid::now_v7(),
        authorized_account: Some("root".to_string()),
        api_key_id: None,
        publication_version_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: None,
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };

    let load_debug_artifact = move |requested_artifact_id: Uuid| async move {
        (requested_artifact_id == artifact_id).then(|| {
            serde_json::json!({
                "node-start": {
                    "query": "current question",
                    "model": "deepseek-chat",
                    "history": [
                        { "role": "system", "content": "hidden" },
                        { "role": "user", "content": "old question" },
                        { "role": "assistant", "content": "old answer" }
                    ]
                }
            })
        })
    };
    let page = conversation_messages_from_single_run(
        &run,
        &ApplicationConversationMessagesQuery {
            around_run_id: None,
            before: None,
            after: None,
            limit: Some(5),
        },
        &load_debug_artifact,
    )
    .await;

    assert_eq!(page.items.len(), 4);
    assert_eq!(page.items[0].role.as_deref(), Some("system"));
    assert_eq!(page.items[0].content.as_deref(), Some("hidden"));
    assert!(!page.items[0].can_open_detail);
    assert_eq!(page.items[1].role.as_deref(), Some("user"));
    assert_eq!(page.items[1].content.as_deref(), Some("old question"));
    assert_eq!(page.items[2].role.as_deref(), Some("assistant"));
    assert_eq!(page.items[2].content.as_deref(), Some("old answer"));
    assert_eq!(page.items[3].run_id, run_id.to_string());
    assert_eq!(page.items[3].query.as_deref(), Some("current question"));
    assert!(page.items[3].can_open_detail);
}

#[tokio::test]
async fn run_conversation_hydrates_artifact_backed_current_answer() {
    let run_id = Uuid::now_v7();
    let artifact_id = Uuid::now_v7();
    let full_answer = "full final answer from artifact";
    let run = domain::FlowRunRecord {
        id: run_id,
        application_id: Uuid::now_v7(),
        flow_id: Uuid::now_v7(),
        draft_id: Uuid::now_v7(),
        compiled_plan_id: None,
        debug_session_id: "debug-session".to_string(),
        flow_schema_version: "1flowbase.flow/v2".to_string(),
        document_hash: "hash".to_string(),
        run_mode: domain::FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "current question".to_string(),
        status: domain::FlowRunStatus::Succeeded,
        input_payload: serde_json::json!({
            "node-start": {
                "query": "current question",
                "model": "deepseek-chat"
            }
        }),
        output_payload: serde_json::json!({
            "answer": {
                "__runtime_debug_artifact": true,
                "artifact_ref": artifact_id.to_string(),
                "field_path": ["answer"],
                "preview": "preview final answer"
            }
        }),
        error_payload: None,
        created_by: Uuid::now_v7(),
        authorized_account: Some("root".to_string()),
        api_key_id: None,
        publication_version_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: None,
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };

    let load_debug_artifact = move |requested_artifact_id: Uuid| async move {
        (requested_artifact_id == artifact_id)
            .then(|| serde_json::Value::String(full_answer.to_string()))
    };
    let page = conversation_messages_from_single_run(
        &run,
        &ApplicationConversationMessagesQuery {
            around_run_id: None,
            before: None,
            after: None,
            limit: Some(5),
        },
        &load_debug_artifact,
    )
    .await;

    let current = page.items.last().expect("current run message exists");
    assert_eq!(current.answer.as_deref(), Some(full_answer));
}
