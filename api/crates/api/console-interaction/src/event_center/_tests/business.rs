use super::*;
use control_plane::event_center::{
    EventCenterRuntimePolicy, EventRuntimeSubscription, EventSourceTarget,
};
use domain::{FlowRunMode, FlowRunStatus};

fn runtime() -> EventCenterRuntime {
    EventCenterRuntime::new(
        Arc::new(storage_ephemeral::MemoryEventBus::new()),
        Some(EventCenterRuntimePolicy {
            bus_limits: EventBusLimits {
                max_items: 32,
                max_json_bytes: 256_000,
                max_waiters: 8,
                max_topics: 32,
                max_consumers: 32,
                max_topic_name_bytes: 16_000,
            },
            observation_max_bytes: 256_000,
            observation_ttl_ms: 60_000,
        }),
    )
    .unwrap()
}
fn subscribe(
    runtime: &EventCenterRuntime,
    workspace_id: Uuid,
    contract: &str,
    filters: BTreeMap<String, String>,
) -> control_plane::event_center::EventCenterSubscription {
    runtime
        .subscribe_source(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id,
                contract_id: contract.into(),
                trigger_phase: EventTriggerPhase::Published,
                consumer_id: Uuid::now_v7(),
            },
            EventSourceTarget {
                scope: EventSourceScope::Workspace { workspace_id },
                source_id: format!("business/{contract}"),
                workflow_id: Uuid::now_v7(),
                filters,
            },
        )
        .unwrap()
}
#[test]
fn application_fact_reaches_its_workspace_with_actual_run_identity_and_no_payload_history() {
    let runtime = runtime();
    let observer = BusinessSourceObserver::new(&runtime);
    let workspace_id = Uuid::now_v7();
    let target = subscribe(
        &runtime,
        workspace_id,
        APPLICATION_RUN_ENDED_CONTRACT_ID,
        Default::default(),
    );
    let foreign = subscribe(
        &runtime,
        Uuid::now_v7(),
        APPLICATION_RUN_ENDED_CONTRACT_ID,
        Default::default(),
    );
    let fact = ApplicationLifecycleFact {
        phase: ApplicationLifecyclePhase::Ended,
        workspace_id,
        application_id: Uuid::now_v7(),
        flow_run_id: Uuid::now_v7(),
        run_mode: FlowRunMode::WorkflowScheduleRun,
        status: FlowRunStatus::Failed,
        occurred_at: time::OffsetDateTime::now_utc(),
        workflow_ancestor_ids: vec![Uuid::now_v7()],
    };
    observer.observe(fact.clone());
    let delivery = target.try_recv().unwrap().unwrap();
    let control_plane::event_center::EventCenterSignalBody::Published { payload } =
        &delivery.signal().body
    else {
        panic!("publication expected")
    };
    assert_eq!(*payload, fact.event_payload());
    assert_eq!(
        delivery
            .signal()
            .source_context
            .as_ref()
            .unwrap()
            .attributes["/run_mode"],
        "workflow_schedule_run"
    );
    assert!(foreign.try_recv().unwrap().is_none());
    assert!(
        !serde_json::to_value(runtime.snapshot(workspace_id))
            .unwrap()
            .to_string()
            .contains(&fact.application_id.to_string()),
        "observations cannot retain run bodies"
    );
}
#[test]
fn provider_acceptance_matches_only_its_leaf_and_preserves_reply_linkage() {
    let runtime = runtime();
    let observer = BusinessSourceObserver::new(&runtime);
    let workspace_id = Uuid::now_v7();
    let accepted = subscribe(
        &runtime,
        workspace_id,
        COMMUNICATION_SEND_RESULT_CONTRACT_ID,
        BTreeMap::from([("/result_kind".into(), "accepted".into())]),
    );
    let failed = subscribe(
        &runtime,
        workspace_id,
        COMMUNICATION_SEND_RESULT_CONTRACT_ID,
        BTreeMap::from([("/result_kind".into(), "failed".into())]),
    );
    observer.observed(CommunicationLifecycleFact::SendResult(
        CommunicationSendResultFact {
            workspace_id,
            instance_id: Uuid::now_v7(),
            send_log_id: Some(Uuid::now_v7()),
            input_id: None,
            turn_id: None,
            conversation_id: None,
            type_code: Some("wechat".into()),
            source: Some(domain::NotificationSource::Api),
            result_kind: CommunicationSendResultKind::Accepted,
            failure_code: None,
            target: Some(extension_contracts::NotificationSendTarget {
                sender_id: "sender".into(),
                conversation_id: "conversation".into(),
                provider_message_id: "reply-to-42".into(),
            }),
            content: Some(CommunicationFactContent::Text {
                message_id: None,
                content: "actual reply".into(),
            }),
            occurred_at_ms: 1234,
            workflow_ancestor_ids: Vec::new(),
        },
    ));
    let delivery = accepted.try_recv().unwrap().unwrap();
    let control_plane::event_center::EventCenterSignalBody::Published { payload } =
        &delivery.signal().body
    else {
        panic!("publication expected")
    };
    assert_eq!(payload["target"]["provider_message_id"], "reply-to-42");
    assert_eq!(payload["content"]["content"], "actual reply");
    assert_eq!(payload["result_kind"], "accepted");
    assert!(payload.get("delivered").is_none());
    assert!(failed.try_recv().unwrap().is_none());
    assert!(!serde_json::to_value(runtime.snapshot(workspace_id))
        .unwrap()
        .to_string()
        .contains("actual reply"));
}
