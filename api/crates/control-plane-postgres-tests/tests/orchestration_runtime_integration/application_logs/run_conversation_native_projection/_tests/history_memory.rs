use super::projection_cost::event;
use super::*;
use sqlx::Connection;
use std::{fs::File, path::PathBuf, process::Command};

#[path = "history_memory_probe.rs"]
mod probe;
use probe::Probe;

const TEST: &str = "application_logs::run_conversation_native_projection::incremental::history_memory::history_memory_phase_probe";

fn output() -> PathBuf {
    PathBuf::from(std::env::var("HISTORY_MEMORY_OUT").expect("HISTORY_MEMORY_OUT required"))
}

fn setting(key: &str, default: usize) -> usize {
    std::env::var(key)
        .map(|v| v.parse().unwrap())
        .unwrap_or(default)
}

fn write_json(path: PathBuf, value: &Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

// The coordinator alone owns migrations and the schema/database cleanup guard.
// A exits before B starts. Neither measured process constructs migration data,
// and B never holds the history inputs or A's returned records.
async fn coordinate() {
    let _explicit_fixture_url =
        std::env::var("DATABASE_URL").expect("explicit owned fixture DB required");
    let root = output();
    std::fs::create_dir_all(&root).unwrap();
    let database = isolated_database().await;
    let url = database.database_url().to_owned();
    let database_name = database.database_name().to_owned();
    let schema_name = database.schema_name().to_owned();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let run = native_member(&store, &seeded, &compiled, key, 0, "history-memory", vec![]).await;
    write_json(
        root.join("fixture.json"),
        &json!({
            "flow_run_id":run.id,"application_id":run.application_id,"scope_id":seeded.workspace_id,
            "database":database_name,"schema":schema_name,
            "history":setting("HISTORY_MEMORY_N",1024),"item_bytes":setting("HISTORY_MEMORY_ITEM_BYTES",16384),
            "rounds":setting("HISTORY_MEMORY_ROUNDS",20),"delta":1,"concurrency":1,
            "migrations":"storage_durable_postgres::run_migrations; coordinator only",
        }),
    );
    let only_buffers = std::env::var("HISTORY_MEMORY_ONLY_BUFFERS").as_deref() == Ok("1");
    let only_seed = std::env::var("HISTORY_MEMORY_ONLY_SEED").as_deref() == Ok("1");
    let mut roles = if only_buffers {
        vec!["buffers"]
    } else if only_seed {
        vec!["seed"]
    } else {
        vec!["seed", "consume"]
    };
    if !only_buffers && std::env::var("HISTORY_MEMORY_BUFFER_CONTROL").as_deref() == Ok("1") {
        roles.push("buffers");
    }
    for role in roles {
        let log = File::create(root.join(format!("{role}.log"))).unwrap();
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
            .env("HISTORY_MEMORY_ROLE", role)
            .env("HISTORY_MEMORY_RUN", run.id.to_string())
            .env("HISTORY_MEMORY_APPLICATION", run.application_id.to_string())
            .env("HISTORY_MEMORY_SCOPE", seeded.workspace_id.to_string())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .status()
            .unwrap();
        write_json(
            root.join(format!("{role}-exit.json")),
            &json!({
                "role":role,"exit_code":status.code(),"success":status.success(),"wall_seconds":started.elapsed().as_secs_f64(),
            }),
        );
        assert!(status.success(), "{role} failed; preserve {role}.log");
    }
    store.pool().close().await;
    drop(store);
    write_json(
        root.join("coordinator-complete.json"),
        &json!({"children_succeeded":true,"database_guard_released":true}),
    );
}

async fn owned_store() -> PgControlPlaneStore {
    // One physical connection makes the close/shrink experiment unambiguous.
    // This is a fixture setting, not a production capacity policy.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    PgControlPlaneStore::new(pool)
}

async fn teardown(store: PgControlPlaneStore, trace: &mut Probe) {
    // Re-acquire the same owned connection. shrink_buffers is a SQLx public
    // diagnostic control, never applied to the protected service or candidate.
    trace.begin("buffer-shrink");
    let mut connection = store.pool().acquire().await.unwrap();
    connection.shrink_buffers();
    drop(connection);
    tokio::task::yield_now().await;
    trace.point("after-sqlx-shrink", true);
    trace.begin("pool-close");
    store.pool().close().await;
    drop(store);
    trace.point("after-pool-close", true);
    trace.begin("allocator-idle");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    trace.point("after-release-100ms", false);
    tokio::time::sleep(std::time::Duration::from_secs(12)).await;
    trace.point("after-release-12s", true);
}

async fn seed() {
    let mut trace = Probe::new(output().join("seed"));
    trace.point("process-ready", true);
    trace.canary();
    trace.begin("connect");
    let store = owned_store().await;
    trace.point("pool-ready", true);
    let run: Uuid = std::env::var("HISTORY_MEMORY_RUN")
        .unwrap()
        .parse()
        .unwrap();
    let history = setting("HISTORY_MEMORY_N", 1024);
    let bytes = setting("HISTORY_MEMORY_ITEM_BYTES", 16384);
    trace.begin("input-materialization");
    let inputs = (0..history)
        .map(|i| event(run, format!("history-{i}"), "h".repeat(bytes)))
        .collect::<Vec<_>>();
    trace.point("inputs-ready", true);
    trace.begin("seed-append");
    let started = std::time::Instant::now();
    let records = store.append_runtime_events(&inputs).await.unwrap();
    let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    trace.point("seed-append-returned", true);
    // The semantic oracle is explicitly outside the append phase.
    trace.begin("seed-return-oracle");
    assert_eq!(records.len(), inputs.len());
    for (input, record) in inputs.iter().zip(&records) {
        assert_eq!(record.payload, input.payload);
        assert_eq!(record.flow_run_id, input.flow_run_id);
        assert_eq!(record.event_type, input.event_type);
    }
    for pair in records.windows(2) {
        assert!(pair[0].sequence < pair[1].sequence);
        assert_ne!(pair[0].id, pair[1].id);
    }
    trace.point("seed-return-oracle-complete", false);
    trace.begin("drop-records");
    drop(records);
    trace.point("seed-records-dropped", true);
    trace.begin("drop-inputs");
    drop(inputs);
    trace.point("seed-inputs-dropped", true);
    // Inspect the connection before verification reads can grow its buffers.
    teardown(store, &mut trace).await;
    write_json(
        output().join("seed-timing.json"),
        &json!({"append_wall_ms":wall_ms,"timing_scope":"instrumented diagnostic, not regression latency acceptance"}),
    );
}

async fn consume() {
    let mut trace = Probe::new(output().join("consume"));
    trace.point("process-ready", true);
    trace.canary();
    trace.begin("connect");
    let store = owned_store().await;
    trace.point("pool-ready", true);
    let run: Uuid = std::env::var("HISTORY_MEMORY_RUN")
        .unwrap()
        .parse()
        .unwrap();
    let bytes = setting("HISTORY_MEMORY_ITEM_BYTES", 16384);
    let rounds = setting("HISTORY_MEMORY_ROUNDS", 20);
    let mut timings = Vec::with_capacity(rounds);
    for round in 0..rounds {
        trace.begin(&format!("delta-{round}-input"));
        let inputs = [event(run, format!("delta-{round}"), "d".repeat(bytes))];
        trace.point("delta-input-ready", round == 0);
        trace.begin(&format!("delta-{round}-append"));
        let started = std::time::Instant::now();
        let records = store.append_runtime_events(&inputs).await.unwrap();
        timings.push(started.elapsed().as_secs_f64() * 1000.0);
        trace.point("delta-append-returned", round == 0 || round + 1 == rounds);
        trace.begin(&format!("delta-{round}-return-oracle"));
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].payload, inputs[0].payload);
        assert_eq!(records[0].flow_run_id, run);
        drop(records);
        drop(inputs);
        trace.point(
            "delta-input-and-records-dropped",
            round == 0 || round + 1 == rounds,
        );
    }
    trace.begin("consume-before-assertions");
    trace.point("pool-platform-before-verification", true);
    // Teardown this measured connection before the full-body oracle. The oracle
    // below uses a different connection so it cannot inflate append stage peaks.
    teardown(store, &mut trace).await;
    trace.begin("full-history-oracle-connect");
    let store = owned_store().await;
    let app: Uuid = std::env::var("HISTORY_MEMORY_APPLICATION")
        .unwrap()
        .parse()
        .unwrap();
    let scope: Uuid = std::env::var("HISTORY_MEMORY_SCOPE")
        .unwrap()
        .parse()
        .unwrap();
    trace.begin("full-history-oracle");
    let originals: Vec<(Uuid, i64, Value)> = sqlx::query_as("select id,sequence,runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where flow_run_id=$1 order by sequence")
        .bind(run).fetch_all(store.pool()).await.unwrap();
    let history = setting("HISTORY_MEMORY_N", 1024);
    assert_eq!(originals.len(), history + rounds);
    for (index, (_, _, payload)) in originals.iter().enumerate() {
        let (id, body) = if index < history {
            (format!("history-{index}"), "h".repeat(bytes))
        } else {
            (format!("delta-{}", index - history), "d".repeat(bytes))
        };
        assert_eq!(
            *payload,
            json!({"item":{"type":"message","id":id,"content":body}})
        );
    }
    for pair in originals.windows(2) {
        assert!(pair[0].1 < pair[1].1);
        assert_ne!(pair[0].0, pair[1].0);
    }
    trace.point("full-persisted-originals-held", true);
    let messages = store
        .read_native_agent_log_messages(scope, app, run)
        .await
        .unwrap();
    let outputs = messages
        .iter()
        .filter(|v| {
            v["native_message"]["_source_item"]["id"]
                .as_str()
                .is_some_and(|s| s.starts_with("history-") || s.starts_with("delta-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), originals.len());
    for (message, (_, sequence, payload)) in outputs.iter().zip(&originals) {
        let native = &message["native_message"];
        assert_eq!(native["_source_item"], payload["item"]);
        assert_eq!(native["content"], payload["item"]["content"]);
        assert_eq!(native["_log_conflicting"], false);
        assert!(
            native["_log_source_revision"]
                .as_str()
                .unwrap()
                .starts_with("v7:")
        );
        assert_eq!(native["_log_source_sequence"], *sequence);
    }
    trace.point("full-ui-projection-and-originals-held", true);
    drop(outputs);
    drop(messages);
    drop(originals);
    trace.point("full-oracle-bodies-dropped", true);
    teardown(store, &mut trace).await;
    write_json(
        output().join("consume-timing.json"),
        &json!({"samples_ms":timings,"updates":rounds,"timing_scope":"instrumented diagnostic; setup and full-history oracle excluded"}),
    );
}

// A single 16 MiB value isolates SQLx write and read retention without history
// folding, migrations or another archive copy. Not a production query change.
async fn buffers() {
    const BYTES: usize = 16 * 1024 * 1024;
    let mut trace = Probe::new(output().join("buffers"));
    trace.canary();
    trace.begin("buffer-control-connect");
    let store = owned_store().await;
    let mut connection = store.pool().acquire().await.unwrap();
    trace.point("buffer-control-ready", true);
    trace.begin("write-only-input");
    let input = "w".repeat(BYTES);
    trace.point("write-input-held", true);
    trace.begin("write-only-query");
    let length: i32 = sqlx::query_scalar("select length($1::text)")
        .bind(&input)
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert_eq!(length as usize, BYTES);
    trace.point("write-query-returned", true);
    drop(input);
    trace.point("write-input-dropped", true);
    connection.shrink_buffers();
    trace.point("write-buffer-shrunk", true);
    trace.begin("read-only-query");
    let row = sqlx::query("select repeat('r', $1)")
        .bind(BYTES as i32)
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    trace.point("read-row-held", true);
    use sqlx::Row;
    let body: &str = row.get(0);
    assert_eq!(body.len(), BYTES);
    assert!(body.as_bytes().iter().all(|v| *v == b'r'));
    drop(row);
    trace.point("read-row-dropped", true);
    connection.shrink_buffers();
    trace.point("read-buffer-shrunk", true);
    // fetch_one can leave protocol completion bytes sharing the large backing
    // allocation. Distinguish draining that response from shrinking spare space.
    connection.ping().await.unwrap();
    trace.point("read-response-drained", true);
    connection.shrink_buffers();
    trace.point("read-drained-buffer-shrunk", true);
    drop(connection);
    teardown(store, &mut trace).await;
}

#[tokio::test]
#[ignore = "owned seed/fresh-consumer memory diagnosis; HISTORY_MEMORY_OUT and explicit DATABASE_URL required"]
async fn history_memory_phase_probe() {
    match std::env::var("HISTORY_MEMORY_ROLE").as_deref() {
        Ok("seed") => seed().await,
        Ok("consume") => consume().await,
        Ok("buffers") => buffers().await,
        Err(_) => coordinate().await,
        other => panic!("unknown probe role {other:?}"),
    }
}
