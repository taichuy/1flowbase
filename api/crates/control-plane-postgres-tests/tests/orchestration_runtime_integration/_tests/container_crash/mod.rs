use super::*;
use control_plane::orchestration_runtime::{
    ContinueFlowDebugRunCommand, OrchestrationRuntimeService,
};
use orchestration_runtime::execution_engine::container::ContainerCheckpoint;

mod harness;
mod provider;

#[tokio::test]
#[ignore = "subprocess entry; parent crash matrix owns execution"]
async fn container_crash_child_process() {
    let Ok(raw) = std::env::var("BOUNDED_CONTAINER_CRASH_CHILD") else {
        return;
    };
    let config: Value = serde_json::from_str(&raw).unwrap();
    let pool = sqlx::PgPool::connect(config["database_url"].as_str().unwrap())
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(pool);
    let run_id = Uuid::parse_str(config["run_id"].as_str().unwrap()).unwrap();
    let runtime = OrchestrationRuntimeService::new(
        store.clone(),
        provider::ReceiptProvider {
            address: config["ledger"].as_str().unwrap().into(),
            run_id,
            fallback: callback_resume::semantic_runtime::PausedProvider::new(),
        },
        Arc::new(runtime_core::runtime_engine::RuntimeEngine::for_tests()),
        "fixture-key",
        Arc::new(storage_ephemeral::MemoryProviderTransportStore::new(
            Duration::minutes(5),
        )),
        false,
    )
    .with_node_artifact_context(
        "semantic-fixture",
        std::path::PathBuf::from(config["package"].as_str().unwrap()),
    );
    let result = runtime
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: Uuid::parse_str(config["application_id"].as_str().unwrap()).unwrap(),
            flow_run_id: run_id,
            workspace_id: Uuid::parse_str(config["workspace_id"].as_str().unwrap()).unwrap(),
        })
        .await
        .unwrap();
    println!(
        "container_crash_child_terminal={}",
        json!({"flow_run_id":run_id,"status":result.flow_run.status.as_str(),"node_count":result.node_runs.len()})
    );
    store.pool().close().await;
}

async fn prepare_plan(
    store: &PgControlPlaneStore,
    compiled: &domain::CompiledPlanRecord,
    break_case: bool,
) {
    let mut plan: Value = sqlx::query_scalar("select plan from flow_compiled_plans where id=$1")
        .bind(compiled.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    plan["topological_order"] = json!(["loop", "node-llm"]);
    plan["nodes"]["node-llm"]["container_id"] = json!("loop");
    let selected_output = if break_case { "missing_result" } else { "text" };
    plan["nodes"]["loop"] = json!({
        "node_id":"loop","node_type":"loop","alias":"Bounded crash matrix","container_id":null,
        "dependency_node_ids":[],"downstream_node_ids":[],
        "bindings":{"output":{"kind":"selector","raw_value":["node-llm",selected_output],"selector_paths":[["node-llm",selected_output]]}},
        "outputs":[{"key":"result","title":"Result","value_type":"array","selector":[]},{"key":"variables","title":"Variables","value_type":"object","selector":[]}],
        "config":{"max_rounds":2,"round_timeout_ms":120000,"parallelism":1,"error_policy":if break_case {"break"} else {"fail"}}
    });
    // Use the same frozen compiled-plan consumer as the existing real service
    // fixtures; no persistence adapter or execution engine is substituted.
    let _: orchestration_runtime::compiled_plan::CompiledPlan =
        serde_json::from_value(plan.clone()).unwrap();
    sqlx::query("update flow_compiled_plans set plan=$2 where id=$1")
        .bind(compiled.id)
        .bind(plan)
        .execute(store.pool())
        .await
        .unwrap();
}

async fn checkpoint(store: &PgControlPlaneStore, run: Uuid) -> ContainerCheckpoint {
    serde_json::from_value(store.load_container_checkpoint(run).await.unwrap().unwrap()).unwrap()
}

#[tokio::test]
async fn container_real_process_crash_matrix_preserves_durable_work_and_external_receipts() {
    for phase in [
        "before_round_start",
        "after_round_start",
        "external_effect",
        "after_node_commit",
        "after_round_advance",
        "after_break_marker",
    ] {
        let schema = isolated_database().await;
        let database_url = schema.database_url().to_owned();
        let pool = schema.connect().await.unwrap();
        run_migrations(&pool).await.unwrap();
        let store = PgControlPlaneStore::new(pool);
        let seeded = seed_runtime_base(&store).await;
        let compiled = seed_compiled_plan(&store, &seeded).await;
        let (_, _, package) = callback_resume::semantic_runtime::seed_runtime_consumer(
            &store, &seeded, &compiled, None,
        )
        .await;
        prepare_plan(&store, &compiled, phase == "after_break_marker").await;
        let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
        let ledger = harness::ReceiptLedger::start(phase == "external_effect").await;
        harness::install_gate(&store, phase).await;
        let mut barrier = store.pool().begin().await.unwrap();
        let blocker_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
            .fetch_one(&mut *barrier)
            .await
            .unwrap();
        sqlx::query("select pg_advisory_xact_lock(27110005)")
            .execute(&mut *barrier)
            .await
            .unwrap();
        let config = json!({"database_url":database_url,"run_id":run.id,"application_id":seeded.application_id,
            "workspace_id":seeded.workspace_id,"package":package.0,"ledger":ledger.address});
        let logs = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tmp/test-governance/bounded-containers-crash")
            .join(run.id.to_string());
        std::fs::create_dir_all(&logs).unwrap();
        let mut child = harness::spawn_child(&config, &logs.join(format!("{phase}-crashed.log")));
        if phase == "external_effect" {
            tokio::time::timeout(
                std::time::Duration::from_secs(25),
                ledger.received.notified(),
            )
            .await
            .expect("child must reach independent external receipt ledger");
        } else {
            harness::wait_for_db_gate(&store, blocker_pid).await;
        }
        let before = checkpoint(&store, run.id).await;
        let old_nodes: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
            "select e.execution_id,n.id,n.status from flow_container_executions e join node_runs n on n.id=e.node_run_id where e.flow_run_id=$1 order by n.id",
        ).bind(run.id).fetch_all(store.pool()).await.unwrap();
        let old_rounds: Vec<(Uuid, i64)> = sqlx::query_as(
            "select execution_id,deadline_unix_ms from flow_container_rounds where flow_run_id=$1 order by round_index",
        ).bind(run.id).fetch_all(store.pool()).await.unwrap();
        let before_requests = ledger.receipts.lock().unwrap().len();
        std::fs::write(
            logs.join("before-kill.json"),
            serde_json::to_vec_pretty(&json!({
                "phase":phase,"checkpoint":before,"nodes":old_nodes,"rounds":old_rounds,
                "external_receipts":ledger.receipts.lock().unwrap().clone(),
            }))
            .unwrap(),
        )
        .unwrap();
        let expected_before = if matches!(phase, "before_round_start" | "after_round_start") {
            0
        } else {
            1
        };
        assert_eq!(before_requests, expected_before, "phase={phase}");
        let frame = before
            .frames
            .values()
            .find(|frame| frame.identity.node_id == "loop")
            .unwrap();
        assert_eq!(
            frame.started_rounds,
            if phase == "before_round_start" { 0 } else { 1 },
            "phase={phase}"
        );
        if phase == "after_break_marker" {
            assert!(frame.closed);
        }
        child.kill().await.unwrap();
        assert!(
            !child.wait().await.unwrap().success(),
            "the original process must actually die"
        );
        barrier.rollback().await.unwrap();
        // The killed connection may still be unwinding its blocked SQL. Acquiring
        // its run lock proves rollback completed before reading recovery state.
        sqlx::query("select id from flow_runs where id=$1 for update")
            .bind(run.id)
            .execute(store.pool())
            .await
            .unwrap();
        harness::remove_gate(&store, phase).await;
        assert_eq!(
            checkpoint(&store, run.id).await,
            before,
            "uncommitted proposal escaped phase={phase}"
        );
        let mut recovered =
            harness::spawn_child(&config, &logs.join(format!("{phase}-recovered.log")));
        let exit = tokio::time::timeout(std::time::Duration::from_secs(30), recovered.wait())
            .await
            .expect("fresh production recovery must be bounded")
            .unwrap();
        std::fs::write(
            logs.join("after-recovery-receipts.json"),
            serde_json::to_vec_pretty(&json!({
                "phase":phase,"process_success":exit.success(),
                "external_receipts":ledger.receipts.lock().unwrap().clone(),
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(exit.success(), "phase={phase}; inspect {}", logs.display());
        let final_run = store
            .get_flow_run(seeded.application_id, run.id)
            .await
            .unwrap()
            .unwrap();
        let uncertain = phase == "external_effect";
        assert_eq!(
            final_run.status,
            if uncertain {
                FlowRunStatus::Failed
            } else {
                FlowRunStatus::Succeeded
            },
            "phase={phase} error={:?}",
            final_run.error_payload
        );
        let expected_rounds: i64 = if uncertain || phase == "after_break_marker" {
            1
        } else {
            2
        };
        let rounds: (i64, i64) = sqlx::query_as("select count(*),coalesce(max(round_index),-1) from flow_container_rounds where flow_run_id=$1")
            .bind(run.id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(
            rounds,
            (expected_rounds, expected_rounds - 1),
            "recovery must never reset round budget phase={phase}"
        );
        let after = checkpoint(&store, run.id).await;
        let final_frame = after
            .frames
            .get(&frame.identity.execution_id)
            .expect("logical frame identity must survive fresh recovery");
        assert_eq!(final_frame.started_rounds, expected_rounds as u64);
        assert_eq!(final_frame.frozen_limit, 2);
        for (identity, receipt, status) in &old_nodes {
            let record = store
                .get_container_node_run(run.id, *identity)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(record.id, *receipt, "recovery must reuse durable receipt");
            if status == "succeeded" {
                assert_eq!(record.status, NodeRunStatus::Succeeded);
            }
        }
        for (identity, deadline) in old_rounds {
            let actual: i64 = sqlx::query_scalar("select deadline_unix_ms from flow_container_rounds where flow_run_id=$1 and execution_id=$2")
                .bind(run.id).bind(identity).fetch_one(store.pool()).await.unwrap();
            assert_eq!(actual, deadline, "recovery cannot renew the round deadline");
        }
        let receipts = ledger.receipts.lock().unwrap().clone();
        assert_eq!(
            receipts.len(),
            expected_rounds as usize,
            "external requests must not replay phase={phase}"
        );
        let identities: std::collections::BTreeSet<_> = receipts
            .iter()
            .map(|receipt| {
                receipt["execution_identity"]["execution_id"]
                    .as_str()
                    .unwrap()
            })
            .collect();
        assert_eq!(
            identities.len(),
            receipts.len(),
            "independent ledger detects duplicate execution"
        );
        for receipt in &receipts {
            let identity: domain::NodeExecutionIdentity =
                serde_json::from_value(receipt["execution_identity"].clone()).unwrap();
            let record = store
                .get_container_node_run(run.id, identity.execution_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(record.execution_identity.as_ref(), Some(&identity));
            if uncertain {
                assert_eq!(record.status, NodeRunStatus::Failed);
                assert!(record
                    .error_payload
                    .unwrap()
                    .to_string()
                    .contains("container_execution_outcome_uncertain"));
            }
        }
        let unfinished: i64 = sqlx::query_scalar(
            "select count(*) from node_runs where flow_run_id=$1 and finished_at is null",
        )
        .bind(run.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(
            unfinished, 0,
            "fresh recovery must finish all durable receipts phase={phase}"
        );
        let evidence = json!({"phase":phase,"run_id":run.id,"crashed_process":true,"before_requests":before_requests,
            "after_requests":receipts.len(),"round_count":rounds.0,"started_rounds":final_frame.started_rounds,
            "status":final_run.status.as_str(),"external_receipts":receipts});
        std::fs::write(
            logs.join("receipt.json"),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();
        println!("container_crash_matrix={evidence}");
        drop(ledger);
        store.pool().close().await;
    }
}
