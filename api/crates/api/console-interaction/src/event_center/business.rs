//! Best-effort projection of facts reported by the transaction and provider owners.
use std::{collections::BTreeMap, sync::Arc};

use control_plane::event_center::EventCenterRuntime;
use control_plane_contracts::ports::*;
use domain::event_center::*;
use serde_json::{json, Value};
use uuid::Uuid;

pub(crate) struct BusinessSourceObserver(pub(crate) EventCenterRuntime);
impl BusinessSourceObserver {
    pub(crate) fn new(runtime: &EventCenterRuntime) -> Arc<Self> {
        Arc::new(Self(runtime.clone()))
    }
}

pub(crate) fn descriptor(
    contract: &str,
    schema: Value,
    scope: EventSourceScope,
    group: &str,
) -> EventSourceDescriptor {
    EventSourceDescriptor {
        source_id: format!("business/{contract}"), owner_id: format!("control-plane.{group}"),
        point_id: contract.into(), contract_id: contract.into(), name: contract.into(),
        description: "Committed business fact. Workflow queues fan out across subscriptions; consumers within one queue compete for handoff.".into(),
        payload_schema: schema, origin: EventDefinitionOrigin::Core, scope,
        binding: EventSourceBinding::Active, coverage: EventSourceCoverage::Live,
        supported_phases: vec![EventTriggerPhase::Published],
    }
}

pub(crate) fn publish(
    runtime: &EventCenterRuntime,
    source: EventSourceDescriptor,
    original_event_id: String,
    occurred_at_unix_ms: i64,
    workflow_ancestor_ids: Vec<Uuid>,
    attributes: BTreeMap<String, String>,
    mut payload: impl FnMut() -> Option<Value>,
) {
    // The registration is an invocation/commit-scoped lease. Static catalog declarations
    // remain available without retaining a workspace list or any business payload cache.
    let Ok(lease) = runtime.register_source(source) else {
        return;
    };
    lease.observe_with_payload(
        EventSourceObservation {
            original_event_id: Some(original_event_id),
            producer_id: Some("business-owner".into()),
            occurred_at_unix_ms,
            workflow_ancestor_ids,
            attributes,
            fact: EventSourceFact::Published,
        },
        &mut payload,
    );
}

impl ApplicationLifecycleObserver for BusinessSourceObserver {
    fn observe(&self, fact: ApplicationLifecycleFact) {
        let contract = fact.contract_id();
        publish(
            &self.0,
            descriptor(
                contract,
                ApplicationLifecycleFact::event_source_schema(),
                EventSourceScope::Workspace {
                    workspace_id: fact.workspace_id,
                },
                "applications",
            ),
            format!(
                "{}/{}/{}",
                fact.flow_run_id,
                fact.phase.as_str(),
                fact.status.as_str()
            ),
            (fact.occurred_at.unix_timestamp_nanos() / 1_000_000) as i64,
            fact.workflow_ancestor_ids.clone(),
            BTreeMap::from([
                ("/run_mode".into(), fact.run_mode.as_str().into()),
                ("/status".into(), fact.status.as_str().into()),
            ]),
            || Some(fact.event_payload()),
        );
    }
}

impl CommunicationLifecycleObserver for BusinessSourceObserver {
    fn observed(&self, fact: CommunicationLifecycleFact) {
        match fact {
            CommunicationLifecycleFact::Received(fact) => publish(
                &self.0,
                descriptor(
                    COMMUNICATION_RECEIVED_CONTRACT_ID,
                    CommunicationReceivedFact::event_source_schema(),
                    EventSourceScope::Workspace {
                        workspace_id: fact.workspace_id,
                    },
                    "communications",
                ),
                fact.input_id.to_string(),
                fact.received_at_ms,
                fact.workflow_ancestor_ids.clone(),
                BTreeMap::new(),
                || serde_json::to_value(&fact).ok(),
            ),
            CommunicationLifecycleFact::SendResult(fact) => {
                let result = match fact.result_kind {
                    CommunicationSendResultKind::Accepted => "accepted",
                    CommunicationSendResultKind::Failed => "failed",
                    CommunicationSendResultKind::Unknown => "unknown",
                };
                publish(
                    &self.0,
                    descriptor(
                        COMMUNICATION_SEND_RESULT_CONTRACT_ID,
                        CommunicationSendResultFact::event_source_schema(),
                        EventSourceScope::Workspace {
                            workspace_id: fact.workspace_id,
                        },
                        "communications",
                    ),
                    fact.send_log_id
                        .or(fact.turn_id)
                        .or(fact.input_id)
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| Uuid::now_v7().to_string()),
                    fact.occurred_at_ms,
                    fact.workflow_ancestor_ids.clone(),
                    BTreeMap::from([("/result_kind".into(), result.into())]),
                    || serde_json::to_value(&fact).ok(),
                );
            }
        }
    }
}

pub(crate) fn catalog(scope: EventSourceScope) -> Vec<EventCatalogEntry> {
    if !matches!(scope, EventSourceScope::Workspace { .. }) {
        return Vec::new();
    }
    let mut entries = Vec::new();
    for phase in [
        ApplicationLifecyclePhase::Started,
        ApplicationLifecyclePhase::Ended,
    ] {
        let mut entry: EventCatalogEntry = descriptor(
            phase.contract_id(),
            ApplicationLifecycleFact::event_source_schema(),
            scope.clone(),
            "applications",
        )
        .into();
        entry.group_path = vec!["applications".into(), "runs".into()];
        entries.push(entry);
    }
    for (contract, schema) in [
        (
            COMMUNICATION_RECEIVED_CONTRACT_ID,
            CommunicationReceivedFact::event_source_schema(),
        ),
        (
            COMMUNICATION_SEND_RESULT_CONTRACT_ID,
            CommunicationSendResultFact::event_source_schema(),
        ),
    ] {
        let mut entry: EventCatalogEntry =
            descriptor(contract, schema, scope.clone(), "communications").into();
        entry.group_path = vec!["communications".into()];
        if contract == COMMUNICATION_SEND_RESULT_CONTRACT_ID {
            for result in ["accepted", "failed", "unknown"] {
                let mut leaf = entry.clone();
                leaf.name = format!("{contract}.{result}");
                leaf.group_path.push("send_result".into());
                leaf.source
                    .as_mut()
                    .expect("source descriptor")
                    .selection
                    .filters
                    .insert("/result_kind".into(), result.into());
                leaf.payload_schema["properties"]["result_kind"] = json!({"const":result});
                entries.push(leaf);
            }
        }
        entries.push(entry);
    }
    entries
}

#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
#[path = "_tests/business.rs"]
mod tests;
