//! Orthogonal retained-context control. The coordinator creates all large data;
//! measured consumers read only run UUIDs, never get_flow_run before append.
use super::*;

#[path = "history_memory_query.rs"]
mod query;

const BODY_BYTES: usize = 16 * 1024 * 1024;

async fn state(store: &PgControlPlaneStore, run: Uuid) -> Value {
    // Only scalar sizes and progress cross the wire. DB JSONB materialization
    // for these sizes is distinct from application allocation.
    sqlx::query_scalar(r#"select jsonb_build_object(
        'output_event_count',(select count(*) from runtime_events e where e.flow_run_id=f.id and e.event_type='provider_output_item_done'),
        'result_count',p.result_count,'output_sequence',p.output_sequence,
        'source_revision',p.source_revision,'run_revision',f.message_projection_revision,
        'invalidated',p.invalidated,'projection_version',p.projection_version,
        'last_read_context_count',p.last_read_context_count,'last_read_event_count',p.last_read_event_count,
        'has_context_original',f.raw_json_payloads ? 'log_context',
        'log_context_jsonb_text_bytes',octet_length(f.log_context::text),
        'sidecar_jsonb_text_bytes',octet_length(f.raw_json_payloads::text),
        'current_input_jsonb_text_bytes',octet_length(f.input_payload::text))
        from flow_runs f join application_run_native_projection_progress p on p.flow_run_id=f.id where f.id=$1"#)
        .bind(run).fetch_one(store.pool()).await.unwrap()
}

fn assert_progress(value: &Value, count: usize) {
    assert_eq!(value["output_event_count"], count);
    assert_eq!(value["result_count"], 1);
    assert_eq!(value["source_revision"], value["run_revision"]);
    assert_eq!(value["invalidated"], false);
    assert_eq!(value["projection_version"], 7);
}

pub(super) async fn coordinate() {
    std::env::var("DATABASE_URL").expect("explicit owned fixture URL required");
    let root = output();
    std::fs::create_dir_all(&root).unwrap();
    let database = isolated_database().await;
    let url = database.database_url().to_owned();
    let database_name = database.database_name().to_owned();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    query::query_contract();
    query::alias_sentinel(&store).await;
    let mut fixtures = Vec::new();
    let only_case = std::env::var("HISTORY_MEMORY_CONTEXT_CASE_ONLY").ok();
    for (label, bytes, sidecar, concurrency) in [
        ("small", 1024, false, 1),
        ("large", BODY_BYTES, false, 1),
        ("large-c2", BODY_BYTES, false, 2),
        ("large-sidecar", BODY_BYTES, true, 1),
    ] {
        if only_case
            .as_deref()
            .is_some_and(|selected| selected != label)
        {
            continue;
        }
        let folder = root.join(label);
        std::fs::create_dir_all(&folder).unwrap();
        let mut runs = Vec::new();
        let mut states = Vec::new();
        for index in 0..concurrency {
            // Independent apps/run IDs avoid artificial shared-run/app locking.
            let seeded = seed_runtime_base_with_workspace_name(
                &store,
                &format!("History context {label} {index}"),
            )
            .await;
            let compiled = seed_compiled_plan(&store, &seeded).await;
            let key = seed_application_api_key(&store, &seeded).await;
            let mut body = "c".repeat(bytes);
            if sidecar {
                body.replace_range(bytes - 1.., "\0");
            }
            let run = native_member(&store, &seeded, &compiled, key, 0,
                &format!("history-context-{label}-{index}"),
                vec![json!({"type":"function_call_output","call_id":"retained-result","output":body})]).await;
            let records = store
                .append_runtime_events(&[event(run.id, "stored-output".into(), "o".repeat(16384))])
                .await
                .unwrap();
            assert_eq!(records.len(), 1);
            drop(records);
            let before = state(&store, run.id).await;
            assert_progress(&before, 1);
            assert_eq!(before["has_context_original"], sidecar);
            states.push(before);
            runs.push(run.id);
        }
        // C1 has one UUID; C2 has only two independent UUIDs, no body/record data.
        write_json(
            folder.join("run-id.json"),
            &if concurrency == 1 {
                json!(runs[0])
            } else {
                json!(runs)
            },
        );
        fixtures.push(json!({"case":label,"tool_result_utf8_bytes_per_run":bytes,
            "database":database_name,"concurrency":concurrency,"before":states}));
        write_json(
            root.join("fixtures-in-progress.json"),
            &json!({"cases":fixtures}),
        );
        let log = File::create(folder.join("consumer.log")).unwrap();
        let started = std::time::Instant::now();
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                TEST,
                "--nocapture",
                "--test-threads=1",
            ])
            .env("DATABASE_URL", &url)
            .env("HISTORY_MEMORY_OUT", &folder)
            .env("HISTORY_MEMORY_ROLE", "context-consume")
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .status()
            .unwrap();
        write_json(
            folder.join("exit.json"),
            &json!({"exit_code":status.code(),
            "success":status.success(),"wall_seconds":started.elapsed().as_secs_f64()}),
        );
        assert!(
            status.success(),
            "{label} context consumer failed; preserve consumer.log"
        );
    }
    assert!(
        !fixtures.is_empty(),
        "context case filter selected no fixture"
    );
    write_json(
        root.join("fixtures.json"),
        &json!({"cases":fixtures,
        "stored_outputs_before_each_consumer_per_run":1,"delta_outputs_per_run":1,
        "delta_item_bytes":16384,"current_input":"same small native_member input; SQL sizes recorded",
        "sidecar_case_scope":"full 16MiB NUL lossless context; existing full repair, separate from plain incremental pair"}),
    );
    store.pool().close().await;
    drop(store);
    write_json(
        root.join("complete.json"),
        &json!({"consumers_succeeded":true,"database_guard_released":true}),
    );
}

async fn close(store: PgControlPlaneStore, trace: &mut Probe) {
    trace.begin("context-pool-close");
    store.pool().close().await;
    drop(store);
    trace.point("context-pool-closed", true);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    trace.point("context-release-100ms", false);
}

pub(super) async fn consume() {
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(output().join("run-id.json")).unwrap()).unwrap();
    let runs: Vec<Uuid> = if manifest.is_array() {
        serde_json::from_value(manifest).unwrap()
    } else {
        vec![serde_json::from_value(manifest).unwrap()]
    };
    assert!(matches!(runs.len(), 1 | 2));
    if runs.len() == 2 {
        assert_ne!(runs[0], runs[1]);
    }
    let mut trace = Probe::new(output().join("consumer"));
    trace.begin("context-consumer-connect");
    // C2 must have two ready physical connections before either append starts.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(runs.len() as u32)
        .min_connections(runs.len() as u32)
        .connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(pool);
    let mut connections = Vec::new();
    let mut backend_pids = Vec::new();
    for _ in &runs {
        let mut connection = store.pool().acquire().await.unwrap();
        let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        backend_pids.push(pid);
        connections.push(connection);
    }
    if runs.len() == 2 {
        assert_ne!(backend_pids[0], backend_pids[1]);
    }
    drop(connections);
    let mut before = Vec::new();
    for run in &runs {
        let value = state(&store, *run).await;
        assert_progress(&value, 1);
        before.push(value);
    }
    trace.point("context-consumer-ready-no-body-loaded", true);
    let inputs = runs
        .iter()
        .map(|run| [event(*run, "orthogonal-delta".into(), "d".repeat(16384))])
        .collect::<Vec<_>>();
    trace.begin("context-real-append");
    let started = std::time::Instant::now();
    let append = |index: usize| {
        let store = &store;
        let input = &inputs[index];
        async move {
            let start_us = started.elapsed().as_micros();
            let records = store.append_runtime_events(input).await.unwrap();
            let end_us = started.elapsed().as_micros();
            (records, start_us, end_us)
        }
    };
    let returned = if runs.len() == 1 {
        vec![append(0).await]
    } else {
        let (left, right) = tokio::join!(append(0), append(1));
        vec![left, right]
    };
    let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    trace.point("context-real-append-returned", true);
    trace.begin("context-return-oracle");
    let mut intervals = Vec::new();
    for (index, (records, start, end)) in returned.iter().enumerate() {
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].payload, inputs[index][0].payload);
        assert_eq!(records[0].flow_run_id, runs[index]);
        intervals.push(json!({"run_id":runs[index],"start_us":start,"end_us":end,
            "wall_ms":(*end-*start) as f64/1000.0}));
    }
    if returned.len() == 2 {
        assert!(
            returned[0].1 < returned[1].2 && returned[1].1 < returned[0].2,
            "C2 append intervals must overlap in one process"
        );
    }
    write_json(
        output().join("append-timing.json"),
        &json!({"concurrency":runs.len(),
        "pid":std::process::id(),"backend_pids_preflight":backend_pids,"intervals":intervals,
        "combined_wall_ms":wall_ms,"scope":"real append only; stages record exact requested peak and endpoint RSS; OS HWM is lifetime only",
        "performance":trace.is_performance()}),
    );
    drop(returned);
    drop(inputs);
    let mut after = Vec::new();
    for run in &runs {
        let value = state(&store, *run).await;
        assert_progress(&value, 2);
        after.push(value);
    }
    write_json(
        output().join("progress.json"),
        &json!({"before":before,"after":after}),
    );
    trace.point("context-real-append-bodies-dropped", true);
    close(store, &mut trace).await;
    if trace.is_performance() {
        return;
    }
    // Dual/triple-row correctness materialization belongs only to proof/stack.
    trace.begin("context-row-control-connect");
    let store = owned_store().await;
    query::alias_sentinel(&store).await;
    query::row_proof(&store, runs[0], &mut trace).await;
    close(store, &mut trace).await;
}
