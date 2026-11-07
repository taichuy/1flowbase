use super::*;

#[test]
fn lifecycle_payload_fields_are_declared_and_ancestry_stays_in_the_envelope() {
    let fact = ApplicationLifecycleFact {
        phase: ApplicationLifecyclePhase::Started,
        workspace_id: Uuid::now_v7(),
        application_id: Uuid::now_v7(),
        flow_run_id: Uuid::now_v7(),
        run_mode: FlowRunMode::WorkflowEventRun,
        status: FlowRunStatus::Running,
        occurred_at: OffsetDateTime::now_utc(),
        workflow_ancestor_ids: vec![Uuid::now_v7()],
    };
    let payload = fact.event_payload();
    let schema = ApplicationLifecycleFact::event_source_schema();
    let fields = payload.as_object().unwrap().keys().collect::<Vec<_>>();
    let declared = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .collect::<Vec<_>>();
    assert_eq!(fields, declared);
    assert_eq!(payload["phase"], "started");
    assert_eq!(payload["status"], "running");
    assert!(payload.get("workflow_ancestor_ids").is_none());
    assert!(payload.get("input_payload").is_none());
    assert_eq!(fact.contract_id(), APPLICATION_RUN_STARTED_CONTRACT_ID);
    let ended = ApplicationLifecycleFact {
        phase: ApplicationLifecyclePhase::Ended,
        status: FlowRunStatus::Cancelled,
        ..fact
    };
    assert_eq!(ended.contract_id(), APPLICATION_RUN_ENDED_CONTRACT_ID);
    assert_eq!(ended.event_payload()["status"], "cancelled");
}
