//! Latest payload-free state, bounded by the current listener configuration set.
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct EventWorkflowStatus {
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub subscription_id: Uuid,
    pub activation: &'static str,
    pub reason: Option<String>,
    pub last_signal_id: Option<Uuid>,
    pub last_run_id: Option<Uuid>,
    pub last_delivery: Option<&'static str>,
    pub last_delivery_reason: Option<String>,
}

#[derive(Clone, Default)]
pub struct EventWorkflowStatuses(
    Arc<Mutex<BTreeMap<Uuid, EventWorkflowStatus>>>,
    Arc<Mutex<Option<String>>>,
    Arc<Mutex<BTreeMap<Uuid, PendingActivation>>>,
);
#[derive(Clone, Copy)]
pub(super) struct PendingActivation {
    pub token: Uuid,
    pub revision_micros: i128,
}
impl EventWorkflowStatuses {
    pub(crate) fn request_activation(
        &self,
        trigger: &domain::event_center::WorkflowEventTriggerRecord,
    ) {
        self.2
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                trigger.id,
                PendingActivation {
                    token: Uuid::now_v7(),
                    revision_micros: trigger.updated_at.unix_timestamp_nanos() / 1000,
                },
            );
    }
    pub(super) fn pending_activation(&self, subscription_id: Uuid) -> Option<PendingActivation> {
        self.2
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&subscription_id)
            .copied()
    }
    pub(super) fn activation_recorded(&self, subscription_id: Uuid, activation: PendingActivation) {
        let mut pending = self.2.lock().unwrap_or_else(|error| error.into_inner());
        if pending
            .get(&subscription_id)
            .is_some_and(|current| current.token == activation.token)
        {
            pending.remove(&subscription_id);
        }
    }
    pub(super) fn retain_activations(&self, subscriptions: &std::collections::BTreeSet<Uuid>) {
        self.2
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|id, _| subscriptions.contains(id));
    }

    pub fn refresh_error(&self) -> Option<String> {
        self.1
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
    pub(super) fn refreshed(&self, error: Option<&'static str>) {
        *self.1.lock().unwrap_or_else(|error| error.into_inner()) = error.map(str::to_owned);
    }

    pub fn snapshot(&self, workspace_id: Uuid) -> Vec<EventWorkflowStatus> {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .filter(|row| row.workspace_id == workspace_id)
            .cloned()
            .collect()
    }
    pub(super) fn activation(
        &self,
        trigger: &domain::event_center::WorkflowEventTriggerRecord,
        state: &'static str,
        reason: Option<&'static str>,
    ) {
        let mut statuses = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let row = statuses
            .entry(trigger.application_id)
            .or_insert(EventWorkflowStatus {
                workspace_id: trigger.workspace_id,
                application_id: trigger.application_id,
                subscription_id: trigger.id,
                activation: state,
                reason: None,
                last_signal_id: None,
                last_run_id: None,
                last_delivery: None,
                last_delivery_reason: None,
            });
        row.workspace_id = trigger.workspace_id;
        row.subscription_id = trigger.id;
        row.activation = state;
        row.reason = reason.map(str::to_owned);
    }
    pub(super) fn delivery(
        &self,
        application_id: Uuid,
        signal: Uuid,
        run: Option<Uuid>,
        outcome: &'static str,
        reason: Option<&'static str>,
    ) {
        if let Some(row) = self
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get_mut(&application_id)
        {
            if row.last_signal_id != Some(signal) || run.is_some() {
                row.last_run_id = run;
            }
            row.last_signal_id = Some(signal);
            row.last_delivery = Some(outcome);
            row.last_delivery_reason = reason.map(str::to_owned);
        }
    }
    pub(super) fn retain(&self, configured: &std::collections::BTreeSet<Uuid>) {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|id, _| configured.contains(id));
    }
}

/// Public observation stores stable classifications only. Detailed owner errors stay in their
/// existing logs; provider/SQL errors and dynamic fields are never copied into this read model.
pub(super) fn public_error_code(error: &anyhow::Error, fallback: &'static str) -> &'static str {
    use control_plane_contracts::{ports::EventBusError, ControlPlaneContractError as Error};
    match error.downcast_ref::<Error>() {
        Some(Error::NotAuthenticated) => return "not_authenticated",
        Some(
            Error::PermissionDenied(code)
            | Error::NotFound(code)
            | Error::Conflict(code)
            | Error::InvalidInput(code)
            | Error::UpstreamUnavailable(code),
        ) => return *code,
        Some(Error::PluginUnavailable) => return "plugin_unavailable",
        Some(Error::InvalidComponentModuleExport { .. }) => return "invalid_component_export",
        Some(Error::InvalidStateTransition { .. }) => return "invalid_state_transition",
        None => {}
    }
    if let Some(error) = error.downcast_ref::<EventBusError>() {
        return match error {
            EventBusError::Unconfigured => "event_runtime_unconfigured",
            EventBusError::Closed => "event_subscription_closed",
            EventBusError::ResourceExhausted(_) => "event_runtime_resource_exhausted",
            EventBusError::NoConsumer => "event_consumer_unavailable",
            _ => "event_runtime_unavailable",
        };
    }
    fallback
}
