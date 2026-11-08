//! Generation-owned, ephemeral subscription receivers. One serial execution owner per queue.
mod execution;
pub(crate) mod refresh;
mod status;
use crate::{app_state::ApiState, generation_dispatch::GenerationDispatch};
use control_plane::{
    application_public_api::{
        publications::ApplicationPublicationVersionRecord,
        workflow_event::WorkflowEventTriggerService,
    },
    event_center::{EventCenterSubscription, EventRuntimeSubscription, EventSourceTarget},
    system_recovery::SystemWriteOwner,
};
use control_plane_contracts::ports::EventCenterRepository;
use domain::event_center::WorkflowEventTriggerRecord;
pub use status::{EventWorkflowStatus, EventWorkflowStatuses};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::{
    sync::watch,
    task::{JoinHandle, JoinSet},
};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
struct Snapshot {
    trigger: WorkflowEventTriggerRecord,
    publication: ApplicationPublicationVersionRecord,
}
struct Running {
    snapshot: Snapshot,
    subscription: Arc<EventCenterSubscription>,
    stop: watch::Sender<bool>,
    draining: bool,
    task_id: tokio::task::Id,
    emitted_activation: bool,
}
impl Running {
    fn stop(&mut self) {
        if !self.draining {
            self.subscription.close();
            let _ = self.stop.send(true);
            self.draining = true;
        }
    }
}
/// Keep this handle with the serving generation. Drop closes admission and requests a drain;
/// shutdown() additionally waits for already-started runs to return under their existing contract.
pub(crate) struct EventWorkflowLoopHandle {
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
}
impl EventWorkflowLoopHandle {
    pub(crate) async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}
impl Drop for EventWorkflowLoopHandle {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}
pub(crate) fn spawn_event_workflow_loop(
    state: Arc<ApiState>,
    owner: Arc<GenerationDispatch>,
    statuses: EventWorkflowStatuses,
) -> EventWorkflowLoopHandle {
    let (stop, receiver) = watch::channel(false);
    let task = tokio::spawn(async move {
        let _generation = owner;
        run(state, statuses, receiver).await;
    });
    EventWorkflowLoopHandle {
        stop,
        task: Some(task),
    }
}

async fn load_configured_triggers(
    state: &ApiState,
    statuses: &EventWorkflowStatuses,
) -> anyhow::Result<BTreeMap<Uuid, WorkflowEventTriggerRecord>> {
    let mut triggers = BTreeMap::new();
    for workspace in state.store.list_workflow_event_trigger_workspaces().await? {
        for trigger in state.store.list_workflow_event_triggers(workspace).await? {
            triggers.insert(trigger.application_id, trigger);
        }
    }
    statuses.retain(&triggers.keys().copied().collect());
    statuses.retain_activations(&triggers.values().map(|trigger| trigger.id).collect());
    Ok(triggers)
}

struct LogicalSubscription {
    identity: WorkflowEventTriggerRecord,
    emit_on_activation: bool,
}

async fn reconcile(
    state: &Arc<ApiState>,
    statuses: &EventWorkflowStatuses,
    running: &mut BTreeMap<Uuid, Running>,
    tasks: &mut JoinSet<Uuid>,
    logical: &mut BTreeMap<Uuid, LogicalSubscription>,
    restoring: bool,
) -> anyhow::Result<()> {
    let configured = load_configured_triggers(state, statuses).await?;
    for (id, receiver) in running.iter_mut() {
        if !configured.contains_key(id) {
            receiver.stop();
        }
    }
    update_logical(logical, &configured, restoring);
    for (id, trigger) in &configured {
        let prepared = async {
            let actor = authorized_actor(state, trigger).await?;
            WorkflowEventTriggerService::new(state.store.for_actor(actor.clone()))
                .with_registered_events(
                    state
                        .event_sources
                        .registered_events(&actor, &state.event_center),
                )
                .prepare_subscription(trigger)
                .await
        }
        .await;
        let publication = match prepared {
            Ok(publication) => publication,
            Err(error) => {
                // Publication withdrawal/disable ends the logical listener. A later activation
                // is a new subscription; ordinary refresh/source recovery does not replay it.
                if matches!(
                    error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
                    Some(
                        control_plane::errors::ControlPlaneError::InvalidInput(
                            "application_disabled"
                        ) | control_plane::errors::ControlPlaneError::NotFound(
                            "application_publication"
                        )
                    )
                ) {
                    logical.remove(id);
                }
                if error.downcast_ref::<sqlx::Error>().is_some() {
                    return Err(error);
                }
                if let Some(receiver) = running.get_mut(id) {
                    receiver.stop();
                }
                statuses.activation(
                    trigger,
                    "unavailable",
                    Some(status::public_error_code(&error, "activation_failed")),
                );
                continue;
            }
        };
        let snapshot = Snapshot {
            trigger: trigger.clone(),
            publication,
        };
        let mut activation = statuses.pending_activation(trigger.id);
        if let Some(receiver) = running.get_mut(id) {
            // The API save can commit while another refresh is already installing this exact
            // revision. Do not emit twice if the logical activation was already announced.
            if let Some(pending) = activation {
                if receiver.emitted_activation
                    && receiver.snapshot.trigger.updated_at.unix_timestamp_nanos() / 1000
                        >= pending.revision_micros
                {
                    statuses.activation_recorded(trigger.id, pending);
                    activation = None;
                }
            }
            if receiver.snapshot != snapshot || activation.is_some() {
                receiver.stop();
            }
            if receiver.draining {
                statuses.activation(trigger, "draining", Some("subscription_snapshot_changed"));
            }
            continue;
        }
        let emit_subscribed = logical
            .get(id)
            .is_some_and(|logical| logical.emit_on_activation)
            || activation.is_some();
        let spec = EventRuntimeSubscription {
            subscription_id: trigger.id,
            workspace_id: trigger.workspace_id,
            contract_id: trigger.configuration.contract_id.clone(),
            trigger_phase: trigger.configuration.trigger_phase,
            consumer_id: trigger.application_id,
        };
        let opened = match &trigger.configuration.source {
            Some(source) => state.event_center.subscribe_source(
                spec,
                EventSourceTarget {
                    filters: source.filters.clone(),

                    source_id: source.source_id.clone(),
                    scope: source.scope.clone(),
                    workflow_id: trigger.application_id,
                },
            ),
            None => {
                state
                    .event_center
                    .subscribe_workflow(spec, emit_subscribed, trigger.application_id)
            }
        };
        let subscription = match opened {
            Ok(subscription) => Arc::new(subscription),
            Err(error) => {
                statuses.activation(
                    trigger,
                    "unavailable",
                    Some(status::public_error_code(&error, "activation_failed")),
                );
                continue;
            }
        };
        let (stop, receiver) = watch::channel(false);
        let task_state = state.clone();
        let task_statuses = statuses.clone();
        let task_snapshot = snapshot.clone();
        let task_subscription = subscription.clone();
        let id = *id;
        let task_id = tasks
            .spawn(async move {
                consume(
                    task_state,
                    task_snapshot,
                    task_subscription,
                    task_statuses,
                    receiver,
                )
                .await;
                id
            })
            .id();
        if let Some(pending) = activation {
            statuses.activation_recorded(trigger.id, pending);
        }
        if let Some(logical) = logical.get_mut(&id) {
            logical.emit_on_activation = false;
        }
        statuses.activation(trigger, "active", None);
        running.insert(
            id,
            Running {
                snapshot,
                subscription,
                stop,
                draining: false,
                task_id,
                emitted_activation: emit_subscribed,
            },
        );
    }
    Ok(())
}
fn update_logical(
    logical: &mut BTreeMap<Uuid, LogicalSubscription>,
    configured: &BTreeMap<Uuid, WorkflowEventTriggerRecord>,
    restoring: bool,
) {
    logical.retain(|id, _| configured.contains_key(id));
    for (id, trigger) in configured {
        match logical.get_mut(id) {
            Some(previous) if !same_identity(&previous.identity, trigger) => {
                previous.identity = trigger.clone();
                previous.emit_on_activation = !restoring;
            }
            Some(_) => {}
            None => {
                logical.insert(
                    *id,
                    LogicalSubscription {
                        identity: trigger.clone(),
                        emit_on_activation: !restoring,
                    },
                );
            }
        }
    }
}

fn same_identity(left: &WorkflowEventTriggerRecord, right: &WorkflowEventTriggerRecord) -> bool {
    left.id == right.id
        && left.workspace_id == right.workspace_id
        && left.configuration.contract_id == right.configuration.contract_id
        && left.configuration.trigger_phase == right.configuration.trigger_phase
        && left.configuration.source == right.configuration.source
}

async fn run(
    state: Arc<ApiState>,
    statuses: EventWorkflowStatuses,
    mut stop: watch::Receiver<bool>,
) {
    let mut running = BTreeMap::new();
    let mut tasks = JoinSet::new();
    let mut logical = BTreeMap::new();
    let mut restoring = true;
    let shutdown = crate::shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        let refresh = reconcile(
            &state,
            &statuses,
            &mut running,
            &mut tasks,
            &mut logical,
            restoring,
        )
        .await;
        let refresh_failed = refresh.is_err();
        statuses.refreshed(
            refresh
                .as_ref()
                .err()
                .map(|error| status::public_error_code(error, "refresh_unavailable")),
        );
        if refresh.is_ok() {
            restoring = false;
        } else {
            for receiver in running.values_mut() {
                receiver.stop();
                statuses.activation(
                    &receiver.snapshot.trigger,
                    "unavailable",
                    Some("configuration_refresh_failed"),
                );
            }
            tracing::warn!(error = ?refresh.as_ref().err(), "event workflow configuration refresh failed");
        }
        tokio::select! {
            biased;
            _ = stop.changed() => break,
            _ = &mut shutdown => break,
            _ = state.event_workflow_changes.notified() => {},
            done = tasks.join_next_with_id(), if !tasks.is_empty() => {
                match done {
                    Some(Ok((_task_id, id))) => { running.remove(&id); }
                    Some(Err(error)) => {
                        tracing::error!(%error, "event workflow receiver task failed");
                        if let Some(id) = running.iter().find(|(_, receiver)| receiver.task_id == error.id()).map(|(id, _)| *id) {
                            if let Some(mut receiver) = running.remove(&id) {
                                receiver.stop();
                                statuses.activation(&receiver.snapshot.trigger, "unavailable", Some("receiver_task_failed"));
                            }
                        }
                    }
                    None => {}
                }
            },
            _ = tokio::time::sleep(Duration::from_secs(5)), if refresh_failed => {},
        }
        if *stop.borrow() {
            break;
        }
    }
    for receiver in running.values_mut() {
        receiver.stop();
        statuses.activation(&receiver.snapshot.trigger, "stopping", None);
    }
    while tasks.join_next().await.is_some() {}
    for receiver in running.values() {
        statuses.activation(&receiver.snapshot.trigger, "stopped", Some("owner_stopped"));
    }
    running.clear();
}

/// Background execution uses the configured workspace after current membership authorization,
/// never the user's unrelated default/selected workspace and never a forged root principal.
async fn authorized_actor(
    state: &ApiState,
    trigger: &WorkflowEventTriggerRecord,
) -> anyhow::Result<domain::ActorContext> {
    use control_plane::ports::{AuthRepository, WorkspaceRepository};
    let user = AuthRepository::find_user_by_id(&state.store, trigger.updated_by)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::PermissionDenied(
            "subscription_author_unavailable",
        ))?;
    if user.status != domain::UserStatus::Active {
        return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
            "subscription_author_disabled",
        )
        .into());
    }
    let workspace = WorkspaceRepository::get_accessible_workspace(
        &state.store,
        trigger.updated_by,
        trigger.workspace_id,
    )
    .await?
    .ok_or(control_plane::errors::ControlPlaneError::PermissionDenied(
        "workspace_access_denied",
    ))?;
    AuthRepository::load_actor_context(
        &state.store,
        trigger.updated_by,
        workspace.tenant_id,
        workspace.id,
        None,
    )
    .await
}

async fn consume(
    state: Arc<ApiState>,
    snapshot: Snapshot,
    subscription: Arc<EventCenterSubscription>,
    statuses: EventWorkflowStatuses,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        if *stop.borrow() {
            return;
        }
        let readiness = tokio::select! {
            biased;
            _ = stop.changed() => return,
            ready = subscription.ready() => ready,
        };
        match readiness {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                statuses.activation(
                    &snapshot.trigger,
                    "unavailable",
                    Some(status::public_error_code(&error, "receiver_failed")),
                );
                state.event_workflow_changes.notify_one();
                return;
            }
        }
        // Readiness owns no delivery and holds no maintenance lease while idle. Maintenance
        // may wait here; current authorization must therefore be checked after acquiring it.
        let permit = loop {
            match state
                .system_maintenance
                .try_enter_write(SystemWriteOwner::EventWorkflowExecution)
            {
                Ok(permit) => break permit,
                Err(_) => tokio::select! {
                    biased;
                    _ = stop.changed() => return,
                    _ = state.system_maintenance.wait_until_online() => {},
                },
            }
        };
        let validity = tokio::select! {
            biased;
            _ = stop.changed() => return,
            validity = execution::validate_handoff(&state, &snapshot) => validity,
        };
        if let Err(error) = validity {
            statuses.activation(
                &snapshot.trigger,
                "unavailable",
                Some(status::public_error_code(&error, "handoff_rejected")),
            );
            subscription.close();
            state.event_workflow_changes.notify_one();
            return;
        }
        if *stop.borrow() {
            return;
        }
        // No await between the fresh validity decision and the atomic ownership transfer.
        let delivery = match subscription.try_recv() {
            Ok(Some(delivery)) => delivery,
            Ok(None) => continue, // A competitor won: release the permit and revalidate next time.
            Err(error) => {
                statuses.activation(
                    &snapshot.trigger,
                    "unavailable",
                    Some(status::public_error_code(&error, "receiver_failed")),
                );
                state.event_workflow_changes.notify_one();
                return;
            }
        };
        let outcome = execution::execute_delivery(&state, &snapshot, &delivery, &statuses).await;
        if let Err(error) = &outcome {
            statuses.delivery(
                snapshot.trigger.application_id,
                delivery.signal().event_id,
                None,
                "consumer_failed",
                Some(status::public_error_code(error, "consumer_failed")),
            );
            tracing::warn!(application_id = %snapshot.trigger.application_id, %error, "event workflow consumer failed after handoff");
            state.event_workflow_changes.notify_one();
        }
        drop(permit);
        delivery.complete();
        if matches!(outcome, Ok(execution::DeliveryOutcome::Retire)) {
            subscription.close();
            return;
        }
    }
}

#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
#[path = "_tests/reconciliation.rs"]
mod tests;

#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(all(test, feature = "test-support"))]
#[path = "_tests/host_execution.rs"]
mod host_execution_tests;
