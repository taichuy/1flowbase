use super::*;

#[tokio::test]
async fn terminal_published_run_projects_application_conversation_messages_once() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let started_at = datetime!(2026-05-29 13:00:00 UTC);
    let conversation_id = Uuid::now_v7();
    let run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "published-conversation-projection".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "退款政策".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "system": "请使用简洁中文回答。",
                    "history": [
                        {"role": "user", "content": "上一轮问题"},
                        {"role": "assistant", "content": "上一轮回答"}
                    ],
                    "query": "退款政策是什么？"
                }
            }),
            started_at,
            api_key_id: Some(api_key_id),
            publication_version_id: Some(Uuid::now_v7()),
            assistant_conversation_id: None,
            external_user: Some("customer-1".to_string()),
            external_conversation_id: Some("conversation-1".to_string()),
            external_trace_id: None,
            compatibility_mode: Some("native-v1".to_string()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into application_conversations (
            id,
            scope_id,
            application_id,
            api_key_id,
            external_user,
            external_conversation_id,
            created_at,
            updated_at
        ) values ($1, $2, $3, $4, 'customer-1', 'conversation-1', $5, $5)
        "#,
    )
    .bind(conversation_id)
    .bind(seeded.workspace_id)
    .bind(seeded.application_id)
    .bind(api_key_id)
    .bind(started_at)
    .execute(store.pool())
    .await
    .unwrap();

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "7 天内可申请退款。" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();

    let projected_messages = sqlx::query_as::<_, (String, String, String, i64)>(
        r#"
        select role, content, status, sequence
        from application_conversation_messages
        where flow_run_id = $1
        order by sequence asc, id asc
        "#,
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        projected_messages,
        vec![
            (
                "system".to_string(),
                "请使用简洁中文回答。".to_string(),
                "succeeded".to_string(),
                1_780_059_600_000_001
            ),
            (
                "user".to_string(),
                "退款政策是什么？".to_string(),
                "succeeded".to_string(),
                1_780_059_600_000_002
            ),
            (
                "assistant".to_string(),
                "7 天内可申请退款。".to_string(),
                "succeeded".to_string(),
                1_780_059_600_000_003
            )
        ]
    );

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "7 天内可申请退款。" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(4)),
        },
    )
    .await
    .unwrap();

    let projected_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_conversation_messages where flow_run_id = $1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(projected_count, 3);

    sqlx::query(
        r#"
        update flow_runs
        set input_payload = '{"node-start":{"query":"raw list query must not leak"}}'::jsonb,
            output_payload = '{"answer":"raw list answer must not leak"}'::jsonb,
            error_payload = '{"error":{"message":"raw list error must not leak"}}'::jsonb
        where id = $1
        "#,
    )
    .bind(run.id)
    .execute(store.pool())
    .await
    .unwrap();

    let conversation_runs =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_conversation_runs_page(
            &store,
            seeded.application_id,
            ListApplicationConversationRunsPageInput {
                external_conversation_id: "conversation-1".to_string(),
                around_run_id: Some(run.id),
                before_run_id: None,
                after_run_id: None,
                limit: 20,
            },
        )
        .await
        .unwrap();

    assert_eq!(conversation_runs.items.len(), 1);
    assert_eq!(conversation_runs.items[0].id, run.id);
    assert_eq!(
        conversation_runs.items[0].query.as_deref(),
        Some("退款政策是什么？")
    );
    assert_eq!(
        conversation_runs.items[0].answer.as_deref(),
        Some("7 天内可申请退款。")
    );
    assert_eq!(conversation_runs.items[0].model, None);
}

#[tokio::test]
async fn terminal_claude_code_control_run_does_not_project_conversation_messages() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let started_at = datetime!(2026-06-04 13:00:00 UTC);
    let run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "claude-code-compact-projection".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "compact summary".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": "Your task is to create a detailed summary of the conversation so far",
                    "compatibility": {
                        "claude_code_control": "compact_summary"
                    }
                }
            }),
            started_at,
            api_key_id: Some(api_key_id),
            publication_version_id: Some(Uuid::now_v7()),
            assistant_conversation_id: None,
            external_user: Some("claude-code-user".to_string()),
            external_conversation_id: Some("claude-code-session".to_string()),
            external_trace_id: None,
            compatibility_mode: Some("anthropic-messages-v1".to_string()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "<summary>internal</summary>" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();

    let projected_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_conversation_messages where flow_run_id = $1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(projected_count, 0);

    let logs =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_logs_page(
            &store,
            seeded.application_id,
            ListApplicationRunsPageInput {
                page: 1,
                page_size: 20,
                created_after: None,
                sort_by: Some("created_at".to_string()),
                sort_order: Some("desc".to_string()),
            },
        )
        .await
        .unwrap();
    assert_eq!(logs.total, 0);

    sqlx::query(
        r#"
        insert into application_run_log_summaries (
            flow_run_id,
            scope_id,
            application_id,
            run_mode,
            status,
            target_node_id,
            title,
            input_payload,
            external_user,
            authorized_account,
            api_key_id,
            api_key_name_snapshot,
            publication_version_id,
            external_conversation_id,
            external_trace_id,
            compatibility_mode,
            idempotency_key,
            total_tokens,
            input_tokens,
            output_tokens,
            input_cache_hit_tokens,
            unique_node_count,
            tool_callback_count,
            started_at,
            finished_at,
            created_at,
            updated_at
        )
        select
            flow_runs.id,
            applications.workspace_id,
            flow_runs.application_id,
            flow_runs.run_mode,
            flow_runs.status,
            flow_runs.target_node_id,
            flow_runs.title,
            '{}'::jsonb,
            flow_runs.external_user,
            (
                select users.account
                from users
                where users.id = flow_runs.created_by
            ),
            flow_runs.api_key_id,
            null,
            flow_runs.publication_version_id,
            flow_runs.external_conversation_id,
            flow_runs.external_trace_id,
            flow_runs.compatibility_mode,
            flow_runs.idempotency_key,
            0,
            0,
            0,
            0,
            0,
            0,
            flow_runs.started_at,
            flow_runs.finished_at,
            flow_runs.created_at,
            flow_runs.updated_at
        from flow_runs
        join applications on applications.id = flow_runs.application_id
        where flow_runs.id = $1
        "#,
    )
    .bind(run.id)
    .execute(store.pool())
    .await
    .unwrap();

    let raw_stale_summary_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_run_log_summaries where flow_run_id = $1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(raw_stale_summary_count, 1);

    let logs =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_logs_page(
            &store,
            seeded.application_id,
            ListApplicationRunsPageInput {
                page: 1,
                page_size: 20,
                created_after: None,
                sort_by: Some("created_at".to_string()),
                sort_order: Some("desc".to_string()),
            },
        )
        .await
        .unwrap();
    assert_eq!(logs.total, 0);
    assert!(logs.items.is_empty());

    let report =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_monitoring_report(
            &store,
            seeded.application_id,
            GetApplicationRunMonitoringReportInput {
                started_from: Some(started_at - Duration::minutes(1)),
                started_to: Some(started_at + Duration::minutes(10)),
                bucket: "hour".to_string(),
                slow_run_threshold_ms: 30_000,
            },
        )
        .await
        .unwrap();
    assert_eq!(report.overview.total_count, 0);

    let conversation_runs =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_conversation_runs_page(
            &store,
            seeded.application_id,
            ListApplicationConversationRunsPageInput {
                external_conversation_id: "claude-code-session".to_string(),
                around_run_id: Some(run.id),
                before_run_id: None,
                after_run_id: None,
                limit: 20,
            },
        )
        .await
        .unwrap();
    assert!(conversation_runs.items.is_empty());
}

#[tokio::test]
async fn terminal_claude_code_away_summary_run_does_not_project_business_logs() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let started_at = datetime!(2026-06-04 13:00:00 UTC);
    let run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "claude-code-away-summary".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "away summary".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": "The user stepped away and is coming back. Write exactly 1-3 short sentences. Start by stating the high-level task — what they are building or debugging, not implementation details. Next: the concrete next step. Skip status reports and commit recaps."
                }
            }),
            started_at,
            api_key_id: Some(api_key_id),
            publication_version_id: Some(Uuid::now_v7()),
            assistant_conversation_id: None,
external_user: Some("claude-code-user".to_string()),
            external_conversation_id: Some("claude-code-session".to_string()),
            external_trace_id: None,
            compatibility_mode: Some("anthropic-messages-v1".to_string()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({
                "answer": "You are debugging Claude Code callback handling. Next, resume the pending callback."
            }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();

    let projected_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_conversation_messages where flow_run_id = $1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(projected_count, 0);

    let summary_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_run_log_summaries where flow_run_id = $1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(summary_count, 0);
}

#[tokio::test]
async fn terminal_claude_code_compact_resume_run_without_transcript_does_not_project_business_logs()
{
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let started_at = datetime!(2026-06-04 13:05:00 UTC);
    let run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "claude-code-compact-resume".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "compact resume".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\nSummary:\n- user asked where uploads/image-1.png is implemented\n\nContinue the conversation from where it left off without asking the user any further questions. Resume directly — do not acknowledge the summary, do not recap what was happening, do not preface with \"I'll continue\" or similar. Pick up the last task as if the break never happened."
                }
            }),
            started_at,
            api_key_id: Some(api_key_id),
            publication_version_id: Some(Uuid::now_v7()),
            assistant_conversation_id: None,
external_user: Some("claude-code-user".to_string()),
            external_conversation_id: Some("claude-code-session".to_string()),
            external_trace_id: None,
            compatibility_mode: Some("anthropic-messages-v1".to_string()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({
                "answer": "Continue by checking the application list page."
            }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();

    let projected_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_conversation_messages where flow_run_id = $1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(projected_count, 0);

    let summary_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_run_log_summaries where flow_run_id = $1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(summary_count, 0);
}
