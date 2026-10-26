//! Opt-in, C1 causal baseline on an immutable LOCAL export, never the source DB.
//! The coordinator owns migrations/import; only a fresh child measures a writer.
//! Bodies and database errors stay in restricted files, including assertion failures.
use super::*;
use sqlx::ConnectOptions;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    str::FromStr,
};

fn private_dir() -> PathBuf {
    let path = output().join("private");
    fs::create_dir_all(&path).expect("fixture private directory");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("private permissions");
    path
}

fn private_write(name: &str, value: &Value) {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(private_dir().join(name))
        .expect("private artifact");
    serde_json::to_writer(&mut file, value).expect("private JSON serialization");
    file.flush().expect("private artifact flush");
}

// Never let an anyhow/SQLx/JSON error containing a real value reach libtest logs.
fn safe<T, E: Debug>(result: Result<T, E>, field: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            private_write(
                "failure.json",
                &json!({"field":field,"error":format!("{error:?}")}),
            );
            panic!("initialization fixture failed at {field}; details are private");
        }
    }
}

fn exact(actual: &Value, expected: &Value, field: &str) {
    // Value equality, not a digest oracle; panic never formats either operand.
    if actual != expected {
        private_write(
            "difference.json",
            &json!({"field":field,"actual":actual,"expected":expected}),
        );
        panic!("initialization exact comparison failed at {field}; diff is private");
    }
}

fn load(path: impl AsRef<Path>) -> Value {
    let file = safe(File::open(path), "local artifact open");
    safe(
        serde_json::from_reader(BufReader::new(file)),
        "local artifact JSON",
    )
}

fn uuid(value: &Value) -> Uuid {
    safe(
        Uuid::parse_str(value.as_str().expect("UUID field type")),
        "UUID field",
    )
}

fn original(row: &Value, field: &str) -> Value {
    row[field]
        .as_str()
        .map(|text| safe(serde_json::from_str(text), "original JSON text"))
        .unwrap_or(Value::Null)
}

fn timestamp(value: &Value) -> OffsetDateTime {
    safe(
        OffsetDateTime::parse(
            value.as_str().expect("timestamp field type"),
            &time::format_description::well_known::Rfc3339,
        ),
        "timestamp field",
    )
}

struct Corpus {
    root: PathBuf,
    alias: String,
    entry: String,
    anchor: Uuid,
    members: BTreeSet<Uuid>,
    artifacts: Vec<Value>,
}

impl Corpus {
    fn open() -> Self {
        let root = PathBuf::from(std::env::var("HISTORY_INIT_CORPUS").expect("local corpus path"));
        let alias = std::env::var("HISTORY_INIT_SAMPLE").expect("sample alias");
        let entry = std::env::var("HISTORY_INIT_ENTRY").expect("entry");
        assert!(matches!(
            (alias.as_str(), entry.as_str()),
            ("S03", "create" | "repair") | ("S02", "repair")
        ));
        let manifest = load(root.join("private/sample-manifest.private.json"));
        let sample = manifest["samples"]
            .as_array()
            .expect("samples")
            .iter()
            .find(|sample| sample["alias"] == alias)
            .expect("selected alias");
        let anchor = uuid(&sample["anchor"]["run"]["id"]);
        let members = sample["member_run_ids"]
            .as_array()
            .expect("complete members")
            .iter()
            .map(uuid)
            .collect::<BTreeSet<_>>();
        assert!(members.contains(&anchor), "anchor membership");
        let index = load(root.join("private/export-index.json"));
        let artifacts = index["artifacts"]
            .as_array()
            .expect("export artifacts")
            .clone();
        Self {
            root,
            alias,
            entry,
            anchor,
            members,
            artifacts,
        }
    }

    fn rows(&self, kind: &str) -> Vec<Value> {
        let mut rows = Vec::new();
        for artifact in self.artifacts.iter().filter(|a| a["kind"] == kind) {
            if !artifact["member_run_ids"]
                .as_array()
                .expect("artifact members")
                .iter()
                .any(|id| self.members.contains(&uuid(id)))
            {
                continue;
            }
            let file = safe(
                File::open(
                    self.root
                        .join(artifact["path"].as_str().expect("artifact path")),
                ),
                "raw export open",
            );
            for line in BufReader::new(file).lines() {
                let line = safe(line, "raw export line");
                if line.trim().is_empty() {
                    continue;
                }
                let row: Value = safe(serde_json::from_str(&line), "raw export JSON");
                let metadata = if kind == "progress" {
                    &row["row"]
                } else {
                    &row["metadata"]
                };
                let id = if kind == "runs" {
                    &metadata["id"]
                } else {
                    &metadata["flow_run_id"]
                };
                if self.members.contains(&uuid(id)) {
                    rows.push(row);
                }
            }
        }
        rows
    }
}

async fn owned_connection() -> sqlx::PgPool {
    // No dotenv/fallback. The runner must supply the port of its labelled Docker PG.
    let url = std::env::var("DATABASE_URL").expect("explicit owned PG URL");
    let options = safe(
        sqlx::postgres::PgConnectOptions::from_str(&url),
        "owned PG options",
    );
    let port: u16 = std::env::var("HISTORY_INIT_OWNED_PG_PORT")
        .expect("owned port")
        .parse()
        .expect("owned port number");
    assert!(
        matches!(options.get_host(), "127.0.0.1" | "localhost"),
        "owned loopback PG"
    );
    assert!(
        port != 35432 && port != 7600 && port != 7800,
        "refuse source/service ports"
    );
    assert!(options.get_port() == port, "owned port binding");
    let pool = safe(
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .connect_with(options.disable_statement_logging())
            .await,
        "owned PG connect",
    );
    let label: String = safe(
        sqlx::query_scalar("select current_setting('cluster_name')")
            .fetch_one(&pool)
            .await,
        "owned PG label",
    );
    assert!(
        label == std::env::var("HISTORY_INIT_PG_LABEL").expect("owned PG label"),
        "owned PG identity"
    );
    pool
}

// Identifiers are the fixed table allowlist and actual pg_attribute column names.
// JSON values are bound, never interpolated; generated columns retain DB ownership.
async fn insert_record(store: &PgControlPlaneStore, table: &str, row: &Value) {
    assert!(matches!(
        table,
        "flow_runs"
            | "runtime_events"
            | "application_run_conversation_message_items"
            | "application_run_native_projection_progress"
    ));
    let columns: Vec<String> = safe(sqlx::query_scalar(
        "select attname::text from pg_attribute where attrelid=$1::regclass and attnum>0 and not attisdropped and attgenerated='' order by attnum")
        .bind(table).fetch_all(store.pool()).await, "import columns");
    let columns = columns
        .into_iter()
        .filter(|name| row.get(name).is_some())
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(",");
    safe(sqlx::query(sqlx::AssertSqlSafe(format!(
        "insert into {table} ({columns}) select {columns} from jsonb_populate_record(null::{table},$1::jsonb)")))
        .bind(row).execute(store.pool()).await, "import row");
}

fn map_domain(row: &mut Value, seeded: &RuntimeSeedState, compiled: Uuid, key: Uuid) {
    for (field, value) in [
        ("application_id", seeded.application_id),
        ("scope_id", seeded.workspace_id),
        ("flow_id", seeded.flow_id),
        ("flow_draft_id", seeded.draft_id),
        ("compiled_plan_id", compiled),
        ("api_key_id", key),
        ("created_by", seeded.actor_user_id),
        ("updated_by", seeded.actor_user_id),
    ] {
        if row.get(field).is_some() {
            row[field] = json!(value);
        }
    }
}

async fn verify_run(store: &PgControlPlaneStore, row: &Value) {
    let id = uuid(&row["metadata"]["id"]);
    let actual: Value = safe(
        sqlx::query_scalar(
            "select jsonb_build_object(
        'input',runtime_original_json(input_payload,raw_json_payloads,'input_payload'),
        'output',runtime_original_json(output_payload,raw_json_payloads,'output_payload'),
        'error',runtime_original_json(error_payload,raw_json_payloads,'error_payload'),
        'context',runtime_original_json(log_context,raw_json_payloads,'log_context'),
        'raw',raw_json_payloads) from flow_runs where id=$1",
        )
        .bind(id)
        .fetch_one(store.pool())
        .await,
        "original run read",
    );
    for (name, source) in [
        ("input", "input_payload_json_text"),
        ("output", "output_payload_json_text"),
        ("error", "error_payload_json_text"),
        ("context", "log_context_projection_json_text"),
        ("raw", "raw_json_payloads_json_text"),
    ] {
        exact(
            &actual[name],
            &original(row, source),
            &format!("run.{name}"),
        );
    }
}

async fn import_history(store: &PgControlPlaneStore, corpus: &Corpus) -> Value {
    let seeded = seed_runtime_base(store).await;
    let compiled = seed_compiled_plan(store, &seeded).await;
    let key = seed_application_api_key(store, &seeded).await;
    let runs = corpus.rows("runs");
    assert!(runs.len() == corpus.members.len(), "complete run set");
    let anchor = runs
        .iter()
        .find(|r| uuid(&r["metadata"]["id"]) == corpus.anchor)
        .expect("anchor export");
    let context = original(anchor, "log_context_projection_json_text");
    let conversation = uuid(&context["log_conversation_id"]);
    // Source run/event UUIDs and every original body stay unchanged. Only the
    // relational FK/actor domain is replaced by one coherent fixture domain.
    safe(sqlx::query("insert into application_conversations(id,scope_id,application_id,api_key_id,external_user,external_conversation_id,client_protocol,client_thread_id,created_at,updated_at) values($1,$2,$3,$4,$5,$6,$7,$8,now(),now())")
        .bind(conversation).bind(seeded.workspace_id).bind(seeded.application_id).bind(key)
        .bind(anchor["metadata"]["external_user"].as_str()).bind(format!("fixture:{conversation}"))
        .bind(context["protocol"].as_str()).bind(context["thread_id"].as_str())
        .execute(store.pool()).await, "conversation scaffold");
    for source in &runs {
        let mut row = source["metadata"].clone();
        map_domain(&mut row, &seeded, compiled.id, key);
        for (field, source_field) in [
            ("input_payload", "input_payload_json_text"),
            ("output_payload", "output_payload_json_text"),
            ("error_payload", "error_payload_json_text"),
            ("raw_json_payloads", "raw_json_payloads_json_text"),
            ("log_context", "log_context_projection_json_text"),
        ] {
            row[field] = original(source, source_field);
        }
        // This frozen native corpus has no sidecars or other archive owners.
        exact(
            &row["raw_json_payloads"],
            &json!({}),
            "native corpus sidecar scope",
        );
        exact(
            &row["log_context"]["log_conversation_id"],
            &context["log_conversation_id"],
            "conversation domain",
        );
        assert!(
            row["import_job_id"].is_null() && row["assistant_conversation_id"].is_null(),
            "minimal FK scope"
        );
        insert_record(store, "flow_runs", &row).await;
        verify_run(store, source).await;
    }
    let events = corpus.rows("events");
    let mut node_ids = BTreeSet::new();
    for source in &events {
        let metadata = &source["metadata"];
        if !metadata["node_run_id"].is_null() && node_ids.insert(uuid(&metadata["node_run_id"])) {
            // Referenced node rows were not part of the export. Only satisfy FK;
            // no node execution, billing, provider, or callback state is replayed.
            safe(sqlx::query("insert into node_runs(id,flow_run_id,scope_id,node_id,node_type,node_alias,status,started_at) values($1,$2,$4,'fixture-native-source','llm','fixture source','succeeded',$3)")
                .bind(uuid(&metadata["node_run_id"])).bind(uuid(&metadata["flow_run_id"]))
                .bind(timestamp(&metadata["created_at"])).bind(seeded.workspace_id).execute(store.pool()).await, "node FK scaffold");
        }
        assert!(
            metadata["span_id"].is_null()
                && metadata["parent_span_id"].is_null()
                && metadata["observation_body_content_id"].is_null()
                && metadata["observation_body_manifest_id"].is_null(),
            "inline native event scope"
        );
        let mut row = metadata.clone();
        map_domain(&mut row, &seeded, compiled.id, key);
        row["payload"] = original(source, "payload_projection_json_text");
        row["raw_json_payloads"] = original(source, "raw_json_payloads_json_text");
        insert_record(store, "runtime_events", &row).await;
        let stored: Value = safe(sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id)::jsonb from runtime_events where id=$1")
            .bind(uuid(&metadata["id"])).fetch_one(store.pool()).await, "event original read");
        exact(&stored, &row["payload"], "event.original");
    }
    let projections = corpus.rows("projections");
    for source in &projections {
        let mut row = source["metadata"].clone();
        map_domain(&mut row, &seeded, compiled.id, key);
        assert!(row["record_id"].is_null(), "source projection FK scope");
        for field in ["query", "answer", "content"] {
            row[field] = source[field].clone();
        }
        row["native_message"] = original(source, "native_message_projection_json_text");
        row["raw_json_payloads"] = original(source, "raw_json_payloads_json_text");
        insert_record(store, "application_run_conversation_message_items", &row).await;
        let actual: Value = safe(sqlx::query_scalar("select runtime_original_json(native_message,raw_json_payloads,'native_message')::jsonb from application_run_conversation_message_items where id=$1")
            .bind(uuid(&row["id"])).fetch_one(store.pool()).await, "projection original read");
        exact(&actual, &row["native_message"], "projection.original");
    }
    // Imports invoke normal source triggers; restore the frozen scalar anchors
    // afterwards without disabling triggers or invoking a projection writer.
    for source in &runs {
        safe(sqlx::query("update flow_runs set message_projection_revision=$2,runtime_event_sequence_high_water=$3 where id=$1")
            .bind(uuid(&source["metadata"]["id"]))
            .bind(source["metadata"]["message_projection_revision"].as_i64().expect("revision"))
            .bind(source["metadata"]["runtime_event_sequence_high_water"].as_i64().expect("sequence high water"))
            .execute(store.pool()).await, "restore scalar anchors");
    }
    let progress = corpus.rows("progress");
    for source in &progress {
        insert_record(
            store,
            "application_run_native_projection_progress",
            &source["row"],
        )
        .await;
    }
    if corpus.alias == "S03" {
        assert!(progress.is_empty(), "S03 real absent progress");
        assert!(
            projections
                .iter()
                .all(|r| r["metadata"]["projection_version"] == 6),
            "S03 real v6"
        );
    } else {
        assert!(
            progress.len() == runs.len(),
            "S02 complete original progress"
        );
        assert!(
            projections
                .iter()
                .all(|r| r["metadata"]["projection_version"] == 7),
            "S02 real v7"
        );
        safe(sqlx::query("update application_run_native_projection_progress set invalidated=true where flow_run_id=any($1)")
            .bind(corpus.members.iter().copied().collect::<Vec<_>>()).execute(store.pool()).await, "owned S02 controlled invalidation");
    }
    let entry_source = if corpus.entry == "create" {
        anchor.clone()
    } else {
        // A repair child never reads the anchor input/context, even temporarily:
        // materializing then freeing them would contaminate its initial RSS.
        json!({"metadata":{"id":anchor["metadata"]["id"],"status":anchor["metadata"]["status"],
            "finished_at":anchor["metadata"]["finished_at"]},
            "output_payload_json_text":anchor["output_payload_json_text"],
            "error_payload_json_text":anchor["error_payload_json_text"]})
    };
    let manifest = json!({"alias":corpus.alias,"entry":corpus.entry,"anchor":corpus.anchor,
        "members":corpus.members.iter().collect::<Vec<_>>(),"application":seeded.application_id,
        "scope":seeded.workspace_id,"actor":seeded.actor_user_id,"flow":seeded.flow_id,
        "draft":seeded.draft_id,"compiled":compiled.id,"api_key":key,
        "conversation":conversation,"anchor_source":entry_source});
    private_write("entry.json", &manifest);
    write_json(
        output().join("fixture.json"),
        &json!({"alias":corpus.alias,"entry":corpus.entry,"concurrency":1,
        "runs":runs.len(),"events":events.len(),"messages":projections.len(),"original_progress":progress.len(),
        "source_projection_version":if corpus.alias=="S03" {6} else {7},
        "invalidation":if corpus.alias=="S02" {"controlled in owned fixture DB"} else {"none: real v6/no-progress"},
        "identity":"all original run/event UUIDs and their relative order preserved; coherent scaffold FK/actor domain; source conversation UUID retained",
        "create_scope":"one additional new UUID after all historical members; no original member excluded; source input/prompt/tool-results preserved; production regenerates derived context identity",
        "excluded":"source DB, actual flow/provider/callback execution, original node/billing state; referenced nodes are FK-only scaffold",
        "import_original_value_comparisons":true}),
    );
    manifest
}

pub(super) async fn coordinate() {
    fs::create_dir_all(output()).expect("fixture output");
    private_dir();
    let corpus = Corpus::open();
    let admission = owned_connection().await;
    admission.close().await;
    let database = safe(
        postgres_test_support::PostgresTestSchema::create(
            &std::env::var("DATABASE_URL").expect("owned URL"),
        )
        .await,
        "isolated schema",
    );
    let url = database.database_url().to_owned();
    let store = PgControlPlaneStore::new(safe(database.connect().await, "isolated schema pool"));
    safe(run_migrations(store.pool()).await, "formal migrations");
    import_history(&store, &corpus).await;
    // Import buffers belong to this process. Keep the coordinator's schema guard
    // alive until the child exits; it measures a different one-connection pool.
    drop(corpus);
    let log = safe(
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(output().join("child.log")),
        "child log",
    );
    let status = safe(
        Command::new(std::env::current_exe().expect("binary"))
            .args([
                "--ignored",
                "--exact",
                TEST,
                "--nocapture",
                "--test-threads=1",
            ])
            .env("DATABASE_URL", url)
            .env("HISTORY_MEMORY_ROLE", "initialization-measure")
            .stdout(safe(log.try_clone(), "child log handle"))
            .stderr(log)
            .status(),
        "measured child spawn",
    );
    write_json(
        output().join("child-exit.json"),
        &json!({"success":status.success(),"exit_code":status.code()}),
    );
    store.pool().close().await;
    drop(store);
    assert!(
        status.success(),
        "measured child failed; private evidence retained"
    );
}

fn create_input(manifest: &Value) -> CreateFlowRunInput {
    let source = &manifest["anchor_source"];
    let metadata = &source["metadata"];
    let context = original(source, "log_context_projection_json_text");
    let typed: ApplicationRunLogContext = safe(
        serde_json::from_value(context.clone()),
        "typed request context",
    );
    // Derived log_conversation_id/task/parent associations are not command fields.
    // Check every actual command field by Value, preserving all body and key order.
    let encoded = safe(serde_json::to_value(&typed), "typed context encoding");
    for (field, value) in encoded.as_object().expect("typed context object") {
        if context.get(field).is_some() {
            exact(value, &context[field], &format!("create.context.{field}"));
        }
    }
    CreateFlowRunInput {
        application_run_log_context: Some(typed),
        actor_user_id: uuid(&manifest["actor"]),
        application_id: uuid(&manifest["application"]),
        flow_id: uuid(&manifest["flow"]),
        flow_draft_id: uuid(&manifest["draft"]),
        compiled_plan_id: uuid(&manifest["compiled"]),
        debug_session_id: String::new(),
        flow_schema_version: metadata["flow_schema_version"]
            .as_str()
            .expect("schema version")
            .into(),
        document_hash: metadata["document_hash"]
            .as_str()
            .expect("document hash")
            .into(),
        run_mode: FlowRunMode::PublishedApiRun,
        target_node_id: metadata["target_node_id"].as_str().map(str::to_owned),
        title: metadata["title"].as_str().expect("title").into(),
        status: FlowRunStatus::Queued,
        input_payload: original(source, "input_payload_json_text"),
        started_at: OffsetDateTime::now_utc(),
        api_key_id: Some(uuid(&manifest["api_key"])),
        publication_version_id: metadata["publication_version_id"]
            .as_str()
            .map(|s| safe(Uuid::parse_str(s), "publication UUID")),
        assistant_conversation_id: None,
        external_user: metadata["external_user"].as_str().map(str::to_owned),
        external_conversation_id: metadata["external_conversation_id"]
            .as_str()
            .map(str::to_owned),
        external_trace_id: metadata["external_trace_id"].as_str().map(str::to_owned),
        compatibility_mode: metadata["compatibility_mode"].as_str().map(str::to_owned),
        idempotency_key: None,
    }
}

async fn branch_state(store: &PgControlPlaneStore, members: &[Uuid]) -> Value {
    safe(sqlx::query_scalar("select jsonb_build_object('members',(select count(*) from flow_runs where id=any($1)),
        'v6_messages',(select count(*) from application_run_conversation_message_items where flow_run_id=any($1) and projection_version=6),
        'v7_progress',(select count(*) from application_run_native_projection_progress where flow_run_id=any($1) and projection_version=7),
        'invalidated',(select count(*) from application_run_native_projection_progress where flow_run_id=any($1) and invalidated),
        'read_contexts',(select coalesce(sum(last_read_context_count),0) from application_run_native_projection_progress where flow_run_id=any($1)),
        'read_events',(select coalesce(sum(last_read_event_count),0) from application_run_native_projection_progress where flow_run_id=any($1)))")
        .bind(members).fetch_one(store.pool()).await, "branch scalar evidence")
}

pub(super) async fn measure() {
    let mut trace = Probe::new(output().join("measured"));
    trace.point("process-ready", true);
    let store = PgControlPlaneStore::new(owned_connection().await);
    trace.point("pool-ready-no-history-loaded", true);
    trace.begin("entry-input-materialization");
    let mut manifest = load(private_dir().join("entry.json"));
    let entry = manifest["entry"].as_str().expect("entry").to_owned();
    let mut members = manifest["members"]
        .as_array()
        .expect("member IDs")
        .iter()
        .map(uuid)
        .collect::<Vec<_>>();
    let before = branch_state(&store, &members).await;
    if manifest["alias"] == "S03" {
        assert!(
            before["v7_progress"] == 0 && before["v6_messages"].as_u64().is_some_and(|n| n > 0),
            "real S03 legacy branch precondition"
        );
    } else {
        assert!(
            before["invalidated"].as_u64() == Some(members.len() as u64),
            "controlled S02 full-repair precondition"
        );
    }
    let source = manifest
        .as_object_mut()
        .expect("entry object")
        .remove("anchor_source")
        .expect("anchor source");
    let returned;
    let target;
    let started;
    if entry == "create" {
        manifest["anchor_source"] = source;
        let input = create_input(&manifest);
        let source = manifest
            .as_object_mut()
            .expect("entry object")
            .remove("anchor_source")
            .expect("source");
        drop(source);
        trace.point("create-input-ready", true);
        trace.begin("actual-create-published-flow-run");
        started = std::time::Instant::now();
        let receipt = safe(
            ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input).await,
            "actual repo create",
        );
        let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        trace.point("create-returned-held", true);
        trace.begin("create-return-oracle");
        assert!(receipt.created, "create must not be an idempotency hit");
        target = receipt.flow_run.id;
        assert!(!members.contains(&target), "new run UUID");
        assert!(
            members.iter().all(|id| *id < target),
            "new UUID is after retained original order"
        );
        exact(
            &receipt.flow_run.input_payload,
            &input.input_payload,
            "create.return.input_payload",
        );
        private_write("new-run.json", &json!({"target":target}));
        write_json(
            output().join("entry-timing.json"),
            &json!({"entry":entry,"calls":1,"concurrency":1,"wall_ms":wall_ms,
            "created":true,"idempotency_key_null":true,"new_uuid_after_all_original_members":true,
            "scope":"repository creation/context commit plus separate synchronous summary/messages/task transaction; excludes translation/service prefix"}),
        );
        returned = receipt.flow_run;
        members.push(target);
        drop(input);
    } else {
        // Source input/context bodies were exported for coordinator/oracle only.
        // Extract only UpdateFlowRunInput fields, then release all source material
        // BEFORE marking the measured entry. No get_flow_run setup read occurs.
        let metadata = &source["metadata"];
        target = uuid(&manifest["anchor"]);
        let status = match metadata["status"].as_str().expect("source status") {
            "succeeded" => FlowRunStatus::Succeeded,
            "cancelled" => FlowRunStatus::Cancelled,
            "failed" => FlowRunStatus::Failed,
            _ => panic!("selected repair anchor must be terminal"),
        };
        let input = UpdateFlowRunInput {
            flow_run_id: target,
            status,
            output_payload: original(&source, "output_payload_json_text"),
            error_payload: if source["error_payload_json_text"].is_null() {
                None
            } else {
                Some(original(&source, "error_payload_json_text"))
            },
            finished_at: metadata["finished_at"]
                .as_str()
                .map(|_| timestamp(&metadata["finished_at"])),
        };
        drop(source);
        trace.point("repair-small-update-ready", true);
        trace.begin("actual-update-flow-run-full-repair");
        started = std::time::Instant::now();
        returned = safe(
            store.update_flow_run(&input).await,
            "actual retained repair",
        );
        let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        trace.point("repair-returned-held", true);
        trace.begin("repair-return-oracle");
        exact(
            &returned.output_payload,
            &input.output_payload,
            "repair.return.output_payload",
        );
        assert!(
            returned.id == target && returned.status == status,
            "repair return identity/status"
        );
        write_json(
            output().join("entry-timing.json"),
            &json!({"entry":entry,"calls":1,"concurrency":1,"wall_ms":wall_ms,
            "scope":"update_flow_run real transaction, complete native repair, commit and terminal conversation-message follow-up; setup import excluded"}),
        );
        drop(input);
    }
    trace.point("returned-run-held-input-dropped", true);
    trace.begin("drop-return-and-manifest");
    drop(returned);
    let application = uuid(&manifest["application"]);
    let conversation = uuid(&manifest["conversation"]);
    private_write("target.json", &json!({"target":target,"members":members}));
    drop(manifest);
    trace.point("entry-bodies-dropped-pool-open", true);
    let after = branch_state(&store, &members).await;
    assert!(
        after["v6_messages"] == 0 && after["invalidated"] == 0,
        "repair completed before return"
    );
    assert!(
        after["v7_progress"].as_u64() == Some(members.len() as u64),
        "all members publish v7 progress"
    );
    let associated: Vec<Uuid> = safe(
        sqlx::query_scalar(
            "select run_id from application_run_log_conversation_runs($1,$2) order by run_id",
        )
        .bind(application)
        .bind(conversation)
        .fetch_all(store.pool())
        .await,
        "actual associated member set",
    );
    let mut expected_members = members.clone();
    expected_members.sort();
    assert!(
        associated == expected_members,
        "complete actual conversation association"
    );
    write_json(
        output().join("branch-evidence.json"),
        &json!({"before":before,"after":after,
        "no_get_flow_run_before_entry":true,"whole_history_loaded_by_measured_child_before_repair":false}),
    );
    trace.begin("close-measured-pool");
    store.pool().close().await;
    drop(store);
    trace.point("measured-pool-closed", true);
    // Optional runner handshake, after the timed entry and all drop endpoints.
    // It allows one PG cgroup snapshot before the separate, body-heavy oracle;
    // no sampler/backtrace or handshake runs inside the measured operation.
    if std::env::var("HISTORY_INIT_ORACLE_GATE").as_deref() == Ok("1") {
        write_json(
            output().join("entry-complete.json"),
            &json!({"entry_complete":true}),
        );
        let wait = std::time::Instant::now();
        while !output().join("oracle-release").exists() {
            assert!(wait.elapsed().as_secs() < 120, "runner oracle gate timeout");
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }
    // Full originals/UI are deliberately outside the real-entry phase, on a
    // different connection. The same checks run once, even in performance mode.
    trace.begin("full-original-and-ui-oracle");
    oracle(&members, target).await;
    trace.point("oracle-complete", true);
}

async fn oracle(members: &[Uuid], target: Uuid) {
    let corpus = Corpus::open();
    let store = PgControlPlaneStore::new(owned_connection().await);
    let mut expected = BTreeMap::<String, (Uuid, i64, Value, bool)>::new();
    let mut merge = |key: String, owner: Uuid, sequence: i64, item: Value| {
        if let Some((prior_owner, prior_sequence, prior_item, conflict)) = expected.get_mut(&key) {
            *conflict |= *prior_item != item;
            if (owner, sequence) < (*prior_owner, *prior_sequence) {
                *prior_owner = owner;
                *prior_sequence = sequence;
                *prior_item = item;
            }
        } else {
            expected.insert(key, (owner, sequence, item, false));
        }
    };
    for source in corpus.rows("runs") {
        verify_run(&store, &source).await;
        let owner = uuid(&source["metadata"]["id"]);
        let mut context = original(&source, "log_context_projection_json_text");
        let results = context
            .as_object_mut()
            .expect("context object")
            .remove("tool_results")
            .expect("results");
        for (index, item) in results.as_array().expect("result array").iter().enumerate() {
            let id = item["call_id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{owner}:{}", index + 1));
            merge(
                format!("result:{id}"),
                owner,
                -1_000_000 + index as i64 + 1,
                item.clone(),
            );
        }
    }
    for source in corpus.rows("events") {
        let metadata = &source["metadata"];
        let payload = original(&source, "payload_projection_json_text");
        let stored:Value=safe(sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id)::jsonb from runtime_events where id=$1")
            .bind(uuid(&metadata["id"])).fetch_one(store.pool()).await,"oracle event original");
        exact(&stored, &payload, "oracle.event.original");
        let item = payload["item"].clone();
        let kind = if item["call_id"].is_string() {
            "tool"
        } else {
            item["type"].as_str().unwrap_or("unknown")
        };
        let id = item["call_id"]
            .as_str()
            .or_else(|| item["id"].as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| uuid(&metadata["id"]).to_string());
        merge(
            format!("output:{kind}:{id}"),
            uuid(&metadata["flow_run_id"]),
            metadata["sequence"].as_i64().expect("event sequence"),
            item,
        );
    }
    drop(merge);
    let mut seen = BTreeSet::new();
    let mut ui_messages = 0;
    let manifest = load(private_dir().join("entry.json"));
    for owner in members {
        let rows:Vec<Value>=safe(sqlx::query_scalar("select jsonb_build_object('id',id,'key',source_item_key,'sequence',source_occurrence_sequence,'body',runtime_original_json(native_message,raw_json_payloads,'native_message')) from application_run_conversation_message_items where flow_run_id=$1 order by display_sequence,id")
            .bind(owner).fetch_all(store.pool()).await,"oracle canonical messages");
        let mut previous = None;
        for row in &rows {
            if row["sequence"].is_null() {
                continue;
            }
            let key = row["key"].as_str().expect("source key");
            let (expected_owner, sequence, item, conflict) =
                expected.get(key).expect("canonical key from real source");
            assert!(
                *owner == *expected_owner,
                "canonical minimum original owner"
            );
            assert!(
                row["sequence"].as_i64() == Some(*sequence),
                "canonical source order"
            );
            let rank = (*sequence, key.to_owned());
            assert!(
                previous.as_ref().is_none_or(|last| *last < rank),
                "canonical display ordering"
            );
            previous = Some(rank);
            exact(
                &row["body"]["_source_item"],
                item,
                "oracle.canonical._source_item",
            );
            if *conflict {
                assert!(
                    row["body"]["_log_conflicting"] == true,
                    "observed conflict retained"
                );
            }
            assert!(
                seen.insert(key.to_owned()),
                "canonical key unique across members"
            );
        }
        let ui = safe(
            store
                .read_native_agent_log_messages(
                    uuid(&manifest["scope"]),
                    uuid(&manifest["application"]),
                    *owner,
                )
                .await,
            "oracle actual UI reader",
        );
        assert!(ui.len() == rows.len(), "UI complete row count");
        let revision: i64 = safe(sqlx::query_scalar("select source_revision from application_run_native_projection_progress where flow_run_id=$1")
            .bind(owner).fetch_one(store.pool()).await, "UI published revision");
        for message in &ui {
            let row = rows
                .iter()
                .find(|row| row["id"] == message["id"])
                .expect("UI row identity");
            let mut expected_body = row["body"].clone();
            expected_body["_log_source_revision"] = json!(format!("v7:{revision}"));
            exact(
                &message["native_message"],
                &expected_body,
                "UI.full_native_message",
            );
        }
        ui_messages += ui.len();
        // Full Value snapshots for later paired baseline/candidate comparison.
        private_write(&format!("ui-{owner}.json"), &json!(ui));
    }
    assert!(
        seen.len() == expected.len(),
        "all retained source keys projected"
    );
    let state:Value=safe(sqlx::query_scalar("select jsonb_build_object('messages',(select jsonb_agg(to_jsonb(m) order by flow_run_id,display_sequence,id) from application_run_conversation_message_items m where flow_run_id=any($1)),'progress',(select jsonb_agg(to_jsonb(p) order by flow_run_id) from application_run_native_projection_progress p where flow_run_id=any($1)),'summary',(select jsonb_agg(to_jsonb(s) order by flow_run_id) from application_run_log_summaries s where flow_run_id=any($1)),'tasks',(select jsonb_agg(to_jsonb(t) order by id) from application_run_log_tasks t where application_id=$2))")
        .bind(members).bind(uuid(&manifest["application"])).fetch_one(store.pool()).await,"private complete result snapshot");
    private_write("result-snapshot.json", &state);
    if corpus.entry == "create" {
        let context:Value=safe(sqlx::query_scalar("select runtime_original_json(log_context,raw_json_payloads,'log_context')::jsonb from flow_runs where id=$1")
            .bind(target).fetch_one(store.pool()).await,"new run retained context");
        let source = original(
            &manifest["anchor_source"],
            "log_context_projection_json_text",
        );
        for field in ["prompt", "tool_results"] {
            exact(
                &context[field],
                &source[field],
                &format!("new-run.context.{field}"),
            );
        }
    }
    write_json(
        output().join("oracle.json"),
        &json!({"original_values_exact":true,"canonical_source_items_exact":true,
        "original_owner_order_preserved":true,"ui_source_items_exact":true,"members":members.len(),
        "canonical_keys":seen.len(),"ui_messages":ui_messages,"snapshot":"private/result-snapshot.json",
        "scope":"full Value equality; historical projection version is expected to advance; no digest-only equivalence claim; source node/billing fixture not imported"}),
    );
    store.pool().close().await;
}
