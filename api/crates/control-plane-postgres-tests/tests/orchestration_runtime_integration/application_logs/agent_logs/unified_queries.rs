use super::*;
use domain::{ResourceFilterExpr as F, ResourceFilterOperator as O};

fn field(name: &str, operator: O, value: serde_json::Value) -> F {
    F::Field {
        field: name.into(),
        operator,
        value,
    }
}
fn records(filter: F) -> ApplicationLogRecordsQuery {
    ApplicationLogRecordsQuery {
        filter,
        sort_field: "finished_at".into(),
        descending: false,
        cursor: None,
        limit: 1,
    }
}
fn trajectory(keyword: Option<&str>) -> RecordClientTrajectoryQuery {
    RecordClientTrajectoryQuery {
        filter: F::All(vec![]),
        cursor: None,
        limit: 1,
        keyword: keyword.map(Into::into),
        search_sections: vec![],
    }
}

#[tokio::test]
async fn agent_logs_unified_query_mixed_sources_typed_filters_and_null_last_cursor() {
    let (store, scope, app) = setup().await;
    let service = AgentLogsService::new(store.clone());
    let mut ids = vec![];
    for i in 0..3 {
        let mut e = event(
            &format!("query-{i}"),
            1,
            AgentLogEventKind::User,
            Some("query needle"),
        );
        e.source_task_id = format!("task-{i}");
        e.reasoning_effort = Some("high".into());
        ids.push(
            service
                .ingest(app, scope, Uuid::now_v7(), batch(vec![e]))
                .await
                .unwrap()
                .record_ids[0],
        );
    }
    // Equal sort keys exercise the UUID tie-breaker; the third record is NULL.
    sqlx::query("update application_run_log_tasks set finished_at='2026-10-08T00:00:00Z',total_tokens=12,total_cost=0.25 where id=any($1)")
        .bind(&ids[..2]).execute(store.pool()).await.unwrap();
    ids[..2].sort();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let native = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        OffsetDateTime::now_utc(),
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    sqlx::query("update application_run_log_summaries set requested_model_id='native-query-model',reasoning_effort='medium',total_cost=0.5 where flow_run_id=$1").bind(native.id).execute(store.pool()).await.unwrap();
    let all = store
        .query_application_log_records(
            scope,
            &[app],
            &ApplicationLogRecordsQuery {
                limit: 20,
                ..records(F::All(vec![]))
            },
        )
        .await
        .unwrap();
    assert_eq!(
        all.items.iter().map(|r| r.record_id).collect::<Vec<_>>(),
        ids
    );
    assert_eq!(all.items[0].source_id.as_deref(), Some("collector"));
    assert_eq!(all.items[0].source_kind, "imported");
    assert_eq!(
        all.items[0].requested_model_id.as_deref(),
        Some("unknown-model")
    );
    assert_eq!(all.items[0].reasoning_effort.as_deref(), Some("high"));
    assert_eq!(
        all.items[0]
            .total_cost
            .as_deref()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        0.25
    );
    // Native may belong to a different workspace; only query its actual scope.
    let mixed_scope = seeded.workspace_id;
    let mut e = event("mixed", 1, AgentLogEventKind::User, Some("mixed imported"));
    e.source_task_id = "mixed-task".into();
    let owner = seeded.actor_user_id;
    let imported_app = store
        .create_application(&CreateApplicationInput {
            actor_user_id: owner,
            workspace_id: mixed_scope,
            application_type: ApplicationType::AgentLogs,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "mixed".into(),
            description: "".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let imported = service
        .ingest(imported_app.id, mixed_scope, Uuid::now_v7(), batch(vec![e]))
        .await
        .unwrap()
        .record_ids[0];
    let mixed = store
        .query_application_log_records(
            mixed_scope,
            &[seeded.application_id, imported_app.id],
            &ApplicationLogRecordsQuery {
                limit: 20,
                ..records(F::All(vec![]))
            },
        )
        .await
        .unwrap();
    assert_eq!(mixed.items.len(), 2);
    assert_eq!(
        mixed
            .items
            .iter()
            .find(|r| r.record_id == native.id)
            .unwrap()
            .native_run_id,
        Some(native.id)
    );
    assert_eq!(
        mixed
            .items
            .iter()
            .find(|r| r.record_id == imported)
            .unwrap()
            .source_kind,
        "imported"
    );
    let native_summary = mixed
        .items
        .iter()
        .find(|r| r.record_id == native.id)
        .unwrap();
    assert_eq!(native_summary.source_kind, "native");
    assert_eq!(
        native_summary.requested_model_id.as_deref(),
        Some("native-query-model")
    );
    assert_eq!(native_summary.reasoning_effort.as_deref(), Some("medium"));
    assert_eq!(
        native_summary
            .total_cost
            .as_deref()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        0.5
    );
    let filter = F::All(vec![
        field("application_id", O::Eq, json!(app)),
        field("total_tokens", O::Gte, json!(12)),
        F::Any(vec![
            field("user_input", O::Includes, json!("NEED%")),
            field("source_kind", O::Eq, json!("native")),
        ]),
        field("finished_at", O::Lt, json!("2026-10-09T08:00:00+08:00")),
    ]);
    let mut query = records(filter);
    let first = store
        .query_application_log_records(scope, &[app], &query)
        .await
        .unwrap();
    assert_eq!(first.items[0].record_id, ids[0]);
    query.cursor = first.next_cursor;
    assert!(query.cursor.is_some());
    let second = store
        .query_application_log_records(scope, &[app], &query)
        .await
        .unwrap();
    assert_eq!(second.items[0].record_id, ids[1]);
    assert!(second.next_cursor.is_none());
    query.descending = true;
    assert!(store
        .query_application_log_records(scope, &[app], &query)
        .await
        .is_err());
    query.descending = false;
    assert!(store
        .query_application_log_records(scope, &[], &query)
        .await
        .is_err());
    assert!(store
        .query_application_log_records(Uuid::now_v7(), &[app], &query)
        .await
        .is_err());
    let bad = records(field("total_tokens", O::Includes, json!("12")));
    assert!(store
        .query_application_log_records(scope, &[], &bad)
        .await
        .is_err());
    assert!(store
        .query_application_log_records(Uuid::now_v7(), &[app], &records(F::All(vec![])))
        .await
        .unwrap()
        .items
        .is_empty());
    let mut query = records(F::All(vec![]));
    let mut paged = vec![];
    loop {
        let page = store
            .query_application_log_records(scope, &[app], &query)
            .await
            .unwrap();
        paged.extend(page.items.into_iter().map(|r| r.record_id));
        query.cursor = page.next_cursor;
        if query.cursor.is_none() {
            break;
        }
        assert!(paged.len() <= 3);
    }
    assert_eq!(paged, ids);
}

#[tokio::test]
async fn agent_logs_unified_query_searches_beyond_source_page_restores_nul_and_binds_record() {
    let (store, scope, app) = setup().await;
    let mut events = (0..105)
        .map(|i| {
            event(
                &format!("deep-{i}"),
                i,
                AgentLogEventKind::Assistant,
                Some("ordinary progress"),
            )
        })
        .collect::<Vec<_>>();
    events[104].content = Some("before\0deep needle after".into());
    let id = AgentLogsService::new(store.clone())
        .ingest(app, scope, Uuid::now_v7(), batch(events))
        .await
        .unwrap()
        .record_ids[0];
    let page = store
        .query_record_client_trajectory(app, id, &trajectory(Some("deep needle")))
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].item_id.as_deref(), Some("deep-104"));
    assert_eq!(page.search_sections, vec!["result"]);
    assert_eq!(page.matches.len(), 1);
    let hit = &page.matches[0];
    assert_eq!(hit.step_id, page.items[0].id);
    assert_eq!(hit.section, "result");
    assert!(hit.snippet.contains('\0'));
    let section = store
        .record_client_trajectory_section(app, id, hit.step_id, &hit.section, None, 8)
        .await
        .unwrap()
        .unwrap();
    assert!(section
        .items
        .iter()
        .any(|part| part.sequence == hit.sequence
            && part.value == json!("before\0deep needle after")));
    assert!(store
        .query_record_client_trajectory(Uuid::now_v7(), id, &trajectory(Some("deep needle")))
        .await
        .is_err());
    let mut query = trajectory(None);
    let first = store
        .query_record_client_trajectory(app, id, &query)
        .await
        .unwrap();
    query.cursor = first.next_cursor;
    assert!(query.cursor.is_some());
    query.keyword = Some("ordinary".into());
    assert!(store
        .query_record_client_trajectory(app, id, &query)
        .await
        .is_err());
    query.keyword = None;
    assert!(store
        .query_record_client_trajectory(app, Uuid::now_v7(), &query)
        .await
        .is_err());
    query.cursor = None;
    query.filter = field("request_id", O::Eq, json!("not-a-uuid"));
    assert!(store
        .query_record_client_trajectory(app, Uuid::now_v7(), &query)
        .await
        .is_err());
    let record = store
        .application_log_record(app, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.source_id.as_deref(), Some("collector"));
    assert!(!record.messages.iter().any(|m| m.role == "assistant"));
}

#[tokio::test]
async fn agent_logs_unified_query_semantic_hit_beyond_section_page_has_real_locator() {
    let db = isolated_database().await;
    let store = PgControlPlaneStore::new(db.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
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
    let request_id = Uuid::now_v7();
    let step_id = Uuid::now_v7();
    let at = "2026-10-08T00:00:00Z";
    let step = ClientTrajectoryStep {
        id: step_id,
        request_id,
        sequence: 1,
        created_at: at.into(),
        category: "tool".into(),
        name: "section fixture".into(),
        namespace: None,
        preview: "short preview without keyword".into(),
        parameters_preview: None,
        result_preview: None,
        status: "observed".into(),
        origin: "native".into(),
        protocol: "responses".into(),
        transport: ClientTrajectoryTransport::Http,
        flow_run_id: Some(run.id),
        node_run_id: None,
        parent_id: None,
        call_id: None,
        item_id: None,
        response_id: None,
        turn_id: None,
        related_step_id: None,
        available_sections: vec!["result".into()],
    };
    for fact in [
        ClientTrajectoryFact::Integrity {
            status: "pending".into(),
            dropped_count: 0,
            persist_failed_count: 0,
        },
        ClientTrajectoryFact::Step {
            step: Box::new(step),
        },
    ] {
        store
            .append_client_trajectory(&AppendClientTrajectoryInput {
                flow_run_id: run.id,
                node_run_id: None,
                request_id,
                observed_at: at.into(),
                fact,
            })
            .await
            .unwrap();
    }
    for i in 0..40 {
        store
            .append_client_trajectory(&AppendClientTrajectoryInput {
                flow_run_id: run.id,
                node_run_id: None,
                request_id,
                observed_at: at.into(),
                fact: ClientTrajectoryFact::Section {
                    step_id,
                    section: "result".into(),
                    value: json!(if i == 39 {
                        "late section needle"
                    } else {
                        "irrelevant"
                    }),
                },
            })
            .await
            .unwrap();
    }
    let first = store
        .record_client_trajectory_section(
            seeded.application_id,
            run.id,
            step_id,
            "result",
            None,
            32,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.items.len(), 32);
    assert!(first.next_cursor.is_some());
    assert!(first
        .items
        .iter()
        .all(|v| v.value != json!("late section needle")));
    let result = store
        .query_record_client_trajectory(
            seeded.application_id,
            run.id,
            &trajectory(Some("late section needle")),
        )
        .await
        .unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].step_id, step_id);
    assert_eq!(result.matches[0].section, "result");
    let second = store
        .record_client_trajectory_section(
            seeded.application_id,
            run.id,
            step_id,
            "result",
            first.next_cursor,
            32,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(second.items.iter().any(
        |v| v.sequence == result.matches[0].sequence && v.value == json!("late section needle")
    ));
    let mut query = trajectory(Some("late section needle"));
    query.filter = field("preview", O::Includes, json!("needle"));
    assert!(store
        .query_record_client_trajectory(seeded.application_id, run.id, &query)
        .await
        .unwrap()
        .items
        .is_empty());
}

use control_plane_contracts::application_public_runtime::ApplicationPublishedFlowRunRepository;
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

#[tokio::test]
async fn agent_logs_unified_native_overview_keeps_persisted_final_without_phase_and_context_order()
{
    let store = PgControlPlaneStore::new(isolated_database().await.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started = OffsetDateTime::now_utc();
    let run=seed_native_run_conversation_flow_run(&store,&seeded,&compiled,"unified-final",started,json!({"__native_model_prompt_context":{"system":[{"text":"client system"}],"messages":[{"role":"developer","content":"client developer"}]},"node-start":{"query":"old question","system":"execution system"}}),Some(json!({"role":"user","content":"current question"}))).await;
    for item in [
        json!({"type":"message","role":"assistant","phase":"commentary","content":[{"type":"output_text","text":"progress only"}]}),
        json!({"type":"custom_tool_call","name":"exec","input":"tool only"}),
        json!({"type":"error","message":"error only"}),
    ] {
        append_provider_output_item(&store, run.id, item).await;
    }
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({"answer":"persisted final without phase"}),
            error_payload: None,
            finished_at: Some(started + Duration::seconds(1)),
        })
        .await
        .unwrap();
    let overview = store
        .application_log_record(seeded.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        overview.projection_output.is_none(),
        "persisted final belongs to messages; projection output is supplemental"
    );
    assert_eq!(overview.status, "succeeded");
    assert!(overview.output_state.is_some());
    let formal = store
        .list_application_run_conversation_message_items_page(
            seeded.application_id,
            run.id,
            ListApplicationRunConversationMessageItemsPageInput {
                before_sequence: None,
                after_sequence: None,
                limit: 1,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        formal
            .contexts
            .iter()
            .map(|c| (c.role.as_str(), c.content.as_str()))
            .collect::<Vec<_>>(),
        overview.messages[..2]
            .iter()
            .map(|m| (m.role.as_str(), m.content.as_str()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        formal.items[0].answer.as_deref(),
        Some("persisted final without phase")
    );
    let contents = overview
        .messages
        .iter()
        .map(|m| (m.role.as_str(), m.content.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        contents,
        vec![
            ("system", "client system"),
            ("developer", "client developer"),
            ("user", "current question"),
            ("assistant", "persisted final without phase")
        ]
    );
    assert_eq!(overview.native_run_id, Some(run.id));
    assert_eq!(overview.source_kind, "native");
}
