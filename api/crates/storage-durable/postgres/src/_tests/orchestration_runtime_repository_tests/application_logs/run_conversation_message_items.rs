use super::*;

#[tokio::test]
async fn migration_creates_run_conversation_message_item_projection_table_and_indexes() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let table_name: Option<String> = sqlx::query_scalar(
        "select to_regclass('application_run_conversation_message_items')::text",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        table_name.as_deref(),
        Some("application_run_conversation_message_items")
    );

    let indexes = sqlx::query_scalar::<_, String>(
        r#"
        select indexname
        from pg_indexes
        where schemaname = current_schema()
          and tablename = 'application_run_conversation_message_items'
        order by indexname asc
        "#,
    )
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert!(
        indexes
            .iter()
            .any(|name| name == "application_run_conversation_message_items_run_sequence_idx"),
        "projection reads need application_id + flow_run_id + display_sequence index"
    );
    assert!(
        indexes.iter().any(|name| {
            name == "application_run_conversation_message_items_scope_created_id_idx"
        }),
        "managed table expansion reads need scope_id + created_at + id index"
    );
}

#[tokio::test]
async fn terminal_run_writes_conversation_message_items_and_pages_by_display_sequence() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-25 09:00:00 UTC);
    let run = seed_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "run-conversation-items-page",
        started_at,
        json!({
            "node-start": {
                "system": "Use concise Chinese.",
                "query": "current question",
                "model": "deepseek-chat",
                "history": [
                    { "role": "user", "content": "old question 1" },
                    { "role": "assistant", "content": "old answer 1" },
                    { "role": "user", "content": "old question 2" },
                    { "role": "assistant", "content": "old answer 2" }
                ]
            }
        }),
    )
    .await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "current answer" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();

    let rows = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, bool)>(
        r#"
        select display_sequence, source_kind, role, content, is_current
        from application_run_conversation_message_items
        where flow_run_id = $1
        order by display_sequence asc
        "#,
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (
                0,
                "imported_context".to_string(),
                Some("system".to_string()),
                Some("Use concise Chinese.".to_string()),
                false,
            ),
            (
                1,
                "imported_context".to_string(),
                Some("user".to_string()),
                Some("old question 1".to_string()),
                false,
            ),
            (
                2,
                "imported_context".to_string(),
                Some("assistant".to_string()),
                Some("old answer 1".to_string()),
                false,
            ),
            (
                3,
                "imported_context".to_string(),
                Some("user".to_string()),
                Some("old question 2".to_string()),
                false,
            ),
            (
                4,
                "imported_context".to_string(),
                Some("assistant".to_string()),
                Some("old answer 2".to_string()),
                false,
            ),
            (5, "current_run".to_string(), None, None, true),
        ]
    );

    let initial_page =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_conversation_message_items_page(
            &store,
            seeded.application_id,
            run.id,
            ListApplicationRunConversationMessageItemsPageInput {
                before_sequence: None,
                after_sequence: None,
                limit: 2,
            },
        )
        .await
        .unwrap();
    assert_eq!(initial_page.total_count, 6);
    assert!(initial_page.has_before);
    assert!(!initial_page.has_after);
    assert_eq!(initial_page.before_cursor, Some(4));
    assert_eq!(initial_page.after_cursor, None);
    assert_eq!(
        initial_page
            .items
            .iter()
            .map(|item| (item.display_sequence, item.source_kind.as_str()))
            .collect::<Vec<_>>(),
        vec![(4, "imported_context"), (5, "current_run")]
    );
    assert_eq!(
        initial_page.items[1].query.as_deref(),
        Some("current question")
    );
    assert_eq!(
        initial_page.items[1].answer.as_deref(),
        Some("current answer")
    );
    assert_eq!(initial_page.items[1].detail_run_id, Some(run.id));
    assert!(initial_page.items[1].can_open_detail);

    let previous_page =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_conversation_message_items_page(
            &store,
            seeded.application_id,
            run.id,
            ListApplicationRunConversationMessageItemsPageInput {
                before_sequence: Some(4),
                after_sequence: None,
                limit: 3,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        previous_page
            .items
            .iter()
            .map(|item| item.display_sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(previous_page.has_before);
    assert!(previous_page.has_after);
    assert_eq!(previous_page.before_cursor, Some(1));
    assert_eq!(previous_page.after_cursor, Some(3));

    let empty_page =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_conversation_message_items_page(
            &store,
            seeded.application_id,
            run.id,
            ListApplicationRunConversationMessageItemsPageInput {
                before_sequence: None,
                after_sequence: Some(5),
                limit: 2,
            },
        )
        .await
        .unwrap();
    assert!(empty_page.items.is_empty());
    assert_eq!(empty_page.total_count, 6);
    assert!(empty_page.has_before);
    assert!(!empty_page.has_after);
    assert_eq!(empty_page.before_cursor, Some(6));
}

#[tokio::test]
async fn terminal_failed_and_cancelled_runs_write_current_projection_items() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-25 10:00:00 UTC);

    for (index, status) in [FlowRunStatus::Failed, FlowRunStatus::Cancelled]
        .into_iter()
        .enumerate()
    {
        let run = seed_run_conversation_flow_run(
            &store,
            &seeded,
            &compiled,
            &format!("run-conversation-terminal-{index}"),
            started_at + Duration::minutes(index as i64),
            json!({
                "node-start": {
                    "query": format!("question {index}")
                }
            }),
        )
        .await;

        <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
            &store,
            &UpdateFlowRunInput {
                flow_run_id: run.id,
                status,
                output_payload: json!({}),
                error_payload: Some(json!({
                    "error": {
                        "message": format!("terminal error {index}")
                    }
                })),
                finished_at: Some(
                    started_at + Duration::minutes(index as i64) + Duration::seconds(2),
                ),
            },
        )
        .await
        .unwrap();

        let current_item =
            <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_conversation_current_item(
                &store,
                seeded.application_id,
                run.id,
            )
            .await
            .unwrap()
            .expect("terminal run should exist");
        assert_eq!(current_item.status, status.as_str());
        assert_eq!(
            current_item.answer.as_deref(),
            Some(format!("terminal error {index}").as_str())
        );

        let projected_current = sqlx::query_as::<_, (String, String)>(
            r#"
            select status, answer
            from application_run_conversation_message_items
            where flow_run_id = $1
              and source_kind = 'current_run'
            "#,
        )
        .bind(run.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(projected_current.0, status.as_str());
        assert_eq!(projected_current.1, format!("terminal error {index}"));
    }
}

#[tokio::test]
async fn terminal_projection_reads_llm_node_system_prompt_when_run_input_has_no_system() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-25 10:30:00 UTC);
    let run = seed_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "run-conversation-llm-system",
        started_at,
        json!({
            "node-start": {
                "query": "current question",
                "model": "gpt-system"
            }
        }),
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
                "prompt_messages": [
                    {
                        "role": "user",
                        "content": "current question"
                    }
                ]
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
                "llm_context": {
                    "effective_system": "Use the node effective system prompt."
                }
            }),
            finished_at: started_at + Duration::seconds(1),
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "current answer" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(2)),
        },
    )
    .await
    .unwrap();

    let projected_system = sqlx::query_as::<_, (i64, String, String)>(
        r#"
        select display_sequence, role, content
        from application_run_conversation_message_items
        where flow_run_id = $1
          and source_kind = 'imported_context'
        order by display_sequence asc
        limit 1
        "#,
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();

    assert_eq!(
        projected_system,
        (
            0,
            "system".to_string(),
            "Use the node effective system prompt.".to_string()
        )
    );
}

#[tokio::test]
async fn migration_repairs_missing_system_projection_without_rebuilding_history() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-25 10:40:00 UTC);
    let run = seed_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "run-conversation-repair-system",
        started_at,
        json!({
            "node-start": {
                "system": "Use the repaired system prompt.",
                "query": "current question",
                "model": "gpt-repair",
                "history": [
                    { "role": "user", "content": "old question" },
                    { "role": "assistant", "content": "old answer" }
                ]
            }
        }),
    )
    .await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "current answer" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(2)),
        },
    )
    .await
    .unwrap();

    sqlx::query(
        r#"
        delete from application_run_conversation_message_items
        where flow_run_id = $1
          and role = 'system'
        "#,
    )
    .bind(run.id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        update application_run_conversation_message_items
        set display_sequence = display_sequence + 1000000000
        where flow_run_id = $1
        "#,
    )
    .bind(run.id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        update application_run_conversation_message_items
        set display_sequence = display_sequence - 1000000001
        where flow_run_id = $1
        "#,
    )
    .bind(run.id)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::raw_sql(include_str!(
        "../../../../migrations/20260630120000_repair_run_conversation_projection_system.sql"
    ))
    .execute(store.pool())
    .await
    .unwrap();

    let rows = sqlx::query_as::<_, (i64, Option<String>, Option<String>, bool)>(
        r#"
        select display_sequence, role, content, is_current
        from application_run_conversation_message_items
        where flow_run_id = $1
        order by display_sequence asc
        "#,
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert_eq!(
        rows,
        vec![
            (
                0,
                Some("system".to_string()),
                Some("Use the repaired system prompt.".to_string()),
                false,
            ),
            (
                1,
                Some("user".to_string()),
                Some("old question".to_string()),
                false,
            ),
            (
                2,
                Some("assistant".to_string()),
                Some("old answer".to_string()),
                false,
            ),
            (3, None, None, true),
        ]
    );
}

#[tokio::test]
async fn terminal_projection_keeps_imported_history_after_artifact_payload_update() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-25 10:45:00 UTC);
    let run = seed_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "run-conversation-artifact-offload",
        started_at,
        json!({
            "node-start": {
                "query": "current question",
                "history": [
                    { "role": "user", "content": "old question from full payload" },
                    { "role": "assistant", "content": "old answer from full payload" }
                ]
            }
        }),
    )
    .await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "current answer" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(2)),
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run_payloads(
        &store,
        &UpdateFlowRunPayloadsInput {
            flow_run_id: run.id,
            input_payload: json!({
                "__runtime_debug_artifact": true,
                "artifact_ref": Uuid::now_v7().to_string(),
                "is_truncated": true,
                "preview": "{\"node-start\":{\"query\":\"current question\""
            }),
            output_payload: json!({
                "answer": {
                    "__runtime_debug_artifact": true,
                    "artifact_ref": Uuid::now_v7().to_string(),
                    "preview": "current answer preview"
                }
            }),
            error_payload: None,
        },
    )
    .await
    .unwrap();

    let projected_text = sqlx::query_as::<_, (String, String)>(
        r#"
        select role, content
        from application_run_conversation_message_items
        where flow_run_id = $1
          and source_kind = 'imported_context'
        order by display_sequence asc
        "#,
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert_eq!(
        projected_text,
        vec![
            (
                "user".to_string(),
                "old question from full payload".to_string()
            ),
            (
                "assistant".to_string(),
                "old answer from full payload".to_string()
            )
        ]
    );
}

#[tokio::test]
async fn non_terminal_run_has_no_projection_but_returns_bounded_current_item() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-25 11:00:00 UTC);
    let run = seed_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "run-conversation-running",
        started_at,
        json!({
            "node-start": {
                "query": "running question",
                "model": "gpt-running"
            }
        }),
    )
    .await;

    let projection_page =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_conversation_message_items_page(
            &store,
            seeded.application_id,
            run.id,
            ListApplicationRunConversationMessageItemsPageInput {
                before_sequence: None,
                after_sequence: None,
                limit: 5,
            },
        )
        .await
        .unwrap();
    assert_eq!(projection_page.total_count, 0);
    assert!(projection_page.items.is_empty());

    let current_item =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_conversation_current_item(
            &store,
            seeded.application_id,
            run.id,
        )
        .await
        .unwrap()
        .expect("running run should return current fallback item");
    assert_eq!(current_item.status, "running");
    assert_eq!(current_item.query.as_deref(), Some("running question"));
    assert_eq!(current_item.model.as_deref(), Some("gpt-running"));
    assert_eq!(current_item.source_kind, "current_run");
}

#[tokio::test]
async fn terminal_projection_missing_rebuilds_conversation_message_projection() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-25 12:00:00 UTC);
    let run = seed_run_conversation_flow_run(
        &store,
        &seeded,
        &compiled,
        "run-conversation-missing-projection",
        started_at,
        json!({
            "node-start": {
                "system": "Use the recovered system prompt.",
                "query": "historical question",
                "history": [
                    { "role": "user", "content": "old question" },
                    { "role": "assistant", "content": "old answer" }
                ]
            }
        }),
    )
    .await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "historical answer" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(1)),
        },
    )
    .await
    .unwrap();

    sqlx::query("delete from application_run_conversation_message_items where flow_run_id = $1")
        .bind(run.id)
        .execute(store.pool())
        .await
        .unwrap();

    let projection_page =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_conversation_message_items_page(
            &store,
            seeded.application_id,
            run.id,
            ListApplicationRunConversationMessageItemsPageInput {
                before_sequence: None,
                after_sequence: None,
                limit: 5,
            },
        )
        .await
        .unwrap();
    assert_eq!(projection_page.total_count, 4);
    assert_eq!(
        projection_page
            .items
            .iter()
            .map(|item| (
                item.display_sequence,
                item.role.as_deref(),
                item.content.as_deref(),
                item.is_current,
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                0,
                Some("system"),
                Some("Use the recovered system prompt."),
                false,
            ),
            (1, Some("user"), Some("old question"), false),
            (2, Some("assistant"), Some("old answer"), false),
            (3, None, None, true),
        ]
    );
}

async fn seed_run_conversation_flow_run(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    debug_session_id: &str,
    started_at: OffsetDateTime,
    input_payload: serde_json::Value,
) -> domain::FlowRunRecord {
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        store,
        &CreateFlowRunInput {
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
            title: "run conversation".to_string(),
            status: FlowRunStatus::Running,
            input_payload,
            started_at,
            api_key_id: None,
            publication_version_id: Some(Uuid::now_v7()),
            external_user: Some("customer-1".to_string()),
            external_conversation_id: Some(debug_session_id.to_string()),
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        },
    )
    .await
    .unwrap()
}
