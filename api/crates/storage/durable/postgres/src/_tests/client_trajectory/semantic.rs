use super::*;
use sha2::{Digest, Sha256};

async fn seed() -> (PgControlPlaneStore, Uuid) {
    let (pool, run) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    (PgControlPlaneStore::new(pool), run)
}

async fn add_step(store: &PgControlPlaneStore, run: Uuid, request: Uuid, id: Uuid) {
    append(
        store,
        run,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(step(run, None, request, id, "emitted", "tool_call")),
        },
    )
    .await;
}

async fn value(store: &PgControlPlaneStore, run: Uuid, id: Uuid, name: &str) -> Value {
    store
        .client_trajectory_section(run, None, id, name, None, 32)
        .await
        .unwrap()
        .unwrap()
        .items[0]
        .value
        .clone()
}

#[tokio::test]
async fn client_semantic_directories_share_only_complete_original_values_and_preserve_occurrences()
{
    let (store, run) = seed().await;
    let body = json!({"arguments":" {\"n\":\"\\u0000\"} \n", "content":[{"type":"output_text","text":"hello\0world"}],
        "summary":"different child", "key\0":"exact original", "key\\u0000":"distinct original"});
    let mut requests = Vec::new();
    for _ in 0..2 {
        let request = Uuid::now_v7();
        requests.push(request);
        begin(&store, run, None, request).await;
        add_step(&store, run, request, request).await;
        section(
            &store,
            run,
            None,
            request,
            request,
            "overview",
            body.clone(),
        )
        .await;
        section(
            &store,
            run,
            None,
            request,
            request,
            "parameters",
            body["arguments"].clone(),
        )
        .await;
        section(
            &store,
            run,
            None,
            request,
            request,
            "result",
            body["content"].clone(),
        )
        .await;
        for _ in 0..2 {
            section(
                &store,
                run,
                None,
                request,
                request,
                "timing",
                json!({"observed_at":AT}),
            )
            .await;
        }
        // Nonstandard timing still has exact JSON semantics and a distinct occurrence.
        section(
            &store,
            run,
            None,
            request,
            request,
            "timing",
            json!({"observed_at":AT,"extra":"\0"}),
        )
        .await;
        assert_eq!(value(&store, run, request, "overview").await, body);
        assert_eq!(
            value(&store, run, request, "parameters").await,
            body["arguments"]
        );
        assert_eq!(value(&store, run, request, "result").await, body["content"]);
        let first = store
            .client_trajectory_section(run, None, request, "timing", None, 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.items[0].value, json!({"observed_at":AT}));
        let second = store
            .client_trajectory_section(run, None, request, "timing", first.next_cursor, 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.items[0].value, first.items[0].value);
        assert!(second.items[0].sequence > first.items[0].sequence);
        let third = store
            .client_trajectory_section(run, None, request, "timing", second.next_cursor, 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(third.items[0].value, json!({"observed_at":AT,"extra":"\0"}));
        assert!(third.next_cursor.is_none());
    }
    let duplicated_events:i64=sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1 and payload#>>'{fact,kind}' in ('step','section')")
        .bind(run).fetch_one(store.pool()).await.unwrap();
    assert_eq!(duplicated_events, 0);
    let shared:Vec<(Uuid,String,Uuid,Vec<String>)>=sqlx::query_as("select step_id,section,content_id,content_path from client_trajectory_sections where flow_run_id=$1 and section in ('overview','parameters','result') order by event_sequence")
        .bind(run).fetch_all(store.pool()).await.unwrap();
    assert!(shared.iter().all(|row| row.2 == shared[0].2));
    assert_eq!(shared[1].3, vec!["arguments"]);
    assert_eq!(shared[2].3, vec!["content"]);
    let single_time:i64=sqlx::query_scalar("select count(*) from client_trajectory_sections where flow_run_id=$1 and body_kind='timing' and content_id is null and event_id is null")
        .bind(run).fetch_one(store.pool()).await.unwrap();
    assert_eq!(single_time, 4);
    // Equal query projections are insufficient: original NUL and its literal escape differ.
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    add_step(&store, run, request, request).await;
    section(
        &store,
        run,
        None,
        request,
        request,
        "overview",
        json!({"arguments":"actual\0"}),
    )
    .await;
    section(
        &store,
        run,
        None,
        request,
        request,
        "parameters",
        json!("actual\\u0000"),
    )
    .await;
    let ids:Vec<Uuid>=sqlx::query_scalar("select content_id from client_trajectory_sections where step_id=$1 order by event_sequence")
        .bind(request).fetch_all(store.pool()).await.unwrap();
    assert_ne!(ids[0], ids[1]);
    assert_eq!(
        value(&store, run, request, "parameters").await,
        json!("actual\\u0000")
    );
    assert!(store
        .client_trajectory_section(Uuid::now_v7(), None, requests[0], "result", None, 1)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn client_semantic_locator_hash_and_content_scope_negatives_reject_wrong_values() {
    let (store, run) = seed().await;
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    add_step(&store, run, request, request).await;
    section(
        &store,
        run,
        None,
        request,
        request,
        "overview",
        json!({"content":"right","summary":"wrong"}),
    )
    .await;
    section(
        &store,
        run,
        None,
        request,
        request,
        "result",
        json!("right"),
    )
    .await;
    let (id,content):(Uuid,Uuid)=sqlx::query_as("select id,content_id from client_trajectory_sections where step_id=$1 and section='result'")
        .bind(request).fetch_one(store.pool()).await.unwrap();
    sqlx::query("update client_trajectory_sections set content_path=array['summary'] where id=$1")
        .bind(id)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .client_trajectory_section(run, None, request, "result", None, 1)
        .await
        .is_err());
    sqlx::query("update client_trajectory_sections set content_path=array['content'] where id=$1")
        .bind(id)
        .execute(store.pool())
        .await
        .unwrap();
    let original_hash: String =
        sqlx::query_scalar("select value_hash from client_trajectory_sections where id=$1")
            .bind(id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    sqlx::query("update client_trajectory_sections set value_hash='sha256:wrong' where id=$1")
        .bind(id)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .client_trajectory_section(run, None, request, "result", None, 1)
        .await
        .is_err());
    sqlx::query("update client_trajectory_sections set value_hash=$2 where id=$1")
        .bind(id)
        .bind(original_hash)
        .execute(store.pool())
        .await
        .unwrap();
    let scope = Uuid::now_v7();
    sqlx::query("insert into workspaces(id,tenant_id,name) select $1,w.tenant_id,'Wrong client scope' from workspaces w join flow_runs f on f.scope_id=w.id where f.id=$2")
        .bind(scope).bind(run).execute(store.pool()).await.unwrap();
    let app = Uuid::now_v7();
    sqlx::query("insert into applications(id,workspace_id,application_type,name,description,created_by,updated_by) select $1,$2,'agent_flow','Wrong client app','',created_by,created_by from flow_runs where id=$3")
        .bind(app).bind(scope).bind(run).execute(store.pool()).await.unwrap();
    let foreign = Uuid::now_v7();
    sqlx::query("insert into runtime_canonical_contents(id,scope_id,application_id,content_hash,content,byte_size) values($1,$2,$3,$4,$5,42)")
        .bind(foreign).bind(scope).bind(app).bind(format!("sha256:{}","0".repeat(64)))
        .bind(json!({"content":"right"})).execute(store.pool()).await.unwrap();
    sqlx::query("update client_trajectory_sections set content_id=$2 where id=$1")
        .bind(id)
        .bind(foreign)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .client_trajectory_section(run, None, request, "result", None, 1)
        .await
        .is_err());
    sqlx::query("update client_trajectory_sections set content_id=$2 where id=$1")
        .bind(id)
        .bind(content)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(value(&store, run, request, "result").await, json!("right"));
    // Controlled canonical hash collision must fail before an occurrence is persisted.
    let target = json!("collision target");
    let hash = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&target).unwrap())
    );
    sqlx::query("insert into runtime_canonical_contents(id,scope_id,application_id,content_hash,content,byte_size) select $1,scope_id,application_id,$2,$3,$4 from flow_runs where id=$5")
        .bind(Uuid::now_v7()).bind(hash).bind(json!("wrong immutable original"))
        .bind(serde_json::to_vec(&target).unwrap().len() as i64).bind(run).execute(store.pool()).await.unwrap();
    assert!(store
        .append_client_trajectory(&AppendClientTrajectoryInput {
            flow_run_id: run,
            node_run_id: None,
            request_id: request,
            observed_at: AT.into(),
            fact: ClientTrajectoryFact::Section {
                step_id: request,
                section: "schema".into(),
                value: target
            }
        })
        .await
        .is_err());
    let n: i64 = sqlx::query_scalar(
        "select count(*) from client_trajectory_sections where step_id=$1 and section='schema'",
    )
    .bind(request)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(n, 0);
}

fn event(run: Uuid) -> AppendRuntimeEventInput {
    AppendRuntimeEventInput {
        flow_run_id: run,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        event_type: "fixture_client_sequence".into(),
        layer: domain::RuntimeEventLayer::RuntimeItem,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload: json!({}),
        visibility: domain::RuntimeEventVisibility::Internal,
        durability: domain::RuntimeEventDurability::Durable,
    }
}

#[tokio::test]
async fn client_semantic_high_water_preserves_batch_and_concurrent_runtime_sequences() {
    let (store, run) = seed().await;
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    add_step(&store, run, request, request).await;
    let stable = store
        .client_trajectory_page(run, None, None, 10)
        .await
        .unwrap()
        .items[0]
        .sequence;
    add_step(&store, run, request, request).await;
    let before: i64 =
        sqlx::query_scalar("select runtime_event_sequence_high_water from flow_runs where id=$1")
            .bind(run)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(
        before > stable,
        "metadata revisions reserve their own durable sequence"
    );
    let batch = store
        .append_runtime_events(&[event(run), event(run), event(run)])
        .await
        .unwrap();
    assert_eq!(
        batch.iter().map(|e| e.sequence).collect::<Vec<_>>(),
        vec![before + 1, before + 2, before + 3]
    );
    let section_input = AppendClientTrajectoryInput {
        flow_run_id: run,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact: ClientTrajectoryFact::Section {
            step_id: request,
            section: "schema".into(),
            value: json!({"type":"object"}),
        },
    };
    let runtime_input = event(run);
    let (directory, runtime) = tokio::join!(
        store.append_client_trajectory(&section_input),
        store.append_runtime_event(&runtime_input)
    );
    directory.unwrap();
    runtime.unwrap();
    let mut sequences:Vec<i64>=sqlx::query_scalar("select sequence from runtime_events where flow_run_id=$1 union all select event_sequence from client_trajectory_steps where flow_run_id=$1 union all select event_sequence from client_trajectory_sections where flow_run_id=$1")
        .bind(run).fetch_all(store.pool()).await.unwrap();
    sequences.sort_unstable();
    assert!(!sequences.windows(2).any(|pair| pair[0] == pair[1]));
    let final_event = store.append_runtime_event(&event(run)).await.unwrap();
    assert_eq!(final_event.sequence, *sequences.last().unwrap() + 1);
    assert_eq!(
        store
            .client_trajectory_page(run, None, None, 10)
            .await
            .unwrap()
            .items[0]
            .sequence,
        stable
    );
}

#[tokio::test]
async fn client_semantic_metadata_originals_control_call_matching_and_namespace_inheritance() {
    let (store, run) = seed().await;
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    let exact = Uuid::now_v7();
    for (id, call_id) in [(exact, "actual\0id"), (Uuid::now_v7(), "actual\\u0000id")] {
        let mut call = step(run, None, request, id, "emitted", "tool_call");
        call.name = "original\0name".into();
        call.call_id = Some(call_id.into());
        call.namespace = Some("namespace\0".into());
        append(
            &store,
            run,
            None,
            request,
            ClientTrajectoryFact::Step {
                step: Box::new(call),
            },
        )
        .await;
    }
    let next = Uuid::now_v7();
    begin(&store, run, None, next).await;
    let mut result = step(run, None, next, next, "submitted", "tool_result");
    result.call_id = Some("actual\0id".into());
    append(
        &store,
        run,
        None,
        next,
        ClientTrajectoryFact::Step {
            step: Box::new(result),
        },
    )
    .await;
    let page = store
        .client_trajectory_page(run, None, None, 10)
        .await
        .unwrap();
    assert_eq!(page.items[0].name, "original\0name");
    let result = page.items.iter().find(|step| step.id == next).unwrap();
    assert_eq!(result.related_step_id, Some(exact));
    assert_eq!(result.namespace.as_deref(), Some("namespace\0"));
}

async fn legacy_event(
    store: &PgControlPlaneStore,
    input: &AppendClientTrajectoryInput,
    projection: Value,
) -> Uuid {
    let id = Uuid::now_v7();
    let originals = json!({"payload":serde_json::to_string(input).unwrap()});
    sqlx::query("insert into runtime_events(id,flow_run_id,node_run_id,sequence,event_type,layer,source,trust_level,payload,raw_json_payloads,visibility,durability) select $1,id,$2,(select coalesce(max(sequence),0)+1 from runtime_events where flow_run_id=$5),'client_protocol_trajectory','runtime_item','host','host_fact',$3,$4,'internal','durable' from flow_runs where id=$5")
        .bind(id).bind(input.node_run_id).bind(projection).bind(originals).bind(input.flow_run_id)
        .execute(store.pool()).await.unwrap();
    id
}

#[tokio::test]
async fn client_semantic_history_preserves_anchors_originals_cursors_and_retained_nul_progress() {
    let (store, run) = seed().await;
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    let mut historical = step(run, None, request, request, "emitted", "tool_call");
    historical.name = "legacy\0name".into();
    let input = AppendClientTrajectoryInput {
        flow_run_id: run,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact: ClientTrajectoryFact::Step {
            step: Box::new(historical),
        },
    };
    let mut projection = serde_json::to_value(&input).unwrap();
    projection["fact"]["step"]["name"] = json!("legacy safe query projection");
    let step_anchor = legacy_event(&store, &input, projection).await;
    assert_eq!(
        store
            .client_trajectory_page(run, None, None, 10)
            .await
            .unwrap()
            .items[0]
            .name,
        "legacy\0name"
    );
    let mut anchors = Vec::new();
    for (name, body) in [
        (
            "overview",
            json!({"arguments":"exact string", "other":"ordinary"}),
        ),
        ("parameters", json!("exact string")),
        ("result", json!({"key\0":"nul original"})),
        ("timing", json!({"observed_at":AT})),
        ("schema", json!({"type":"object"})),
    ] {
        let input = AppendClientTrajectoryInput {
            flow_run_id: run,
            node_run_id: None,
            request_id: request,
            observed_at: AT.into(),
            fact: ClientTrajectoryFact::Section {
                step_id: request,
                section: name.into(),
                value: body.clone(),
            },
        };
        let mut projection = serde_json::to_value(&input).unwrap();
        if name == "result" {
            projection["fact"]["value"] = json!("safe query projection");
        }
        let id = legacy_event(&store, &input, projection).await;
        anchors.push((id, serde_json::to_value(input).unwrap(), body));
    }
    let before = store
        .client_trajectory_section(run, None, request, "parameters", None, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        value(&store, run, request, "result").await,
        json!({"key\0":"nul original"})
    );
    let mut touched = 0;
    loop {
        let n = store
            .migrate_client_trajectory_semantic_history(&[run], 1)
            .await
            .unwrap();
        if n == 0 {
            break;
        }
        touched += n;
    }
    assert_eq!(touched, 6);
    assert_eq!(
        store
            .migrate_client_trajectory_semantic_history(&[run], 2)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .client_trajectory_section(run, None, request, "parameters", None, 1)
            .await
            .unwrap()
            .unwrap()
            .items[0]
            .sequence,
        before.items[0].sequence
    );
    for (id, original, body) in anchors {
        let replayed:Value=sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where id=$1")
            .bind(id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(replayed, original);
        assert_eq!(
            value(
                &store,
                run,
                request,
                original["fact"]["section"].as_str().unwrap()
            )
            .await,
            body
        );
        let physical: Value = sqlx::query_scalar("select payload from runtime_events where id=$1")
            .bind(id)
            .fetch_one(store.pool())
            .await
            .unwrap();
        if original["fact"]["section"] == "result" {
            assert!(physical.get("_client_semantic_ref").is_none());
            let retained: bool = sqlx::query_scalar(
                "select semantic_legacy_retained from client_trajectory_sections where id=$1",
            )
            .bind(id)
            .fetch_one(store.pool())
            .await
            .unwrap();
            assert!(retained);
        } else {
            assert!(physical["fact"].get("value").is_none());
        }
    }
    let present: bool =
        sqlx::query_scalar("select exists(select 1 from runtime_events where id=$1)")
            .bind(step_anchor)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(present);
    assert_eq!(
        store
            .client_trajectory_page(run, None, None, 10)
            .await
            .unwrap()
            .items[0]
            .name,
        "legacy\0name"
    );
    let section = Uuid::now_v7();
    let wrong = AppendClientTrajectoryInput {
        flow_run_id: run,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact: ClientTrajectoryFact::Section {
            step_id: request,
            section: "usage".into(),
            value: json!({"count":1}),
        },
    };
    let id = legacy_event(&store, &wrong, serde_json::to_value(&wrong).unwrap()).await;
    begin(&store, run, None, section).await;
    sqlx::query("update client_trajectory_sections set request_id=$2 where id=$1")
        .bind(id)
        .bind(section)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .migrate_client_trajectory_semantic_history(&[run], 1)
        .await
        .is_err());
    let physical: Value = sqlx::query_scalar("select payload from runtime_events where id=$1")
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(physical, serde_json::to_value(wrong).unwrap());
}

#[tokio::test]
async fn client_semantic_official_upgrade_preserves_old_cursors_and_reserves_after_historical_sequence(
) {
    let (pool, run) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run_before(
        Some(20260929100000),
    )
    .await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    let integrity = AppendClientTrajectoryInput {
        flow_run_id: run,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact: ClientTrajectoryFact::Integrity {
            status: "complete".into(),
            dropped_count: 0,
            persist_failed_count: 0,
        },
    };
    legacy_event(
        &store,
        &integrity,
        serde_json::to_value(&integrity).unwrap(),
    )
    .await;
    let old = AppendClientTrajectoryInput {
        flow_run_id: run,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact: ClientTrajectoryFact::Step {
            step: Box::new(step(run, None, request, request, "emitted", "tool_call")),
        },
    };
    let anchor = legacy_event(&store, &old, serde_json::to_value(&old).unwrap()).await;
    // An existing event reservation above the directory must define the first new cursor.
    sqlx::query("update runtime_events set sequence=9000 where id=$1")
        .bind(anchor)
        .execute(store.pool())
        .await
        .unwrap();
    storage_durable_postgres::run_migrations(store.pool())
        .await
        .unwrap();
    let stable = store
        .client_trajectory_page(run, None, None, 10)
        .await
        .unwrap()
        .items[0]
        .sequence;
    assert_eq!(stable, 2);
    let high: i64 =
        sqlx::query_scalar("select runtime_event_sequence_high_water from flow_runs where id=$1")
            .bind(run)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(high, 9000);
    section(
        &store,
        run,
        None,
        request,
        request,
        "timing",
        json!({"observed_at":AT}),
    )
    .await;
    let timing = store
        .client_trajectory_section(run, None, request, "timing", None, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(timing.items[0].sequence, 9001);
    let batch = store
        .append_runtime_events(&[event(run), event(run)])
        .await
        .unwrap();
    assert_eq!(
        batch.iter().map(|event| event.sequence).collect::<Vec<_>>(),
        vec![9002, 9003]
    );
    assert_eq!(
        store
            .client_trajectory_page(run, None, None, 10)
            .await
            .unwrap()
            .items[0]
            .sequence,
        stable
    );
    let exists: bool =
        sqlx::query_scalar("select exists(select 1 from runtime_events where id=$1)")
            .bind(anchor)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(exists);
}

#[tokio::test]
async fn client_semantic_waiting_text_reads_new_results_and_refreshes_settled_projection() {
    let (store, run) = seed().await;
    sqlx::query("update flow_runs set status='waiting_callback' where id=$1")
        .bind(run)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("insert into application_run_log_summaries(flow_run_id,application_id,run_mode,status,title,input_payload,started_at,created_at,updated_at,scope_id) select id,application_id,run_mode,status,title,input_payload,started_at,created_at,updated_at,scope_id from flow_runs where id=$1")
        .bind(run).execute(store.pool()).await.unwrap();
    sqlx::query("select application_run_log_task_refresh($1)")
        .bind(run)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("update application_run_log_tasks set projection_settled_at=now() where id=$1")
        .bind(run)
        .execute(store.pool())
        .await
        .unwrap();
    let deadline: time::OffsetDateTime = sqlx::query_scalar(
        "select projection_deadline_at from application_run_log_tasks where id=$1",
    )
    .bind(run)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let request = Uuid::now_v7();
    begin(&store, run, None, request).await;
    append(
        &store,
        run,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(step(run, None, request, request, "emitted", "assistant")),
        },
    )
    .await;
    section(&store,run,None,request,request,"overview",json!({"content":[{"type":"output_text","text":"actual emitted"},{"type":"refusal","refusal":" response"}]})).await;
    section(&store,run,None,request,request,"result",json!([{"type":"output_text","text":"actual emitted"},{"type":"refusal","refusal":" response"}])).await;
    let output: String =
        sqlx::query_scalar("select projection_output from application_run_log_tasks where id=$1")
            .bind(run)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(output, "actual emitted response");
    let submitted = Uuid::now_v7();
    append(
        &store,
        run,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(step(
                run,
                None,
                request,
                submitted,
                "submitted",
                "assistant",
            )),
        },
    )
    .await;
    section(
        &store,
        run,
        None,
        request,
        submitted,
        "result",
        json!("submitted history"),
    )
    .await;
    let output: String =
        sqlx::query_scalar("select application_run_log_task_last_client_text(array[$1]::uuid[])")
            .bind(run)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(output, "actual emitted response");
    section(
        &store,
        run,
        None,
        request,
        request,
        "result",
        json!("text\0original"),
    )
    .await;
    let output: String =
        sqlx::query_scalar("select projection_output from application_run_log_tasks where id=$1")
            .bind(run)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(output, "text\\u0000original");
    let after: time::OffsetDateTime = sqlx::query_scalar(
        "select projection_deadline_at from application_run_log_tasks where id=$1",
    )
    .bind(run)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(after, deadline);
    let original = store
        .client_trajectory_section(run, None, request, "result", None, 32)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        original.items.last().unwrap().value,
        json!("text\0original")
    );
}
