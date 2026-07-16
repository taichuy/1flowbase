use super::*;

#[tokio::test]
async fn orchestration_runtime_data_model_node_compiles_with_code_and_action() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({}),
            document_snapshot: Some(data_model_flow_document(
                seeded.flow_id,
                vec![data_model_node("node-list", "list", json!({}), json!({}))],
                vec![],
            )),
            debug_session_id: None,
        })
        .await
        .unwrap();

    assert_eq!(started.flow_run.status, domain::FlowRunStatus::Running);
}

#[tokio::test]
async fn orchestration_runtime_data_model_list_returns_records_and_total() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({}),
            document_snapshot: Some(data_model_flow_document(
                seeded.flow_id,
                vec![
                    data_model_node(
                        "node-create",
                        "create",
                        json!({ "payload": { "title": "Order A", "status": "draft" } }),
                        json!({}),
                    ),
                    data_model_node("node-list", "list", json!({}), json!({})),
                ],
                vec![("node-create", "node-list")],
            )),
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Succeeded);
    let list_node = node_run(&detail, "node-list");
    assert_eq!(list_node.output_payload["total"], json!(1));
    assert_eq!(
        list_node.output_payload["records"][0]["title"],
        json!("Order A")
    );
}

#[tokio::test]
async fn orchestration_runtime_data_model_get_requires_record_id() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let detail = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![data_model_node("node-get", "get", json!({}), json!({}))],
        vec![],
    )
    .await;

    assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Failed);
    let get_node = node_run(&detail, "node-get");
    assert_eq!(get_node.status, domain::NodeRunStatus::Failed);
    assert!(get_node
        .error_payload
        .as_ref()
        .and_then(|payload| payload["message"].as_str())
        .is_some_and(|message| message.contains("record_id")));
}

#[tokio::test]
async fn orchestration_runtime_data_model_create_rejects_non_object_payload() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let detail = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![data_model_node(
            "node-create",
            "create",
            json!({ "payload": "not-object" }),
            json!({}),
        )],
        vec![],
    )
    .await;

    assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Failed);
    let create_node = node_run(&detail, "node-create");
    assert_eq!(create_node.status, domain::NodeRunStatus::Failed);
    assert!(create_node
        .error_payload
        .as_ref()
        .and_then(|payload| payload["message"].as_str())
        .is_some_and(|message| message.contains("payload")));
}

#[tokio::test]
async fn orchestration_runtime_data_model_write_requires_side_effect_policy() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let detail = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![data_model_node(
            "node-create",
            "create",
            json!({
                "payload": { "title": "Order A", "status": "draft" },
                "side_effect_policy": "disabled"
            }),
            json!({}),
        )],
        vec![],
    )
    .await;

    assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Failed);
    let create_node = node_run(&detail, "node-create");
    assert_eq!(create_node.status, domain::NodeRunStatus::Failed);
    assert!(create_node
        .error_payload
        .as_ref()
        .and_then(|payload| payload["message"].as_str())
        .is_some_and(|message| message.contains("DATA_MODEL_SIDE_EFFECT_DISABLED")));
}

#[tokio::test]
async fn orchestration_runtime_data_model_confirm_each_run_waits_for_callback() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let detail = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![data_model_node(
            "node-create",
            "create",
            json!({
                "payload": { "title": "Order A", "status": "draft" },
                "side_effect_policy": "confirm_each_run"
            }),
            json!({}),
        )],
        vec![],
    )
    .await;

    assert_eq!(
        detail.flow_run.status,
        domain::FlowRunStatus::WaitingCallback
    );
    let create_node = node_run(&detail, "node-create");
    assert_eq!(create_node.status, domain::NodeRunStatus::WaitingCallback);
    assert_eq!(create_node.output_payload, json!({}));
    assert_eq!(
        create_node.debug_payload["side_effect_policy"],
        json!("confirm_each_run")
    );
    assert!(create_node.debug_payload["idempotency_key"]
        .as_str()
        .is_some_and(|value| value.starts_with("data_model:")));
    assert_eq!(
        create_node.debug_payload["payload_hash"]
            .as_str()
            .map(|value| value.starts_with("sha256:")),
        Some(true)
    );
    assert_eq!(detail.checkpoints.len(), 1);
    assert_eq!(
        detail.checkpoints[0].status,
        "waiting_data_model_side_effect_confirmation"
    );
    assert_eq!(detail.callback_tasks.len(), 1);
    assert_eq!(
        detail.callback_tasks[0].callback_kind,
        "data_model_side_effect_confirmation"
    );
    assert_eq!(
        detail.callback_tasks[0].request_payload["node_id"],
        json!("node-create")
    );
    assert_eq!(
        detail.callback_tasks[0].request_payload["run_id"],
        json!(detail.flow_run.id)
    );
    assert_eq!(
        detail.callback_tasks[0].request_payload["actor_user_id"],
        json!(seeded.actor_user_id)
    );
}

#[tokio::test]
async fn orchestration_runtime_data_model_confirmed_callback_executes_write_once() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let waiting = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![data_model_node(
            "node-create",
            "create",
            json!({
                "payload": { "title": "Order A", "status": "draft" },
                "side_effect_policy": "confirm_each_run"
            }),
            json!({}),
        )],
        vec![],
    )
    .await;

    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: waiting.callback_tasks[0].id,
            response_payload: json!({ "approved": true }),
        })
        .await
        .unwrap();

    assert_eq!(completed.callback_tasks[0].status.as_str(), "completed");
    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    let create_node = node_run(&completed, "node-create");
    assert_eq!(create_node.status, domain::NodeRunStatus::Succeeded);
    assert_eq!(
        create_node.output_payload["record"]["title"],
        json!("Order A")
    );
    assert!(create_node
        .output_payload
        .get("__side_effect_receipt")
        .is_none());
    assert_eq!(
        create_node.metrics_payload["side_effect_receipt"]["status"],
        json!("succeeded")
    );
    assert_eq!(
        create_node.metrics_payload["side_effect_replayed"],
        json!(false)
    );

    let duplicate_error = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: waiting.callback_tasks[0].id,
            response_payload: json!({ "approved": true }),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        duplicate_error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict("callback_task_not_pending"))
    ));
}

#[tokio::test]
async fn data_model_callback_resume_starts_downstream_node_before_execution() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let waiting = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![
            data_model_node(
                "node-create",
                "create",
                json!({
                    "payload": { "title": "Order A", "status": "draft" },
                    "side_effect_policy": "confirm_each_run"
                }),
                json!({}),
            ),
            data_model_node("node-list", "list", json!({}), json!({})),
        ],
        vec![("node-create", "node-list")],
    )
    .await;

    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: waiting.callback_tasks[0].id,
            response_payload: json!({ "approved": true }),
        })
        .await
        .unwrap();

    let list_node = node_run(&completed, "node-list");
    assert_eq!(list_node.output_payload["total"], json!(1));
    assert!(
        service
            .list_runtime_events(completed.flow_run.id, 0)
            .await
            .iter()
            .any(|event| {
                event.event_type == "node_started"
                    && event.payload["node_run_id"] == json!(list_node.id)
            }),
        "data model callback downstream node must be observable before execution"
    );
}

#[tokio::test]
async fn orchestration_runtime_data_model_confirmed_callback_replays_same_run_receipt() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let waiting = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![data_model_node(
            "node-create",
            "create",
            json!({
                "payload": { "title": "Order A", "status": "draft" },
                "side_effect_policy": "confirm_each_run"
            }),
            json!({}),
        )],
        vec![],
    )
    .await;
    let create_node = node_run(&waiting, "node-create");
    let callback_payload = &waiting.callback_tasks[0].request_payload;
    service
        .upsert_data_model_side_effect_receipt_for_tests(&UpsertDataModelSideEffectReceiptInput {
            workspace_id: Uuid::nil(),
            application_id: seeded.application_id,
            draft_id: waiting.flow_run.draft_id,
            flow_run_id: waiting.flow_run.id,
            node_run_id: create_node.id,
            node_id: "node-create".to_string(),
            action: "create".to_string(),
            model_code: "orders".to_string(),
            record_id: Some("record-from-receipt".to_string()),
            deleted_id: None,
            affected_count: 1,
            idempotency_key: callback_payload["idempotency_key"]
                .as_str()
                .expect("callback idempotency key")
                .to_string(),
            payload_hash: callback_payload["payload_hash"]
                .as_str()
                .expect("callback payload hash")
                .to_string(),
            actor_user_id: seeded.actor_user_id,
            scope_id: Uuid::nil(),
            status: "succeeded".to_string(),
            output_payload: json!({
                "record": {
                    "id": "record-from-receipt",
                    "title": "Order From Receipt"
                }
            }),
        })
        .await;

    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: waiting.callback_tasks[0].id,
            response_payload: json!({ "approved": true }),
        })
        .await
        .unwrap();

    let create_node = node_run(&completed, "node-create");
    assert_eq!(
        create_node.output_payload["record"]["id"],
        json!("record-from-receipt")
    );
    assert_eq!(
        create_node.metrics_payload["side_effect_replayed"],
        json!(true)
    );
}

#[tokio::test]
async fn orchestration_runtime_data_model_confirmation_rejects_different_actor() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let waiting = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![data_model_node(
            "node-create",
            "create",
            json!({
                "payload": { "title": "Order A", "status": "draft" },
                "side_effect_policy": "confirm_each_run"
            }),
            json!({}),
        )],
        vec![],
    )
    .await;

    let error = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: Uuid::now_v7(),
            application_id: seeded.application_id,
            callback_task_id: waiting.callback_tasks[0].id,
            response_payload: json!({ "approved": true }),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied(
            "data_model_side_effect_confirmation_actor"
        ))
    ));

    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: waiting.callback_tasks[0].id,
            response_payload: json!({ "approved": true }),
        })
        .await
        .unwrap();
    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
}

#[tokio::test]
async fn orchestration_runtime_data_model_update_rejects_non_object_payload() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let detail = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![
            data_model_node(
                "node-create",
                "create",
                json!({ "payload": { "title": "Order A", "status": "draft" } }),
                json!({}),
            ),
            data_model_node(
                "node-update",
                "update",
                json!({ "payload": "not-object" }),
                json!({ "record_id": selector_binding(["node-create", "record", "id"]) }),
            ),
        ],
        vec![("node-create", "node-update")],
    )
    .await;

    assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Failed);
    let update_node = node_run(&detail, "node-update");
    assert_eq!(update_node.status, domain::NodeRunStatus::Failed);
    assert!(update_node
        .error_payload
        .as_ref()
        .and_then(|payload| payload["message"].as_str())
        .is_some_and(|message| message.contains("payload")));
}

#[tokio::test]
async fn orchestration_runtime_data_model_create_update_delete_runtime_crud() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let detail = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![
            data_model_node(
                "node-create",
                "create",
                json!({ "payload": { "title": "Order A", "status": "draft" } }),
                json!({}),
            ),
            data_model_node(
                "node-update",
                "update",
                json!({ "payload": { "status": "paid" } }),
                json!({ "record_id": selector_binding(["node-create", "record", "id"]) }),
            ),
            data_model_node(
                "node-delete",
                "delete",
                json!({}),
                json!({ "record_id": selector_binding(["node-update", "record", "id"]) }),
            ),
        ],
        vec![
            ("node-create", "node-update"),
            ("node-update", "node-delete"),
        ],
    )
    .await;

    assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Succeeded);
    let create_node = node_run(&detail, "node-create");
    let update_node = node_run(&detail, "node-update");
    let delete_node = node_run(&detail, "node-delete");
    assert_eq!(
        create_node.output_payload["record"]["title"],
        json!("Order A")
    );
    assert_eq!(
        update_node.output_payload["record"]["status"],
        json!("paid")
    );
    assert_eq!(
        delete_node.output_payload["deleted_id"],
        update_node.output_payload["record"]["id"]
    );
}

#[tokio::test]
async fn orchestration_runtime_data_model_permission_denied_records_node_error() {
    let service = OrchestrationRuntimeService::for_tests_without_data_model_scope_grant();
    let seeded = service.seed_application_with_flow("Data Model Agent").await;

    let detail = run_data_model_flow(
        &service,
        seeded.actor_user_id,
        seeded.application_id,
        seeded.flow_id,
        vec![data_model_node("node-list", "list", json!({}), json!({}))],
        vec![],
    )
    .await;

    assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Failed);
    let list_node = node_run(&detail, "node-list");
    assert_eq!(list_node.status, domain::NodeRunStatus::Failed);
    assert!(list_node
        .error_payload
        .as_ref()
        .and_then(|payload| payload["message"].as_str())
        .is_some_and(|message| message.contains("permission denied")));
}
