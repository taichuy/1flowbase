use super::*;
use control_plane_contracts::application_public_runtime::ApplicationPublishedFlowRunRepository;
use control_plane_contracts::ports::ApplicationRunLogContext;

fn task_fixture_input(
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    key: Uuid,
    title: &str,
) -> CreateFlowRunInput {
    CreateFlowRunInput {
        application_run_log_context: None,
        actor_user_id: seeded.actor_user_id,
        application_id: seeded.application_id,
        flow_id: seeded.flow_id,
        flow_draft_id: seeded.draft_id,
        compiled_plan_id: compiled.id,
        debug_session_id: String::new(),
        flow_schema_version: compiled.schema_version.clone(),
        document_hash: compiled.document_hash.clone(),
        run_mode: FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: title.into(),
        status: FlowRunStatus::Running,
        // The snapshot must expose the caller's selection from node-start, not
        // the internal sys value that a later routing stage may carry.
        input_payload: json!({
            "query": title,
            "history": [],
            "node-start": {"model": "gpt-5.6-sol"},
            "sys": {
                "requested_model_id": "routed-model-id",
                "model_parameters": {"reasoning": {"effort": "medium"}}
            }
        }),
        started_at: OffsetDateTime::now_utc(),
        api_key_id: Some(key),
        publication_version_id: None,
        assistant_conversation_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: Some("openai-responses".into()),
        idempotency_key: None,
    }
}

async fn append_output_item(
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
            payload: json!({"item":item}),
        })
        .await
        .unwrap();
}

async fn finish(store: &PgControlPlaneStore, flow_run_id: Uuid, tokens: i64) {
    sqlx::query("insert into runtime_usage_ledger(id,flow_run_id,total_tokens,usage_status) values($1,$2,$3,'recorded')")
        .bind(Uuid::now_v7()).bind(flow_run_id).bind(tokens).execute(store.pool()).await.unwrap();
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({"answer":"done"}),
            error_payload: None,
            finished_at: Some(OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
}

// #2035 AC-001/002/003/005/006: the task projection is the list unit, the run
// projection stays pure, and the anchor detail converges to input and output.
#[tokio::test]
async fn issue_2035_task_projection_owns_list_and_converged_detail() {
    let database = isolated_database().await;
    let store = PgControlPlaneStore::new(database.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;

    // One user turn: generate → compact → generate(final answer).
    let mut input = task_fixture_input(&seeded, &compiled, key, "refactor login");
    let mut members = Vec::new();
    for (index, call_kind) in ["generate", "compact", "generate"].iter().enumerate() {
        input.idempotency_key = Some(format!("task-{index}"));
        input.application_run_log_context = Some(ApplicationRunLogContext {
            identity_status: "identified".into(),
            protocol: Some("openai_responses".into()),
            call_kind: Some((*call_kind).into()),
            thread_id: Some("thread-T".into()),
            turn_id: Some("turn-T".into()),
            prompt: Some(json!({"role":"user","content":"refactor login"})),
            ..Default::default()
        });
        let created =
            ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
                .await
                .unwrap();
        let run_id = created.flow_run.id;
        if index == 0 {
            append_output_item(&store, run_id, json!({"type":"custom_tool_call","id":"tool-0","call_id":"call-0","name":"exec","input":"cat entry.txt"})).await;
        }
        if index == 2 {
            append_output_item(&store, run_id, json!({"type":"message","id":"msg-final","role":"assistant","phase":"final_answer","content":[{"type":"output_text","text":"login refactored"}]})).await;
        }
        finish(&store, run_id, 1000 * (index as i64 + 1)).await;
        members.push(run_id);
    }
    // A child task spawned from the turn (Codex subagent thread).
    input.idempotency_key = Some("child-task".into());
    input.title = "review diff".into();
    input.input_payload = json!({"query":"review diff","history":[]});
    input.application_run_log_context = Some(ApplicationRunLogContext {
        identity_status: "identified".into(),
        protocol: Some("openai_responses".into()),
        call_kind: Some("generate".into()),
        subagent_kind: Some("review".into()),
        thread_id: Some("thread-child".into()),
        turn_id: Some("turn-child".into()),
        parent_thread_id: Some("thread-T".into()),
        parent_turn_id: Some("turn-T".into()),
        ..Default::default()
    });
    let child = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap()
        .flow_run
        .id;
    finish(&store, child, 500).await;
    // A plain run without any client identity is its own task.
    input.idempotency_key = Some("plain".into());
    input.title = "plain question".into();
    input.input_payload = json!({"query":"plain question","history":[]});
    input.application_run_log_context = None;
    let plain = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap()
        .flow_run
        .id;
    finish(&store, plain, 70).await;

    // AC-001: every run belongs to exactly one task row.
    let rows: Vec<(Uuid, Vec<Uuid>, Option<Uuid>, bool, String, Option<String>, Option<String>, Option<Uuid>, i64, i64, Option<i64>)> = sqlx::query_as(
        "select id,member_run_ids,parent_task_run_id,is_root,outcome,user_input,final_output,final_output_run_id,invocation_count,compaction_count,total_tokens from application_run_log_tasks where application_id=$1 order by id",
    )
    .bind(seeded.application_id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    let member_rows: Vec<(Uuid, String, Option<i64>, Option<Uuid>, i64)> = sqlx::query_as(
        "select flow_run_id,status,total_tokens,log_task_run_id,invocation_count from application_run_log_summaries where application_id=$1 order by flow_run_id",
    )
    .bind(seeded.application_id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    eprintln!("task_rows={rows:?}\nmember_rows={member_rows:?}");
    assert_eq!(rows.len(), 3, "task, child task, plain run: {rows:?}");
    let task = rows
        .iter()
        .find(|row| row.0 == members[0])
        .expect("task anchor row");
    assert_eq!(task.1, members);
    assert_eq!(task.2, None);
    assert!(task.3);
    assert_eq!(task.4, "final_answer_observed");
    assert_eq!(task.5.as_deref(), Some("refactor login"));
    assert_eq!(task.6.as_deref(), Some("login refactored"));
    assert_eq!(task.7, Some(members[2]));
    assert_eq!((task.8, task.9, task.10), (2, 1, Some(6000)));
    let child_row = rows
        .iter()
        .find(|row| row.0 == child)
        .expect("child task row");
    assert_eq!(child_row.2, Some(members[0]));
    assert!(!child_row.3);
    let plain_row = rows
        .iter()
        .find(|row| row.0 == plain)
        .expect("plain task row");
    assert_eq!(plain_row.1, vec![plain]);
    assert!(plain_row.3);
    assert_eq!(plain_row.5.as_deref(), Some("plain question"));
    assert_eq!(plain_row.6.as_deref(), Some("done"));
    let member_runs: i64 = sqlx::query_scalar(
        "select count(*) from application_run_log_tasks t, unnest(t.member_run_ids) m where t.application_id=$1",
    )
    .bind(seeded.application_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(member_runs, 5);

    // AC-002: the console list reads root tasks; the run model stays pure.
    let page = store
        .list_application_run_logs_page(
            seeded.application_id,
            ListApplicationRunsPageInput {
                page: 1,
                page_size: 100,
                created_after: None,
                sort_by: None,
                sort_order: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 2, "child tasks never become list rows");
    let listed = page
        .items
        .iter()
        .find(|item| item.run.id == members[0])
        .unwrap();
    assert_eq!(listed.member_run_ids, members);
    assert_eq!(listed.outcome, "final_answer_observed");
    assert_eq!(listed.final_output.as_deref(), Some("login refactored"));
    assert_eq!((listed.invocation_count, listed.compaction_count), (2, 1));
    assert_eq!(listed.total_tokens, Some(6000));
    assert_eq!(listed.requested_model_id.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(listed.reasoning_effort.as_deref(), Some("medium"));
    let snapshot_rows: Vec<(Option<String>, Option<String>)> = sqlx::query_as(
        "select requested_model_id,reasoning_effort from application_run_log_summaries where flow_run_id=$1 union all select requested_model_id,reasoning_effort from application_run_log_tasks where id=$1",
    )
    .bind(members[0])
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        snapshot_rows,
        vec![
            (Some("gpt-5.6-sol".into()), Some("medium".into())),
            (Some("gpt-5.6-sol".into()), Some("medium".into())),
        ],
        "the task list reads its snapshot without provider-log joins"
    );
    let metadata = store.list_runtime_model_metadata().await.unwrap();
    let list_records = |code: &str| {
        let metadata = metadata
            .iter()
            .find(|model| model.model_code == code)
            .unwrap_or_else(|| panic!("runtime model {code}"))
            .clone();
        let store = &store;
        let application_id = seeded.application_id;
        async move {
            storage_durable::runtime_record_repository::RuntimeRecordRepository::list_records(
                store,
                &metadata,
                storage_durable::runtime_record_repository::RuntimeListQuery {
                    scope_id: None,
                    owner_user_id: None,
                    filter: domain::ResourceFilterExpr::Field {
                        field: "application_id".into(),
                        operator: domain::ResourceFilterOperator::Eq,
                        value: json!(application_id),
                    },
                    sorts: vec![],
                    expand_relations: vec![],
                    page: 1,
                    page_size: 100,
                },
            )
            .await
            .unwrap()
        }
    };
    let summaries = list_records("application_run_log_summaries").await;
    assert_eq!(summaries.total, 5, "run projection lists one row per run");
    assert!(summaries
        .items
        .iter()
        .all(|row| row["invocation_count"].as_i64().unwrap() <= 1));
    let tasks = list_records("application_run_log_tasks").await;
    assert_eq!(tasks.total, 3);
    let task_row = tasks
        .items
        .iter()
        .find(|row| row["id"] == members[0].to_string())
        .unwrap();
    assert_eq!(task_row["invocation_count"], 2);
    assert_eq!(task_row["compaction_count"], 1);
    assert_eq!(task_row["outcome"], "final_answer_observed");
    assert_eq!(task_row["is_root"], true);
    assert_eq!(
        store
            .expand_application_run_log_tasks(seeded.application_id, &[members[1]])
            .await
            .unwrap(),
        members
    );

    // #2105: opening any member resolves to its owning business turn.
    let task_row = store
        .get_application_run_log_task(seeded.application_id, members[0])
        .await
        .unwrap()
        .expect("task row");
    assert_eq!(task_row.client_thread_id.as_deref(), Some("thread-T"));
    let member_messages = store
        .list_application_run_conversation_message_items_page(
            seeded.application_id,
            members[1],
            ListApplicationRunConversationMessageItemsPageInput {
                limit: 50,
                before_sequence: None,
                after_sequence: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(member_messages.items.len(), 1);
    assert_eq!(member_messages.items[0].id, members[0]);
    assert_eq!(member_messages.items[0].detail_run_id, Some(members[0]));
    assert_eq!(member_messages.items[0].flow_run_id, members[2]);
    assert_eq!(
        member_messages.items[0].answer.as_deref(),
        Some("login refactored")
    );

    // AC-004/005 sources: the anchor detail carries member rounds and child tasks.
    let detail = store
        .get_application_run_detail(seeded.application_id, members[0])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        detail
            .task_rounds
            .iter()
            .map(|round| round.source_flow_run.id)
            .collect::<Vec<_>>(),
        members[1..]
    );
    assert_eq!(detail.task_rounds[0].call_kind, "compact");
    assert!(detail.task_rounds[1]
        .native_messages
        .iter()
        .any(|message| message["_source_item"]["phase"] == "final_answer"));
    assert_eq!(detail.child_task_traces.len(), 1);
    assert_eq!(detail.child_task_traces[0].source_flow_run.id, child);
    assert_eq!(
        detail.child_task_traces[0].subagent_kind.as_deref(),
        Some("review")
    );
    let member_detail = store
        .get_application_run_detail(seeded.application_id, members[1])
        .await
        .unwrap()
        .unwrap();
    assert!(member_detail.task_rounds.is_empty() && member_detail.child_task_traces.is_empty());
    let watermark = store
        .get_application_run_trace_projection_source_watermark(seeded.application_id, members[0])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        watermark,
        control_plane::orchestration_runtime::trace_projection::trace_projection_source_watermark(
            &detail
        )
    );
    assert!(watermark.contains("task_rounds:2"));
}

#[tokio::test]
async fn issue_2105_business_turn_pages_keep_facts_and_reads_do_not_lock_writers() {
    let database = isolated_database().await;
    let store = PgControlPlaneStore::new(database.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let mut ids = Vec::new();
    for index in 0..6 {
        let mut input = task_fixture_input(&seeded, &compiled, key, &format!("Question {index}"));
        input.input_payload["system"] = json!(format!("System {index}"));
        input.application_run_log_context = Some(ApplicationRunLogContext {
            identity_status: "identified".into(),
            protocol: Some("openai_responses".into()),
            thread_id: Some("business-turn-page".into()),
            turn_id: Some(format!("turn-{index}")),
            prompt: Some(json!({"role":"user","content":format!("Question {index}")})),
            ..Default::default()
        });
        let run = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
            .await
            .unwrap()
            .flow_run;
        append_output_item(&store, run.id, json!({"type":"custom_tool_call","id":format!("tool-{index}"),"call_id":format!("call-{index}"),"name":"exec","input":"pwd"})).await;
        append_output_item(&store, run.id, json!({"type":"message","id":format!("intermediate-{index}"),"role":"assistant","phase":"commentary","content":[{"type":"output_text","text":"Working..."}]})).await;
        if index < 5 {
            append_output_item(&store, run.id, json!({"type":"message","id":format!("final-{index}"),"role":"assistant","phase":if index == 4 { serde_json::Value::Null } else { json!("final_answer") },"content":[{"type":"output_text","text":format!("Answer {index}")}]})).await;
            finish(&store, run.id, 0).await;
        }
        ids.push(run.id);
    }
    let task = store
        .get_application_run_log_task(seeded.application_id, ids[0])
        .await
        .unwrap()
        .unwrap();
    let conversation = task.log_conversation_id.unwrap().to_string();
    let page = store
        .list_application_conversation_runs_page(
            seeded.application_id,
            ListApplicationConversationRunsPageInput {
                external_conversation_id: conversation.clone(),
                around_run_id: Some(ids[0]),
                before_run_id: None,
                after_run_id: None,
                limit: 5,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        page.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        ids[1..]
    );
    assert!(page.has_before);
    assert!(!page.has_after);
    assert_eq!(page.items[0].answer.as_deref(), Some("Answer 1"));
    assert_eq!(
        page.items[3].answer.as_deref(),
        Some("Answer 4"),
        "successful ordinary text without phase is retained"
    );
    assert!(
        page.items[4].answer.is_none(),
        "intermediate messages are not a final answer"
    );
    let older = store
        .list_application_conversation_runs_page(
            seeded.application_id,
            ListApplicationConversationRunsPageInput {
                external_conversation_id: conversation,
                around_run_id: None,
                before_run_id: page.before_cursor,
                after_run_id: None,
                limit: 5,
            },
        )
        .await
        .unwrap();
    assert_eq!(older.items.len(), 1);
    assert_eq!(older.items[0].id, ids[0]);
    let mut writer = store.pool().begin().await.unwrap();
    sqlx::query("select id from flow_runs where id=$1 for update")
        .bind(ids[5])
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    let turn = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        store.list_application_run_conversation_message_items_page(
            seeded.application_id,
            ids[5],
            ListApplicationRunConversationMessageItemsPageInput {
                before_sequence: None,
                after_sequence: None,
                limit: 5,
            },
        ),
    )
    .await
    .expect("chat GET must not wait for a writer row lock")
    .unwrap();
    assert_eq!(turn.items.len(), 1);
    assert!(turn.items[0].answer.is_none());
    assert!(turn
        .contexts
        .iter()
        .any(|context| context.flow_run_id == ids[5] && context.content == "System 5"));
    writer.rollback().await.unwrap();
    let retained: i64 = sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1 and event_type='provider_output_item_done'")
        .bind(ids[5]).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        retained, 2,
        "raw tool and intermediate output facts remain retained"
    );
}

#[tokio::test]
async fn issue_2105_failed_payload_is_not_answer_and_successful_plain_text_is() {
    let database = isolated_database().await;
    let store = PgControlPlaneStore::new(database.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    for (index, status) in [FlowRunStatus::Failed, FlowRunStatus::Succeeded]
        .into_iter()
        .enumerate()
    {
        let input = task_fixture_input(&seeded, &compiled, key, &format!("ordinary-{index}"));
        let run = store.create_flow_run(&input).await.unwrap();
        store
            .update_flow_run(&UpdateFlowRunInput {
                flow_run_id: run.id,
                status,
                output_payload: if index == 1 {
                    json!({"answer":"actual answer"})
                } else {
                    json!({})
                },
                error_payload: if index == 0 {
                    Some(json!({"message":"provider failed"}))
                } else {
                    None
                },
                finished_at: Some(OffsetDateTime::now_utc()),
            })
            .await
            .unwrap();
        let page = store
            .list_application_run_conversation_message_items_page(
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
        assert_eq!(page.items.len(), 1);
        if index == 0 {
            assert!(page.items[0].answer.is_none());
        } else {
            assert_eq!(page.items[0].answer.as_deref(), Some("actual answer"));
        }
    }
}
