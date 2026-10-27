use super::*;

pub(super) fn event(run: Uuid, id: String, text: String) -> AppendRuntimeEventInput {
    AppendRuntimeEventInput {
        flow_run_id: run,
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
        payload: json!({"item":{"type":"message","id":id,"content":text}}),
    }
}

pub(super) async fn setup() -> (PgControlPlaneStore, domain::FlowRunRecord) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let run = native_member(
        &store,
        &seeded,
        &compiled,
        key,
        0,
        "projection-cost",
        vec![],
    )
    .await;
    (store, run)
}

// Statement transition tables count real affected tuples without per-row test
// callbacks. Both comparison binaries install exactly the same instrumentation.
pub(super) async fn install_counters(store: &PgControlPlaneStore) {
    sqlx::raw_sql(r#"
        create table projection_cost_counter(relation text, operation text, statements bigint, tuples bigint,
            primary key(relation,operation));
        create function projection_cost_count() returns trigger language plpgsql as $$
        declare affected bigint;
        begin
            select count(*) into affected from affected_rows;
            insert into projection_cost_counter values(tg_table_name,tg_op,1,affected)
            on conflict(relation,operation) do update set
                statements=projection_cost_counter.statements+1,
                tuples=projection_cost_counter.tuples+excluded.tuples;
            return null;
        end $$;
    "#).execute(store.pool()).await.unwrap();
    for table in [
        "application_run_conversation_message_items",
        "application_run_native_projection_progress",
        "application_run_log_summaries",
        "application_run_log_tasks",
        "flow_runs",
        "runtime_events",
    ] {
        for (operation, reference) in [("insert", "new"), ("update", "new"), ("delete", "old")] {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "create trigger cost_{operation} after {operation} on {table} referencing {reference} table as affected_rows for each statement execute function projection_cost_count()"
            ))).execute(store.pool()).await.unwrap();
        }
    }
}

pub(super) async fn counters(store: &PgControlPlaneStore) -> Value {
    sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(c) order by relation,operation),'[]'::jsonb) from projection_cost_counter c")
        .fetch_one(store.pool()).await.unwrap()
}

// Attribute actual task upsert tuples to their task, including partial sharing.
// Install separately so the existing append benchmark's instrumentation is stable.
pub(super) async fn install_task_counters(store: &PgControlPlaneStore) {
    sqlx::raw_sql(r#"
        create table projection_task_cost_counter(task_run_id uuid primary key, writes bigint not null);
        create function projection_task_cost_count() returns trigger language plpgsql as $$
        begin
            insert into projection_task_cost_counter
                select id,count(*) from affected_rows group by id
            on conflict(task_run_id) do update set
                writes=projection_task_cost_counter.writes+excluded.writes;
            return null;
        end $$;
        create trigger task_cost_insert after insert on application_run_log_tasks
            referencing new table as affected_rows for each statement execute function projection_task_cost_count();
        create trigger task_cost_update after update on application_run_log_tasks
            referencing new table as affected_rows for each statement execute function projection_task_cost_count();
    "#).execute(store.pool()).await.unwrap();
}

pub(super) async fn task_writes(store: &PgControlPlaneStore, task: Uuid) -> i64 {
    sqlx::query_scalar(
        "select coalesce((select writes from projection_task_cost_counter where task_run_id=$1),0)",
    )
    .bind(task)
    .fetch_one(store.pool())
    .await
    .unwrap()
}

fn dml_count(writes: &Value, relation: &str, operation: &str, metric: &str) -> i64 {
    writes
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["relation"] == relation && row["operation"] == operation)
        .map(|row| row[metric].as_i64().unwrap())
        .unwrap_or(0)
}

#[tokio::test]
async fn full_native_repair_refreshes_current_task_once() {
    let (store, run) = setup().await;
    store
        .append_runtime_events(&[
            event(run.id, "retained-0".into(), "first retained body".into()),
            event(run.id, "retained-1".into(), "second retained body".into()),
        ])
        .await
        .unwrap();
    let retained: i64 = sqlx::query_scalar(
        "select count(*) from application_run_conversation_message_items where flow_run_id=$1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(retained >= 3);
    // An invalidated progress row forces the production full-repair branch;
    // this measurement must not accidentally exercise the existing append path.
    sqlx::query("update application_run_native_projection_progress set invalidated=true where flow_run_id=$1")
        .bind(run.id).execute(store.pool()).await.unwrap();
    install_counters(&store).await;
    install_task_counters(&store).await;
    store
        .append_runtime_event(&event(run.id, "new".into(), "new body".into()))
        .await
        .unwrap();
    let writes = counters(&store).await;
    assert_eq!(
        dml_count(
            &writes,
            "application_run_conversation_message_items",
            "DELETE",
            "tuples"
        ),
        retained
    );
    assert_eq!(
        dml_count(
            &writes,
            "application_run_conversation_message_items",
            "INSERT",
            "tuples"
        ),
        retained + 1
    );
    // Existing task: one SQL upsert issues one INSERT statement and updates one
    // tuple. The pre-trim full repair issues two of each, so it fails this oracle.
    assert_eq!(
        dml_count(&writes, "application_run_log_tasks", "INSERT", "statements"),
        1
    );
    assert_eq!(
        dml_count(&writes, "application_run_log_tasks", "UPDATE", "tuples"),
        1
    );
    assert_eq!(task_writes(&store, run.id).await, 1);
    // Both summary upserts are still required, including the fresh inner read.
    assert_eq!(
        dml_count(
            &writes,
            "application_run_log_summaries",
            "INSERT",
            "statements"
        ),
        2
    );
    let current: bool = sqlx::query_scalar("select not p.invalidated and p.source_revision=f.message_projection_revision from application_run_native_projection_progress p join flow_runs f on f.id=p.flow_run_id where f.id=$1")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert!(current);
}

async fn storage(store: &PgControlPlaneStore) -> Value {
    sqlx::query_scalar(r#"select jsonb_build_object(
        'message_logical_bytes',coalesce((select sum(pg_column_size(m)) from application_run_conversation_message_items m),0),
        'message_physical_bytes',pg_total_relation_size('application_run_conversation_message_items'),
        'events_physical_bytes',pg_total_relation_size('runtime_events'),
        'database_physical_bytes',pg_database_size(current_database()))"#)
        .fetch_one(store.pool()).await.unwrap()
}

fn process_cpu_ticks() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let fields: Vec<_> = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .collect();
    fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap()
}

fn postgres_cluster_cpu_usec() -> Option<u64> {
    let path = std::env::var("NATIVE_PROJECTION_BENCH_PG_CPU_STAT").ok()?;
    let stat = std::fs::read_to_string(path).unwrap();
    stat.lines()
        .find_map(|line| line.strip_prefix("usage_usec "))
        .map(|value| value.parse().unwrap())
}

async fn query_statistics(store: &PgControlPlaneStore) -> Option<Value> {
    if std::env::var("NATIVE_PROJECTION_BENCH_STATEMENTS").as_deref() != Ok("1") {
        return None;
    }
    Some(sqlx::query_scalar(r#"select jsonb_build_object(
        'top_level_calls',coalesce(sum(calls) filter(where toplevel),0),
        'top_level_select_calls',coalesce(sum(calls) filter(where toplevel and query ~* '^\s*(select|with)'),0),
        'nested_calls',coalesce(sum(calls) filter(where not toplevel),0),
        'top_level_exec_ms',coalesce(sum(total_exec_time) filter(where toplevel),0),
        'top_level_wal_bytes',coalesce(sum(wal_bytes) filter(where toplevel),0))
        from pg_stat_statements where dbid=(select oid from pg_database where datname=current_database())"#)
        .fetch_one(store.pool()).await.unwrap())
}

fn process_memory() -> Value {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let mut out = serde_json::Map::new();
    for field in ["VmRSS", "VmHWM"] {
        let value = status
            .lines()
            .find(|line| line.starts_with(&format!("{field}:")))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse::<u64>()
            .unwrap();
        out.insert(format!("{field}_KiB"), json!(value));
    }
    Value::Object(out)
}

fn setting(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .map(|value| value.parse().unwrap())
        .unwrap_or(default)
}

// Opt-in resource experiment, never a PR-latency measurement. The measured
// boundary is the production append_runtime_events transaction, not a fold copy.
#[tokio::test]
#[ignore = "controlled resource experiment; set NATIVE_PROJECTION_BENCH_OUT"]
async fn native_projection_cost_benchmark() {
    let output = std::env::var("NATIVE_PROJECTION_BENCH_OUT").unwrap();
    let history = setting("NATIVE_PROJECTION_BENCH_HISTORY", 128);
    let delta = setting("NATIVE_PROJECTION_BENCH_DELTA", 1);
    let rounds = setting("NATIVE_PROJECTION_BENCH_ROUNDS", 20);
    let concurrency = setting("NATIVE_PROJECTION_BENCH_CONCURRENCY", 1);
    let bytes = setting("NATIVE_PROJECTION_BENCH_ITEM_BYTES", 8192);
    assert!((1..=2).contains(&concurrency));
    let (store, first) = setup().await;
    let mut runs = vec![first];
    for index in 1..concurrency {
        // Independent conversations; concurrency does not turn a serialized
        // conversation lock into an apparent throughput regression.
        let seeded =
            seed_runtime_base_with_workspace_name(&store, &format!("Projection bench {index}"))
                .await;
        let compiled = seed_compiled_plan(&store, &seeded).await;
        let key = seed_application_api_key(&store, &seeded).await;
        runs.push(
            native_member(
                &store,
                &seeded,
                &compiled,
                key,
                index,
                "projection-cost-other",
                vec![],
            )
            .await,
        );
    }
    for run in &runs {
        let inputs = (0..history)
            .map(|index| event(run.id, format!("history-{index}"), "h".repeat(bytes)))
            .collect::<Vec<_>>();
        if !inputs.is_empty() {
            store.append_runtime_events(&inputs).await.unwrap();
        }
    }
    install_counters(&store).await;
    if std::env::var("NATIVE_PROJECTION_BENCH_STATEMENTS").as_deref() == Ok("1") {
        sqlx::query("create extension if not exists pg_stat_statements")
            .execute(store.pool())
            .await
            .unwrap();
    }
    let before_storage = storage(&store).await;
    let before_memory = process_memory();
    if std::env::var("NATIVE_PROJECTION_BENCH_STATEMENTS").as_deref() == Ok("1") {
        sqlx::query("select pg_stat_statements_reset(0,(select oid from pg_database where datname=current_database()),0)")
            .execute(store.pool()).await.unwrap();
    }
    let before_postgres_cpu = postgres_cluster_cpu_usec();
    let before_cpu = process_cpu_ticks();
    let started = std::time::Instant::now();
    let mut samples_ms = Vec::new();
    for round in 0..rounds {
        let mut operations = Vec::new();
        for run in &runs {
            let inputs = (0..delta)
                .map(|index| event(run.id, format!("delta-{round}-{index}"), "d".repeat(bytes)))
                .collect::<Vec<_>>();
            let store = store.clone();
            operations.push(tokio::spawn(async move {
                let started = std::time::Instant::now();
                store.append_runtime_events(&inputs).await.unwrap();
                started.elapsed().as_secs_f64() * 1000.0
            }));
        }
        for operation in operations {
            samples_ms.push(operation.await.unwrap());
        }
    }
    let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    let cpu_ticks = process_cpu_ticks() - before_cpu;
    let after_postgres_cpu = postgres_cluster_cpu_usec();
    let after_memory = process_memory();
    let query_statistics = query_statistics(&store).await;
    let raw_counters = counters(&store).await;
    let after_storage = storage(&store).await;
    for run in &runs {
        let count: i64 = sqlx::query_scalar("select count(*) from application_run_conversation_message_items where flow_run_id=$1 and source_occurrence_sequence is not null")
            .bind(run.id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(count as usize, history + rounds * delta);
        let items: Vec<String> = sqlx::query_scalar("select native_message#>>'{_source_item,id}' from application_run_conversation_message_items where flow_run_id=$1 and source_occurrence_sequence is not null order by display_sequence")
            .bind(run.id).fetch_all(store.pool()).await.unwrap();
        assert_eq!(
            items.first().unwrap(),
            if history > 0 {
                "history-0"
            } else {
                "delta-0-0"
            }
        );
        assert_eq!(
            items.last().unwrap(),
            &format!("delta-{}-{}", rounds - 1, delta - 1)
        );
        let expected_ids =
            (0..history)
                .map(|index| format!("history-{index}"))
                .chain((0..rounds).flat_map(|round| {
                    (0..delta).map(move |index| format!("delta-{round}-{index}"))
                }))
                .collect::<Vec<_>>();
        assert_eq!(
            items, expected_ids,
            "every displayed source identity must retain fixture order"
        );
        let originals: Vec<Value>=sqlx::query_scalar("select runtime_original_json(native_message,raw_json_payloads,'native_message') from application_run_conversation_message_items where flow_run_id=$1 and source_occurrence_sequence is not null order by display_sequence")
            .bind(run.id).fetch_all(store.pool()).await.unwrap();
        for (index, body) in originals.iter().enumerate() {
            assert_eq!(
                body["_source_item"],
                json!({"type":"message","id":items[index],"content":if index<history {"h".repeat(bytes)} else {"d".repeat(bytes)}})
            );
            assert_eq!(body["_log_conflicting"], false);
        }
    }
    let evidence = json!({"history_N_per_run":history,"delta_per_update":delta,"rounds":rounds,"concurrency":concurrency,
        "item_bytes":bytes,"updates":rounds*concurrency,"cpu_ticks":cpu_ticks,"clock_ticks_per_second":std::process::Command::new("getconf").arg("CLK_TCK").output().map(|o|String::from_utf8_lossy(&o.stdout).trim().to_owned()).unwrap(),
        "wall_ms":wall_ms,"samples_ms":samples_ms,"memory_before":before_memory,"memory_after":after_memory,
        "storage_before":before_storage,"storage_after":after_storage,"dml_counters":raw_counters,
        "query_statistics":query_statistics,"postgres_cluster_cpu_usec_before":before_postgres_cpu,"postgres_cluster_cpu_usec_after":after_postgres_cpu,
        "rss_scope":"process peak includes identical history setup; not an update-only peak",
        "query_scope":"optional pg_stat_statements top-level and nested calls, scoped to this owned test database; DML counters report actual tuples",
        "instrumentation":"test-only statement transition counters, identical for both binaries"});
    std::fs::write(output, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
}
