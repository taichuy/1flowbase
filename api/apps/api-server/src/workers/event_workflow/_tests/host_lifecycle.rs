//! Real lifecycle observer workflows; queue handoff and workflow outcome have separate owners.
use super::*;
use control_plane::event_center::{EventCenterSignalBody, EventRuntimeSubscription};
use futures_util::FutureExt;
use std::{future::Future, panic::AssertUnwindSafe};

async fn with_worker(
    fixture: &Fixture,
    statuses: EventWorkflowStatuses,
    checks: impl Future<Output = ()>,
) {
    let _router = crate::app_with_state_and_config(fixture.state.clone(), &test_config());
    let owner = GenerationDispatch::acquire(&fixture.state).expect("real serving generation");
    let worker = spawn_event_workflow_loop(fixture.state.clone(), owner, statuses);
    let result = AssertUnwindSafe(tokio::time::timeout(Duration::from_secs(15), checks))
        .catch_unwind()
        .await;
    tokio::time::timeout(Duration::from_secs(5), worker.shutdown())
        .await
        .unwrap();
    match result {
        Ok(result) => result.expect("lifecycle workflow must reach the observed state"),
        Err(panic) => std::panic::resume_unwind(panic),
    }
    assert!(fixture
        .state
        .event_center
        .snapshot(fixture.actor.current_workspace_id)
        .queues
        .is_empty());
}

async fn idle(fixture: &Fixture, statuses: &EventWorkflowStatuses, application_id: Uuid) {
    loop {
        if statuses
            .snapshot(fixture.actor.current_workspace_id)
            .iter()
            .any(|row| row.application_id == application_id && row.activation == "active")
            && control_plane_contracts::ports::EventBus::event_bus_snapshot(fixture.bus.as_ref())
                .waiters
                > 0
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn outcome(
    fixture: &Fixture,
    statuses: &EventWorkflowStatuses,
    application_id: Uuid,
    delivery: &str,
) -> EventWorkflowStatus {
    loop {
        if let Some(row) = statuses
            .snapshot(fixture.actor.current_workspace_id)
            .into_iter()
            .find(|row| row.application_id == application_id && row.last_delivery == Some(delivery))
        {
            return row;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn count(fixture: &Fixture, application_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM flow_runs WHERE application_id=$1")
        .bind(application_id)
        .fetch_one(fixture.state.store.pool())
        .await
        .unwrap()
}

fn original(fixture: &Fixture) -> EventRuntimeSubscription {
    EventRuntimeSubscription {
        subscription_id: Uuid::now_v7(),
        workspace_id: fixture.definition.workspace_id,
        contract_id: fixture.definition.contract_id.clone(),
        trigger_phase: EventTriggerPhase::Published,
        consumer_id: Uuid::now_v7(),
    }
}

async fn persisted_observer(
    fixture: &Fixture,
    outcome: &EventWorkflowStatus,
    source: Uuid,
    phase: &str,
) {
    let (status, mode, publication, input, output): (String, String, Uuid, serde_json::Value, serde_json::Value) =
        sqlx::query_as("SELECT status::text,run_mode,publication_version_id,input_payload,output_payload FROM flow_runs WHERE id=$1 AND application_id=$2")
            .bind(outcome.last_run_id.unwrap()).bind(fixture.snapshot.trigger.application_id)
            .fetch_one(fixture.state.store.pool()).await.unwrap();
    assert_eq!(status, "succeeded");
    assert_eq!(mode, "workflow_event_run");
    assert_eq!(publication, fixture.snapshot.publication.id);
    assert_eq!(input["node-workflow-start"]["invoice"], source.to_string());
    assert_eq!(output, json!({"invoice": source.to_string()}));
    assert_eq!(
        input["trigger"]["signal_id"],
        outcome.last_signal_id.unwrap().to_string()
    );
    assert_eq!(
        input["trigger"]["subscription_id"],
        fixture.snapshot.trigger.id.to_string()
    );
    assert_eq!(input["trigger"]["trigger_phase"], phase);
    if phase == "consumed" {
        assert_eq!(input["trigger"]["event_id"], source.to_string());
    } else {
        assert!(input["trigger"]["event_id"].is_null());
    }
    assert!(input["trigger"].get("context").is_none());
}

#[tokio::test]
async fn subscribed_workflow_ignores_own_activation_and_runs_for_later_logical_join() {
    let fixture = fixture_for_phase(EventTriggerPhase::Subscribed, "/subscription_id").await;
    let statuses = EventWorkflowStatuses::default();
    // Explicit activation requests a real Subscribed fact even during initial restore.
    statuses.request_activation(&fixture.snapshot.trigger);
    with_worker(&fixture, statuses.clone(), async {
        idle(&fixture, &statuses, fixture.snapshot.trigger.application_id).await;
        // Idle means the actual receiver reached its readiness wait, so its own join
        // cannot merely be a buffered signal whose execution we have not waited for.
        assert_eq!(
            count(&fixture, fixture.snapshot.trigger.application_id).await,
            0
        );
        let before = fixture
            .state
            .event_center
            .snapshot(fixture.definition.workspace_id);
        assert!(before
            .activity
            .iter()
            .any(|row| row.phase == EventTriggerPhase::Subscribed
                && row.subscription_id == Some(fixture.snapshot.trigger.id)
                && row.admission.is_none()));
        assert!(!before
            .activity
            .iter()
            .any(|row| row.phase == EventTriggerPhase::Subscribed && row.admission.is_some()));
        let spec = original(&fixture);
        let _later = fixture
            .state
            .event_center
            .subscribe(spec.clone(), true)
            .unwrap();
        let result = outcome(
            &fixture,
            &statuses,
            fixture.snapshot.trigger.application_id,
            "succeeded",
        )
        .await;
        persisted_observer(&fixture, &result, spec.subscription_id, "subscribed").await;
        let snapshot = fixture
            .state
            .event_center
            .snapshot(fixture.definition.workspace_id);
        let join = snapshot
            .activity
            .iter()
            .find(|row| {
                row.phase == EventTriggerPhase::Subscribed
                    && row.subscription_id == Some(spec.subscription_id)
                    && row.admission.is_none()
            })
            .unwrap();
        assert_eq!(result.last_signal_id, Some(join.event_id));
        assert_eq!(
            count(&fixture, fixture.snapshot.trigger.application_id).await,
            1
        );
    })
    .await;
}

#[tokio::test]
async fn consumed_workflow_needs_original_handoff_and_finishes_while_original_guard_is_held() {
    let fixture = fixture_for_phase(EventTriggerPhase::Consumed, "/source_event_id").await;
    let statuses = EventWorkflowStatuses::default();
    with_worker(&fixture, statuses.clone(), async {
        idle(&fixture, &statuses, fixture.snapshot.trigger.application_id).await;
        // A Consumed observer alone is not an original business target.
        let orphan = fixture
            .state
            .event_center
            .publish(&fixture.definition, json!({"invoice":"no consumer"}))
            .unwrap();
        assert!(orphan.targets.is_empty());
        assert_eq!(
            count(&fixture, fixture.snapshot.trigger.application_id).await,
            0
        );
        assert!(!fixture
            .state
            .event_center
            .snapshot(fixture.definition.workspace_id)
            .activity
            .iter()
            .any(|row| row.phase == EventTriggerPhase::Consumed));
        let spec = original(&fixture);
        let consumer = fixture
            .state
            .event_center
            .subscribe(spec.clone(), false)
            .unwrap();
        let receipt = fixture
            .state
            .event_center
            .publish(&fixture.definition, json!({"invoice":"held by original"}))
            .unwrap();
        assert_eq!(receipt.targets.len(), 1);
        assert_eq!(receipt.targets[0].subscription_id, spec.subscription_id);
        assert!(!fixture
            .state
            .event_center
            .snapshot(fixture.definition.workspace_id)
            .activity
            .iter()
            .any(|row| row.phase == EventTriggerPhase::Consumed));
        let held = consumer.recv().await.unwrap().unwrap();
        assert_eq!(held.signal().event_id, receipt.event_id);
        assert!(matches!(
            &held.signal().body,
            EventCenterSignalBody::Published { .. }
        ));
        // Do not complete/drop the original guard until the real observer executor returns.
        let result = outcome(
            &fixture,
            &statuses,
            fixture.snapshot.trigger.application_id,
            "succeeded",
        )
        .await;
        persisted_observer(&fixture, &result, receipt.event_id, "consumed").await;
        let snapshot = fixture
            .state
            .event_center
            .snapshot(fixture.definition.workspace_id);
        let queue = snapshot
            .queues
            .iter()
            .find(|queue| queue.subscription_id == spec.subscription_id)
            .unwrap();
        assert_eq!((queue.buffered, queue.consumed, queue.in_flight), (0, 1, 1));
        let facts: Vec<_> = snapshot
            .activity
            .iter()
            .filter(|row| row.phase == EventTriggerPhase::Consumed && row.admission.is_none())
            .collect();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].source_event_id, Some(receipt.event_id));
        assert_eq!(facts[0].subscription_id, Some(spec.subscription_id));
        assert_eq!(facts[0].consumer_id, Some(spec.consumer_id));
        assert_eq!(result.last_signal_id, Some(facts[0].event_id));
        // Simulate the original consumer abandoning work: Drop is capacity release, not success.
        drop(held);
        let after = fixture
            .state
            .event_center
            .snapshot(fixture.definition.workspace_id);
        assert_eq!(
            after
                .activity
                .iter()
                .filter(|row| row.phase == EventTriggerPhase::Consumed && row.admission.is_none())
                .count(),
            1
        );
        assert_eq!(
            after
                .queues
                .iter()
                .find(|queue| queue.subscription_id == spec.subscription_id)
                .unwrap()
                .in_flight,
            0
        );
        assert_eq!(
            count(&fixture, fixture.snapshot.trigger.application_id).await,
            1
        );
    })
    .await;
}

#[tokio::test]
async fn published_worker_mapping_rejection_after_handoff_does_not_prevent_consumed_workflow() {
    let fixture = fixture_for_phase(EventTriggerPhase::Consumed, "/source_event_id").await;
    // This is an invocation input-mapping failure after handoff, not an engine failure.
    let failing = workflow_snapshot(
        &fixture.state,
        &fixture.actor,
        &fixture.definition,
        EventTriggerPhase::Published,
        "/payload/missing_invoice",
    )
    .await;
    let statuses = EventWorkflowStatuses::default();
    with_worker(&fixture, statuses.clone(), async {
        idle(&fixture, &statuses, fixture.snapshot.trigger.application_id).await;
        idle(&fixture, &statuses, failing.trigger.application_id).await;
        let receipt = fixture
            .state
            .event_center
            .publish(
                &fixture.definition,
                json!({"invoice":"valid business payload"}),
            )
            .unwrap();
        assert_eq!(receipt.targets.len(), 1);
        assert_eq!(receipt.targets[0].subscription_id, failing.trigger.id);
        let rejected = outcome(
            &fixture,
            &statuses,
            failing.trigger.application_id,
            "consumer_failed",
        )
        .await;
        assert_eq!(rejected.last_signal_id, Some(receipt.event_id));
        assert!(rejected.last_run_id.is_none());
        assert_eq!(count(&fixture, failing.trigger.application_id).await, 0);
        let observed = outcome(
            &fixture,
            &statuses,
            fixture.snapshot.trigger.application_id,
            "succeeded",
        )
        .await;
        persisted_observer(&fixture, &observed, receipt.event_id, "consumed").await;
        let snapshot = fixture
            .state
            .event_center
            .snapshot(fixture.definition.workspace_id);
        let facts: Vec<_> = snapshot
            .activity
            .iter()
            .filter(|row| row.phase == EventTriggerPhase::Consumed && row.admission.is_none())
            .collect();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].source_event_id, Some(receipt.event_id));
        assert_eq!(facts[0].subscription_id, Some(failing.trigger.id));
        assert_eq!(facts[0].consumer_id, Some(failing.trigger.application_id));
        assert_eq!(observed.last_signal_id, Some(facts[0].event_id));
        assert_eq!(
            count(&fixture, fixture.snapshot.trigger.application_id).await,
            1
        );
    })
    .await;
}

#[tokio::test]
async fn publication_state_automatically_controls_listener_with_legacy_disabled_config() {
    use crate::routes::application_api::{interface_publication, PatchApplicationApiStatusBody};
    let fixture = fixture().await;
    sqlx::query("UPDATE workflow_event_triggers SET enabled=false WHERE id=$1")
        .bind(fixture.snapshot.trigger.id)
        .execute(fixture.state.store.pool())
        .await
        .unwrap();
    let statuses = EventWorkflowStatuses::default();
    let port = interface_publication::port(
        fixture.state.store.clone(),
        fixture.state.infrastructure.cache_store(),
        fixture.state.event_workflow_changes.clone(),
    );
    let principal = interface_runtime::UserPrincipal::server_delegation(fixture.actor.clone());
    with_worker(&fixture, statuses.clone(), async {
        idle(&fixture, &statuses, fixture.snapshot.trigger.application_id).await;
        port.execute(
            &principal,
            interface_publication::ApplicationPublicationInput::SetEnabled {
                application_id: fixture.snapshot.trigger.application_id,
                body: PatchApplicationApiStatusBody { api_enabled: false },
            },
        )
        .await
        .unwrap();
        loop {
            if statuses
                .snapshot(fixture.actor.current_workspace_id)
                .iter()
                .any(|row| {
                    row.application_id == fixture.snapshot.trigger.application_id
                        && row.activation == "unavailable"
                        && row.reason.as_deref() == Some("application_disabled")
                })
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        port.execute(
            &principal,
            interface_publication::ApplicationPublicationInput::SetEnabled {
                application_id: fixture.snapshot.trigger.application_id,
                body: PatchApplicationApiStatusBody { api_enabled: true },
            },
        )
        .await
        .unwrap();
        idle(&fixture, &statuses, fixture.snapshot.trigger.application_id).await;
        port.execute(
            &principal,
            interface_publication::ApplicationPublicationInput::Unpublish {
                application_id: fixture.snapshot.trigger.application_id,
            },
        )
        .await
        .unwrap();
        loop {
            if statuses
                .snapshot(fixture.actor.current_workspace_id)
                .iter()
                .any(|row| {
                    row.application_id == fixture.snapshot.trigger.application_id
                        && row.activation == "unavailable"
                })
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            count(&fixture, fixture.snapshot.trigger.application_id).await,
            0
        );
    })
    .await;
}
