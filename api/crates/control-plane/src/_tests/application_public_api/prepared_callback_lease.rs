use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn lease(
    deadline: Instant,
) -> (
    CallbackExecutionLease,
    watch::Sender<bool>,
    Arc<AtomicUsize>,
) {
    let (sender, lost) = watch::channel(false);
    let cleanups = Arc::new(AtomicUsize::new(0));
    let counted = cleanups.clone();
    let cleanup: Cleanup = Box::new(move || {
        Box::pin(async move {
            counted.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    });
    (
        CallbackExecutionLease {
            lost,
            heartbeat: tokio::spawn(std::future::pending()),
            deadline: Arc::new(std::sync::Mutex::new(deadline)),
            cleanup: Some(cleanup),
        },
        sender,
        cleanups,
    )
}

#[tokio::test]
async fn lost_prepared_lease_cancels_awaited_setup_before_side_effect() {
    struct ExecutionDrop(Arc<AtomicUsize>);
    impl Drop for ExecutionDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let (lease, sender, cleanups) = lease(Instant::now() + std::time::Duration::from_secs(60));
    let effects = AtomicUsize::new(0);
    let drops = Arc::new(AtomicUsize::new(0));
    let execution_drop = ExecutionDrop(drops.clone());
    sender.send_replace(true);
    assert!(lease
        .while_owned(Box::pin(async {
            let _execution_drop = execution_drop;
            effects.fetch_add(1, Ordering::SeqCst);
        }))
        .await
        .is_err());
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    lease.abort().await.unwrap();
    assert_eq!(cleanups.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn deadline_cancels_blocked_setup_even_if_heartbeat_task_is_not_scheduled() {
    let (lease, _sender, cleanups) = lease(Instant::now() + std::time::Duration::from_millis(5));
    assert!(tokio::time::timeout(
        std::time::Duration::from_secs(1),
        lease.while_owned(std::future::pending::<()>())
    )
    .await
    .unwrap()
    .is_err());
    lease.abort().await.unwrap();
    assert_eq!(cleanups.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn dropped_prepared_lease_runs_cleanup_once_and_settled_lease_disarms_it() {
    let (first, _sender, cleanups) = lease(Instant::now() + std::time::Duration::from_secs(60));
    drop(first);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while cleanups.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(cleanups.load(Ordering::SeqCst), 1);
    let (mut settled, _sender, cleanups) =
        lease(Instant::now() + std::time::Duration::from_secs(60));
    settled.disarm();
    drop(settled);
    tokio::task::yield_now().await;
    assert_eq!(cleanups.load(Ordering::SeqCst), 0);
}

#[test]
fn pending_native_snapshot_requires_exact_original_a_before_admission_release() {
    use crate::application_public_api::native::{NativeRequiredAction, NativeRunStatus};
    let calls =
        json!([{"id":"c1","name":"one","arguments":{}},{"id":"c2","name":"two","arguments":{}}]);
    let task = domain::CallbackTaskRecord {
        id: Uuid::from_u128(1),
        flow_run_id: Uuid::from_u128(2),
        node_run_id: Uuid::from_u128(3),
        callback_kind: "llm_tool_calls".into(),
        status: domain::CallbackTaskStatus::Pending,
        request_payload: json!({"tool_calls":calls}),
        response_payload: None,
        external_ref_payload: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        completed_at: None,
    };
    let run = NativeRunResult {
        id: task.flow_run_id,
        application_id: Uuid::from_u128(4),
        api_key_id: Uuid::from_u128(5),
        publication_version_id: Uuid::from_u128(6),
        status: NativeRunStatus::Waiting,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: None,
        answer_segments: None,
        required_action: Some(NativeRequiredAction {
            action_type: "submit_tool_outputs".into(),
            payload: json!({
                "callback_task_id":task.id,"flow_run_id":task.flow_run_id,"node_run_id":task.node_run_id,"callback_kind":"llm_tool_calls","tool_calls":calls,
            }),
        }),
        tool_calls: Some(calls),
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };
    validate_pending_native_waiting(&run, run.id, run.application_id, &task).unwrap();
    for field in [
        "run",
        "app",
        "callback",
        "node",
        "kind",
        "calls",
        "action",
        "status",
        "completed",
    ] {
        let mut changed = run.clone();
        let mut callback = task.clone();
        match field {
            "run" => changed.id = Uuid::now_v7(),
            "app" => changed.application_id = Uuid::now_v7(),
            "callback" => {
                changed.required_action.as_mut().unwrap().payload["callback_task_id"] =
                    json!(Uuid::now_v7())
            }
            "node" => {
                changed.required_action.as_mut().unwrap().payload["node_run_id"] =
                    json!(Uuid::now_v7())
            }
            "kind" => {
                changed.required_action.as_mut().unwrap().payload["callback_kind"] = json!("other")
            }
            "calls" => changed.tool_calls = Some(json!([])),
            "action" => changed.required_action.as_mut().unwrap().action_type = "other".into(),
            "status" => changed.status = NativeRunStatus::Succeeded,
            "completed" => callback.status = domain::CallbackTaskStatus::Completed,
            _ => unreachable!(),
        }
        assert!(
            validate_pending_native_waiting(&changed, run.id, run.application_id, &callback)
                .is_err(),
            "{field}"
        );
    }
}
