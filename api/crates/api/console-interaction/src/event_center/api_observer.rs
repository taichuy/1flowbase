//! Workspace rules for document-operation facts, separate from legacy canonical rules.
use crate::app_state::ApiState;
use control_plane_contracts::ports::EventCenterRepository;
use domain::event_center::*;
use interface_runtime::{
    DocumentOperationDescriptor, DocumentOperationEvent, DocumentOperationInterestFuture,
    DocumentOperationObserver, DocumentOperationObserverFuture, InterfaceEventContext,
    InterfaceEventPrincipal, InterfaceInvocationTerminal, InterfaceScope,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, Weak};

struct MatchSnapshot {
    ancestors: Vec<uuid::Uuid>,
    definitions: Mutex<Vec<EventDefinitionRecord>>,
}
pub(crate) struct ApiOperationRuleObserver(Weak<ApiState>);
impl ApiOperationRuleObserver {
    pub(crate) fn new(state: &Arc<ApiState>) -> Arc<Self> {
        Arc::new(Self(Arc::downgrade(state)))
    }
}
fn workspace(context: &InterfaceEventContext) -> Option<uuid::Uuid> {
    match context.principal()? {
        InterfaceEventPrincipal::User(user)
            if context.definition().scope() != InterfaceScope::System || user.actor().is_root =>
        {
            Some(user.actor().current_workspace_id)
        }
        InterfaceEventPrincipal::Application(application) => Some(application.workspace_id()),
        _ => None,
    }
}
impl DocumentOperationObserver for ApiOperationRuleObserver {
    fn capture_context(&self) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
        Some(Arc::new(MatchSnapshot {
            ancestors: self.0.upgrade()?.store.workflow_ancestor_ids(),
            definitions: Mutex::new(Vec::new()),
        }))
    }
    fn interested<'a>(
        &'a self,
        context: &'a InterfaceEventContext,
        descriptor: &'a DocumentOperationDescriptor,
    ) -> DocumentOperationInterestFuture<'a> {
        Box::pin(async move {
            let Some(state) = self.0.upgrade() else {
                return Ok(false);
            };
            let Some(workspace_id) = workspace(context) else {
                return Ok(false);
            };
            if !state.event_center.configured() {
                return Ok(false);
            }
            let Some(snapshot) = context.observer_context::<MatchSnapshot>() else {
                return Ok(false);
            };
            let definitions = state
                .store
                .list_event_definitions(workspace_id)
                .await
                .map_err(|_| Arc::<str>::from("api-event-rules-unavailable"))?
                .into_iter()
                .filter(|definition| {
                    definition.enabled
                        && definition.interface_event.as_ref().is_some_and(|binding| {
                            binding.operation_id() == Some(descriptor.operation_id.as_ref())
                        })
                })
                .collect::<Vec<_>>();
            let interested = !definitions.is_empty();
            *snapshot
                .definitions
                .lock()
                .map_err(|_| Arc::<str>::from("api-event-match-snapshot-unavailable"))? =
                definitions;
            Ok(interested)
        })
    }
    fn observe(&self, event: DocumentOperationEvent) -> DocumentOperationObserverFuture<'_> {
        Box::pin(async move {
            let Some(state) = self.0.upgrade() else {
                return Ok(());
            };
            let Some(workspace_id) = workspace(event.context()) else {
                return Ok(());
            };
            let Some(snapshot) = event.context().observer_context::<MatchSnapshot>() else {
                return Ok(());
            };
            let (phase, terminal, payload) = envelope(&event);
            let definitions = snapshot
                .definitions
                .lock()
                .map_err(|_| Arc::<str>::from("api-event-match-snapshot-unavailable"))?;
            let mut failed = false;
            for definition in definitions.iter().filter(|definition| {
                definition.workspace_id == workspace_id
                    && definition.interface_event.as_ref().is_some_and(|binding| {
                        binding.matches_operation(
                            event.descriptor().operation_id.as_ref(),
                            phase,
                            terminal,
                        )
                    })
            }) {
                if let Err(error) = state.event_center.publish_with_ancestors(
                    definition,
                    payload.clone(),
                    snapshot.ancestors.clone(),
                ) {
                    failed = true;
                    tracing::warn!(%error,rule_id=%definition.id,"API operation event rule observation failed");
                }
            }
            if failed {
                Err(Arc::from("api-event-rule-publication-failed"))
            } else {
                Ok(())
            }
        })
    }
}
pub(crate) fn envelope(
    event: &DocumentOperationEvent,
) -> (InterfaceEventPhase, Option<InterfaceEventTerminal>, Value) {
    let mut payload = json!({"operation_id":event.descriptor().operation_id.as_ref(),"invocation_id":event.context().invocation_id().value().to_string()});
    match event {
        DocumentOperationEvent::Before { input, .. } => {
            payload["phase"] = json!("before");
            payload["input"] = input.clone();
            (InterfaceEventPhase::Before, None, payload)
        }
        DocumentOperationEvent::Completion {
            terminal,
            output,
            classification,
            ..
        } => {
            let terminal = match terminal {
                InterfaceInvocationTerminal::Completed => InterfaceEventTerminal::Completed,
                InterfaceInvocationTerminal::Failed => InterfaceEventTerminal::Failed,
                InterfaceInvocationTerminal::Rejected => InterfaceEventTerminal::Rejected,
                InterfaceInvocationTerminal::Cancelled => InterfaceEventTerminal::Cancelled,
            };
            payload["phase"] = json!("completion");
            payload["terminal"] = json!(terminal);
            if terminal == InterfaceEventTerminal::Completed {
                if let Some(output) = output {
                    payload["output"] = output.clone();
                }
            }
            if let Some(classification) = classification {
                payload["classification"] = json!(classification.as_ref());
            }
            (InterfaceEventPhase::Completion, Some(terminal), payload)
        }
    }
}
