use super::*;

#[tokio::test]
async fn monitoring_source_breakdown_uses_four_explicit_run_modes() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-07-20 08:00:00 UTC);

    for (index, run_mode) in [
        FlowRunMode::PublishedApiRun,
        FlowRunMode::WorkflowHttpRun,
        FlowRunMode::WorkflowScheduleRun,
        FlowRunMode::DebugFlowRun,
    ]
    .into_iter()
    .enumerate()
    {
        let run = seed_flow_run_with_mode(
            &store,
            &seeded,
            &compiled,
            started_at + Duration::seconds(index as i64),
            run_mode,
            None,
        )
        .await;
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
            &store,
            &UpdateFlowRunInput {
                flow_run_id: run.id,
                status: FlowRunStatus::Succeeded,
                output_payload: json!({}),
                error_payload: None,
                finished_at: Some(started_at + Duration::seconds(index as i64 + 1)),
            },
        )
        .await
        .unwrap();
    }

    let report =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_monitoring_report(
            &store,
            seeded.application_id,
            GetApplicationRunMonitoringReportInput {
                started_from: Some(started_at - Duration::seconds(1)),
                started_to: Some(started_at + Duration::minutes(1)),
                bucket: "hour".to_string(),
                slow_run_threshold_ms: 30_000,
            },
        )
        .await
        .unwrap();

    let sources = report
        .sources
        .into_iter()
        .map(|item| item.invocation_source)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        sources,
        ["agent_flow_api", "workflow_http", "workflow_schedule", "debug"]
            .into_iter()
            .map(str::to_string)
            .collect()
    );
}

#[tokio::test]
async fn conversation_message_history_ignores_legacy_claude_code_control_runs() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let started_at = datetime!(2026-06-04 13:10:00 UTC);
    let external_user = "claude-code-user";
    let external_conversation_id = "claude-code-session";
    let visible_run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "visible-claude-code-run".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "visible question".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": "visible question"
                }
            }),
            started_at,
            api_key_id: Some(api_key_id),
            publication_version_id: Some(Uuid::now_v7()),
            external_user: Some(external_user.to_string()),
            external_conversation_id: Some(external_conversation_id.to_string()),
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
            flow_run_id: visible_run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "visible answer" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();

    let legacy_control_run =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
            &store,
            &CreateFlowRunInput {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                flow_id: seeded.flow_id,
                flow_draft_id: seeded.draft_id,
                compiled_plan_id: compiled.id,
                debug_session_id: "legacy-claude-code-compact".to_string(),
                flow_schema_version: compiled.schema_version.clone(),
                document_hash: compiled.document_hash.clone(),
                run_mode: FlowRunMode::PublishedApiRun,
                target_node_id: None,
                title: "compact resume".to_string(),
                status: FlowRunStatus::Running,
                input_payload: json!({
                    "node-start": {
                        "query": "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\nSummary: hi\n\nIf you need specific details from before compaction (like exact code snippets, error messages, or content you generated), read the full transcript at: C:\\Users\\Lw\\.claude\\projects\\repo\\session.jsonl"
                    }
                }),
                started_at: started_at + Duration::seconds(10),
                api_key_id: Some(api_key_id),
                publication_version_id: Some(Uuid::now_v7()),
                external_user: Some(external_user.to_string()),
                external_conversation_id: Some(external_conversation_id.to_string()),
                external_trace_id: None,
                compatibility_mode: Some("anthropic-messages-v1".to_string()),
                idempotency_key: None,
            },
        )
        .await
        .unwrap();

    let conversation_id: Uuid = sqlx::query_scalar(
        r#"
        select id
        from application_conversations
        where application_id = $1
          and api_key_id = $2
          and external_user = $3
          and external_conversation_id = $4
        "#,
    )
    .bind(seeded.application_id)
    .bind(api_key_id)
    .bind(external_user)
    .bind(external_conversation_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    for (role, content, ordinal) in [
        (
            "user",
            "This session is being continued from a previous conversation that ran out of context.",
            1_i64,
        ),
        ("assistant", "已恢复上下文。", 2_i64),
    ] {
        sqlx::query(
            r#"
            insert into application_conversation_messages (
                id,
                scope_id,
                conversation_id,
                application_id,
                flow_run_id,
                node_run_id,
                role,
                content,
                sequence,
                status,
                started_at,
                finished_at,
                created_at,
                updated_at
            ) values ($1, $2, $3, $4, $5, null, $6, $7, $8, 'succeeded', $9, $10, $9, $10)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(seeded.workspace_id)
        .bind(conversation_id)
        .bind(seeded.application_id)
        .bind(legacy_control_run.id)
        .bind(role)
        .bind(content)
        .bind((started_at + Duration::seconds(10)).unix_timestamp() * 1_000_000 + ordinal)
        .bind(started_at + Duration::seconds(10))
        .bind(started_at + Duration::seconds(13))
        .execute(store.pool())
        .await
        .unwrap();
    }

    let messages =
        <PgControlPlaneStore as control_plane::application_public_api::conversations::ApplicationPublicConversationRepository>::list_application_public_conversation_messages(
            &store,
            &control_plane::application_public_api::conversations::ListApplicationPublicConversationMessagesInput {
                application_id: seeded.application_id,
                api_key_id,
                external_user: external_user.to_string(),
                external_conversation_id: external_conversation_id.to_string(),
                limit: 10,
            },
        )
        .await
        .unwrap();

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "visible question");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "visible answer");
}

#[tokio::test]
async fn terminal_published_run_without_external_conversation_projects_run_messages() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let started_at = datetime!(2026-05-29 13:10:00 UTC);
    let run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "published-run-scoped-conversation".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "hi".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start": {
                    "query": "hi"
                }
            }),
            started_at,
            api_key_id: Some(api_key_id),
            publication_version_id: Some(Uuid::now_v7()),
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: Some("openai-chat-completions-v1".to_string()),
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
                "answer": {
                    "preview": "\"Hello",
                    "artifact_ref": "run-answer-artifact",
                    "content_type": "application/json"
                }
            }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();

    let projected_messages = sqlx::query_as::<_, (String, String, String)>(
        r#"
        select conversations.external_conversation_id, messages.role, messages.content
        from application_conversation_messages messages
        join application_conversations conversations on conversations.id = messages.conversation_id
        where messages.flow_run_id = $1
        order by messages.sequence asc, messages.id asc
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
                format!("flow-run:{}", run.id),
                "user".to_string(),
                "hi".to_string()
            ),
            (
                format!("flow-run:{}", run.id),
                "assistant".to_string(),
                "Hello".to_string()
            )
        ]
    );
}

#[tokio::test]
async fn failed_flow_run_log_summary_keeps_recorded_usage_ledger_tokens() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-05-29 02:45:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let node_run = seed_node_run_for(
        &store,
        &run,
        "node-llm",
        "llm",
        "LLM",
        json!({ "prompt": "统计失败前已消耗 tokens" }),
        started_at + Duration::seconds(1),
    )
    .await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_node_run(
        &store,
        &UpdateNodeRunInput {
            node_run_id: node_run.id,
            status: NodeRunStatus::Failed,
            output_payload: json!({}),
            error_payload: Some(json!({ "message": "provider runtime timed out" })),
            metrics_payload: json!({ "resumed": true, "callback_kind": "llm_tool_calls" }),
            debug_payload: json!({}),
            finished_at: Some(started_at + Duration::seconds(20)),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_usage_ledger(
        &store,
        &AppendUsageLedgerInput {
            flow_run_id: run.id,
            node_run_id: Some(node_run.id),
            span_id: None,
            failover_attempt_id: None,
            provider_instance_id: None,
            gateway_route_id: None,
            model_id: Some("gpt-5.3-codex-spark".into()),
            upstream_model_id: Some("gpt-5.3-codex-spark".into()),
            upstream_request_id: Some("req-timeout".into()),
            input_tokens: Some(40),
            cached_input_tokens: None,
            output_tokens: Some(2),
            reasoning_output_tokens: None,
            total_tokens: Some(42),
            input_cache_hit_tokens: None,
            input_cache_miss_tokens: None,
            cache_read_tokens: Some(11),
            cache_write_tokens: None,
            price_snapshot: None,
            cost_snapshot: None,
            usage_status: domain::UsageLedgerStatus::Recorded,
            raw_usage: json!({ "total_tokens": 42 }),
            normalized_usage: json!({ "total_tokens": 42 }),
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Failed,
            output_payload: json!({}),
            error_payload: Some(json!({ "message": "provider runtime timed out" })),
            finished_at: Some(started_at + Duration::seconds(30)),
        },
    )
    .await
    .unwrap();

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
    assert_eq!(logs.items[0].run.id, run.id);
    assert_eq!(logs.items[0].total_tokens, Some(42));
    assert_eq!(logs.items[0].input_tokens, Some(40));
    assert_eq!(logs.items[0].output_tokens, Some(2));
    assert_eq!(logs.items[0].input_cache_hit_tokens, Some(11));

    let report =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_monitoring_report(
            &store,
            seeded.application_id,
            GetApplicationRunMonitoringReportInput {
                started_from: Some(started_at - Duration::minutes(1)),
                started_to: Some(started_at + Duration::minutes(1)),
                bucket: "hour".to_string(),
                slow_run_threshold_ms: 30_000,
            },
        )
        .await
        .unwrap();
    assert_eq!(report.tokens.total_tokens_sum, 42);
    assert_eq!(report.tokens.input_tokens_sum, 40);
    assert_eq!(report.tokens.output_tokens_sum, 2);
    assert_eq!(report.tokens.input_cache_hit_tokens_sum, 11);
    assert_eq!(report.tokens.token_recorded_count, 1);
    assert_eq!(report.high_token_runs[0].flow_run_id, run.id);
}

#[tokio::test]
async fn application_run_logs_and_monitoring_read_static_summaries_only() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-05-24 10:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "完成" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(5)),
        },
    )
    .await
    .unwrap();

    sqlx::query("delete from application_run_log_summaries where flow_run_id = $1")
        .bind(run.id)
        .execute(store.pool())
        .await
        .unwrap();

    let logs =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_application_run_logs_page(
            &store,
            seeded.application_id,
            ListApplicationRunsPageInput {
                page: 1,
                page_size: 20,
                created_after: Some(started_at - Duration::minutes(1)),
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
                started_to: Some(started_at + Duration::minutes(1)),
                bucket: "hour".to_string(),
                slow_run_threshold_ms: 30_000,
            },
        )
        .await
        .unwrap();
    assert_eq!(report.overview.total_count, 0);
}

#[tokio::test]
async fn application_run_monitoring_compares_tokens_with_previous_window() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let current_started_at = datetime!(2026-05-24 10:02:00 UTC);
    let previous_started_at = datetime!(2026-05-24 09:55:00 UTC);
    let current_run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        current_started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let previous_run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        previous_started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;

    upsert_terminal_summary_tokens(&store, current_run.id, 200).await;
    upsert_terminal_summary_tokens(&store, previous_run.id, 100).await;

    let report =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_monitoring_report(
            &store,
            seeded.application_id,
            GetApplicationRunMonitoringReportInput {
                started_from: Some(datetime!(2026-05-24 10:00:00 UTC)),
                started_to: Some(datetime!(2026-05-24 10:10:00 UTC)),
                bucket: "hour".to_string(),
                slow_run_threshold_ms: 30_000,
            },
        )
        .await
        .unwrap();

    assert_eq!(report.tokens.total_tokens_sum, 200);
    assert_eq!(report.tokens_comparison.previous_total_tokens_sum, 100);
    assert_eq!(report.tokens_comparison.previous_run_count, 1);
    assert_eq!(report.tokens_comparison.token_change_rate, 1.0);
    assert_eq!(report.tokens_comparison.traffic_effect, 1.0);
    assert_eq!(report.tokens_comparison.cost_per_run_effect, 2.0);
}

#[tokio::test]
async fn application_run_monitoring_report_aggregates_terminal_log_summaries_by_started_at() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let started_at = datetime!(2026-05-24 09:00:00 UTC);

    let console_run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let console_first_node = seed_node_run_for(
        &store,
        &console_run,
        "node-llm",
        "llm",
        "LLM",
        json!({ "prompt": "总结退款政策" }),
        started_at + Duration::seconds(1),
    )
    .await;
    let _console_second_node = seed_node_run_for(
        &store,
        &console_run,
        "node-tool",
        "tool",
        "Tool",
        json!({ "tool_name": "lookup_order" }),
        started_at + Duration::seconds(2),
    )
    .await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_node_run(
        &store,
        &UpdateNodeRunInput {
            node_run_id: console_first_node.id,
            status: NodeRunStatus::Succeeded,
            output_payload: json!({ "answer": "ok" }),
            error_payload: None,
            metrics_payload: json!({ "usage": { "total_tokens": 100 } }),
            debug_payload: json!({}),
            finished_at: Some(started_at + Duration::seconds(3)),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
        &store,
        &CreateCallbackTaskInput {
            flow_run_id: console_run.id,
            node_run_id: console_first_node.id,
            callback_kind: "llm_tool_calls".to_string(),
            request_payload: json!({
                "tool_calls": [{ "id": "call-1" }, { "id": "call-2" }]
            }),
            external_ref_payload: None,
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: console_run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({ "answer": "完成" }),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(5)),
        },
    )
    .await
    .unwrap();

    let public_run = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        &store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "published-api-run".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: compiled.document_hash.clone(),
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "Customer refund".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({ "message": "refund" }),
            started_at: started_at + Duration::seconds(2),
            api_key_id: Some(api_key_id),
            publication_version_id: Some(Uuid::now_v7()),
            external_user: Some("customer-1".to_string()),
            external_conversation_id: Some("conversation-1".to_string()),
            external_trace_id: Some("trace-1".to_string()),
            compatibility_mode: Some("openai-responses-v1".to_string()),
            idempotency_key: Some("idem-1".to_string()),
        },
    )
    .await
    .unwrap();
    let public_node = seed_node_run_for(
        &store,
        &public_run,
        "node-llm",
        "llm",
        "LLM",
        json!({ "prompt": "退款" }),
        started_at + Duration::seconds(3),
    )
    .await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_node_run(
        &store,
        &UpdateNodeRunInput {
            node_run_id: public_node.id,
            status: NodeRunStatus::Failed,
            output_payload: json!({}),
            error_payload: Some(json!({ "message": "timeout" })),
            metrics_payload: json!({ "usage": { "total_tokens": 400 } }),
            debug_payload: json!({}),
            finished_at: Some(started_at + Duration::seconds(12)),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: public_run.id,
            status: FlowRunStatus::Failed,
            output_payload: json!({}),
            error_payload: Some(json!({ "message": "timeout" })),
            finished_at: Some(started_at + Duration::seconds(42)),
        },
    )
    .await
    .unwrap();

    sqlx::query(
        r#"
        update application_run_log_summaries
        set total_tokens = case flow_run_id when $1 then 100 when $2 then 400 end,
            input_tokens = case flow_run_id when $1 then 80 when $2 then 300 end,
            output_tokens = case flow_run_id when $1 then 20 when $2 then 100 end,
            input_cache_hit_tokens = case flow_run_id when $1 then 10 when $2 then 50 end
        where flow_run_id in ($1, $2)
        "#,
    )
    .bind(console_run.id)
    .bind(public_run.id)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::query("delete from api_keys where id = $1")
        .bind(api_key_id)
        .execute(store.pool())
        .await
        .unwrap();

    let outside_run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at - Duration::days(10),
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: outside_run.id,
            status: FlowRunStatus::Cancelled,
            output_payload: json!({}),
            error_payload: None,
            finished_at: Some(started_at - Duration::days(10) + Duration::seconds(1)),
        },
    )
    .await
    .unwrap();

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

    assert_eq!(report.overview.total_count, 2);
    assert_eq!(report.overview.success_count, 1);
    assert_eq!(report.overview.failed_count, 1);
    assert_eq!(report.overview.cancelled_count, 0);
    assert!(!report.overview.running_count_included);
    assert_eq!(report.duration.duration_recorded_count, 2);
    assert_eq!(report.duration.avg_duration_ms.round() as i64, 22_500);
    assert_eq!(report.duration.p50_duration_ms.round() as i64, 22_500);
    assert_eq!(report.duration.p95_duration_ms.round() as i64, 38_250);
    assert_eq!(report.duration.slow_run_rate, 0.5);
    assert_eq!(report.tokens.total_tokens_sum, 500);
    assert_eq!(report.tokens.input_tokens_sum, 380);
    assert_eq!(report.tokens.output_tokens_sum, 120);
    assert_eq!(report.tokens.input_cache_hit_tokens_sum, 60);
    assert_eq!(report.tokens.avg_tokens_per_run, 250.0);
    assert_eq!(report.tokens.token_recorded_count, 2);
    assert_eq!(report.tool_callbacks.total_tool_callback_count, 2);
    assert_eq!(report.tool_callbacks.runs_with_tool_callback, 1);
    assert_eq!(report.nodes.avg_unique_node_count, 1.5);
    assert_eq!(report.nodes.max_unique_node_count, 2);
    assert_eq!(report.concurrency.peak_concurrency, 2);
    assert_eq!(report.tokens_trend[0].total_tokens, 500);
    assert_eq!(report.tokens_trend[0].input_tokens, 380);
    assert_eq!(report.tokens_trend[0].output_tokens, 120);
    assert_eq!(report.tokens_trend[0].input_cache_hit_tokens, 60);
    assert_eq!(report.protocols[0].protocol, "default");
    assert_eq!(report.protocols[1].protocol, "openai-responses-v1");
    assert_eq!(report.sources[0].invocation_source, "debug");
    assert_eq!(report.sources[1].invocation_source, "agent_flow_api");
    assert_eq!(
        report.external_users[0].external_user.as_deref(),
        Some("customer-1")
    );
    assert_eq!(report.api_keys[0].api_key_id, api_key_id);
    assert_eq!(
        report.api_keys[0].api_key_name_snapshot.as_deref(),
        Some("application api key")
    );
    assert_eq!(
        report.external_conversations[0]
            .external_conversation_id
            .as_deref(),
        Some("conversation-1")
    );
    assert_eq!(report.slowest_runs[0].flow_run_id, public_run.id);
    assert_eq!(report.high_token_runs[0].flow_run_id, public_run.id);
}
