use std::borrow::Cow;

use sqlx::migrate::Migrator;

use super::*;

const COMPLETE_SUMMARY_BACKFILL_VERSION: i64 = 20260807130000;
const COMPLETE_SUMMARY_BACKFILL_SQL: &str = include_str!(
    "../../../migrations/20260807130000_backfill_complete_application_run_log_summaries.sql"
);

fn before_complete_summary_backfill_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < COMPLETE_SUMMARY_BACKFILL_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

#[tokio::test]
async fn claude_code_builtin_agent_run_is_included_in_run_logs_and_monitoring() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let started_at = datetime!(2026-06-12 18:02:05 UTC);

    let agent_run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "claude-code-builtin-agent".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "Find UI navigation code".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": "Search the codebase under /home/taichu/git/1flowbase/web for UI code.",
                    "system": "You are Claude Code, Anthropic's official CLI for Claude.\n\nYou are a file search specialist for Claude Code, Anthropic's official CLI for Claude.\n\nNotes:\n- Agent threads always have their cwd reset between bash calls, as a result please only use absolute file paths.\n- Do NOT Write report/summary/findings/analysis .md files. Return findings directly as your final assistant message - the parent agent reads your text output, not files you create."
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

    let raw_summary_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_run_log_summaries where flow_run_id = $1",
    )
    .bind(agent_run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(raw_summary_count, 1);

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: agent_run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "web/app/src/app-shell/Navigation.tsx" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(20)),
        },
    )
    .await
    .unwrap();

    let projected_message_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_conversation_messages where flow_run_id = $1",
    )
    .bind(agent_run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(projected_message_count, 0);

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
    assert_eq!(logs.total, 1);
    assert_eq!(logs.items[0].run.id, agent_run.id);

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
    assert_eq!(report.overview.total_count, 1);

    let conversation_runs =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_conversation_runs_page(
            &store,
            seeded.application_id,
            ListApplicationConversationRunsPageInput {
                external_conversation_id: "claude-code-session".to_string(),
                around_run_id: Some(agent_run.id),
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
async fn complete_summary_backfill_restores_a_missing_internal_run_projection() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_complete_summary_backfill_migrator()
        .run(&pool)
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-08-07 10:00:00 UTC);
    let run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "missing-internal-summary-backfill".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "Backfilled internal run".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": "Your task is to create a detailed summary of the conversation so far",
                    "compatibility": {"claude_code_control": "compact_summary"}
                }
            }),
            started_at,
            api_key_id: None,
            publication_version_id: Some(Uuid::now_v7()),
            assistant_conversation_id: None,
            external_user: Some("backfill-user".to_string()),
            external_conversation_id: Some("backfill-conversation".to_string()),
            external_trace_id: None,
            compatibility_mode: Some("anthropic-messages-v1".to_string()),
            idempotency_key: None,
        },
    )
    .await
    .unwrap();

    sqlx::query("delete from application_run_log_summaries where flow_run_id = $1")
        .bind(run.id)
        .execute(store.pool())
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let restored: (String, String, Option<String>) = sqlx::query_as(
        r#"
        select title, status, compatibility_mode
        from application_run_log_summaries
        where flow_run_id = $1
        "#,
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(restored.0, "Backfilled internal run");
    assert_eq!(restored.1, "running");
    assert_eq!(restored.2.as_deref(), Some("anthropic-messages-v1"));

    sqlx::raw_sql(COMPLETE_SUMMARY_BACKFILL_SQL)
        .execute(store.pool())
        .await
        .unwrap();
    let restored_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_run_log_summaries where flow_run_id = $1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(restored_count, 1);
}

#[tokio::test]
async fn claude_code_agent_tool_links_unique_subagent_trace_to_parent_detail() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let started_at = datetime!(2026-06-12 18:02:05 UTC);
    let prompt = "Search the codebase for Agent trace projection.";

    let parent_run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "claude-code-parent".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "Parent Claude Code run".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": "Coordinate Agent workers"
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
    let parent_node_run = seed_node_run_for(
        &store,
        &parent_run,
        "node-parent-llm",
        "llm",
        "Parent LLM",
        json!({ "prompt_messages": ["Coordinate Agent workers"] }),
        started_at + Duration::seconds(1),
    )
    .await;
    let parent_callback_task =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
            &store,
            &CreateCallbackTaskInput {
                flow_run_id: parent_run.id,
                node_run_id: parent_node_run.id,
                callback_kind: "llm_tool_calls".to_string(),
                request_payload: json!({
                    "tool_calls": [
                        {
                            "id": "tooluse-agent-1",
                            "name": "Agent",
                            "arguments": {
                                "description": "Research trace projection",
                                "prompt": prompt
                            }
                        }
                    ]
                }),
                external_ref_payload: None,
            },
        )
        .await
        .unwrap();
    sqlx::query(
        r#"
        update flow_run_callback_tasks
        set status = 'completed',
            created_at = $2,
            completed_at = $3
        where id = $1
        "#,
    )
    .bind(parent_callback_task.id)
    .bind(started_at + Duration::seconds(2))
    .bind(started_at + Duration::seconds(10))
    .execute(store.pool())
    .await
    .unwrap();

    let subagent_run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "claude-code-subagent".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "Research trace projection".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": prompt,
                    "system": "cc_is_subagent=true\nAgent threads always have their cwd reset between bash calls; the parent agent reads your text output."
                }
            }),
            started_at: started_at + Duration::seconds(4),
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
    let subagent_node_run = seed_node_run_for(
        &store,
        &subagent_run,
        "node-subagent-llm",
        "llm",
        "Research agent",
        json!({ "prompt_messages": [prompt] }),
        started_at + Duration::seconds(5),
    )
    .await;
    let subagent_callback_task =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
            &store,
            &CreateCallbackTaskInput {
                flow_run_id: subagent_run.id,
                node_run_id: subagent_node_run.id,
                callback_kind: "llm_tool_calls".to_string(),
                request_payload: json!({
                    "tool_calls": [
                        {
                            "id": "tooluse-bash-1",
                            "name": "Bash",
                            "arguments": {
                                "command": "rg trace_projection"
                            }
                        }
                    ]
                }),
                external_ref_payload: None,
            },
        )
        .await
        .unwrap();

    let projection_source =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source(
            &store,
            seeded.application_id,
            parent_run.id,
        )
        .await
        .unwrap()
        .expect("parent run projection source should be available");

    assert_eq!(projection_source.subagent_traces.len(), 1);
    let subagent_trace = &projection_source.subagent_traces[0];
    assert_eq!(subagent_trace.parent_tool_call_id, "tooluse-agent-1");
    assert_eq!(
        subagent_trace.parent_callback_task_id,
        parent_callback_task.id
    );
    assert_eq!(subagent_trace.source_flow_run.id, subagent_run.id);
    assert_eq!(subagent_trace.node_runs.len(), 1);
    assert_eq!(subagent_trace.node_runs[0].id, subagent_node_run.id);
    assert_eq!(subagent_trace.callback_tasks.len(), 1);
    assert_eq!(
        subagent_trace.callback_tasks[0].id,
        subagent_callback_task.id
    );

    let duplicate_subagent_run =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
            &store,
            &CreateFlowRunInput {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                flow_id: seeded.flow_id,
                flow_draft_id: seeded.draft_id,
                compiled_plan_id: compiled.id,
                debug_session_id: "claude-code-duplicate-subagent".to_string(),
                flow_schema_version: compiled.schema_version.clone(),
                document_hash: compiled.document_hash.clone(),
                run_mode: FlowRunMode::PublishedApiRun,
                target_node_id: None,
                title: "Duplicate trace projection research".to_string(),
                status: FlowRunStatus::Running,
                input_payload: json!({
                    "node-start": {
                        "query": prompt,
                        "system": "cc_is_subagent=true"
                    }
                }),
                started_at: started_at + Duration::seconds(6),
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
    assert_ne!(duplicate_subagent_run.id, subagent_run.id);

    let ambiguous_projection_source =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source(
            &store,
            seeded.application_id,
            parent_run.id,
        )
        .await
        .unwrap()
        .expect("parent run projection source should still be available");
    assert!(
        ambiguous_projection_source.subagent_traces.is_empty(),
        "ambiguous Agent prompt matches must not link any subagent run"
    );
}

#[tokio::test]
async fn claude_code_agent_watermark_counts_batched_agent_tool_calls() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let started_at = datetime!(2026-06-12 19:02:05 UTC);
    let prompt_a = "Inspect backend Agent relation.";
    let prompt_b = "Inspect frontend Agent relation.";

    let parent_run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "claude-code-parent-batched".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "Parent batched Agent run".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": "Coordinate batched Agent workers"
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
    let parent_node_run = seed_node_run_for(
        &store,
        &parent_run,
        "node-parent-llm",
        "llm",
        "Parent LLM",
        json!({ "prompt_messages": ["Coordinate batched Agent workers"] }),
        started_at + Duration::seconds(1),
    )
    .await;
    let parent_callback_task =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
            &store,
            &CreateCallbackTaskInput {
                flow_run_id: parent_run.id,
                node_run_id: parent_node_run.id,
                callback_kind: "llm_tool_calls".to_string(),
                request_payload: json!({
                    "tool_calls": [
                        {
                            "id": "tooluse-agent-a",
                            "name": "Agent",
                            "arguments": {
                                "description": "Backend relation",
                                "prompt": prompt_a
                            }
                        },
                        {
                            "id": "tooluse-agent-b",
                            "name": "Agent",
                            "arguments": {
                                "description": "Frontend relation",
                                "prompt": prompt_b
                            }
                        }
                    ]
                }),
                external_ref_payload: None,
            },
        )
        .await
        .unwrap();
    sqlx::query(
        r#"
        update flow_run_callback_tasks
        set status = 'completed',
            created_at = $2,
            completed_at = $3
        where id = $1
        "#,
    )
    .bind(parent_callback_task.id)
    .bind(started_at + Duration::seconds(2))
    .bind(started_at + Duration::seconds(10))
    .execute(store.pool())
    .await
    .unwrap();

    for (index, prompt) in [prompt_a, prompt_b].into_iter().enumerate() {
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
            &store,
            &CreateFlowRunInput {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                flow_id: seeded.flow_id,
                flow_draft_id: seeded.draft_id,
                compiled_plan_id: compiled.id,
                debug_session_id: format!("claude-code-subagent-batched-{index}"),
                flow_schema_version: compiled.schema_version.clone(),
                document_hash: compiled.document_hash.clone(),
                run_mode: FlowRunMode::PublishedApiRun,
                target_node_id: None,
                title: format!("Batched Agent {index}"),
                status: FlowRunStatus::Running,
                input_payload: json!({
                    "node-start": {
                        "query": prompt,
                        "system": "cc_is_subagent=true\nAgent threads always have their cwd reset between bash calls; the parent agent reads your text output."
                    }
                }),
                started_at: started_at + Duration::seconds(4 + index as i64),
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
    }

    let projection_source =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source(
            &store,
            seeded.application_id,
            parent_run.id,
        )
        .await
        .unwrap()
        .expect("parent run projection source should be available");
    assert_eq!(projection_source.subagent_traces.len(), 2);

    let source_watermark =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source_watermark(
            &store,
            seeded.application_id,
            parent_run.id,
        )
        .await
        .unwrap()
        .expect("parent run projection watermark should be available");
    assert!(
        source_watermark.ends_with("/subagents:2"),
        "batched Agent tool calls must be counted individually in projection watermark: {source_watermark}"
    );
}
