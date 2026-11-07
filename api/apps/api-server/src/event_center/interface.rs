//! Canonical invocation facts projected into workspace-owned interface rules.
use std::sync::{Arc, Weak};

use control_plane_contracts::ports::EventCenterRepository;
use domain::{event_center::*, ActorContext};
use interface_runtime::{
    CompiledInterfaceRegistry, InterfaceEvent, InterfaceEventCapability, InterfaceEventObserver,
    InterfaceEventObserverFuture, InterfaceEventPrincipal, InterfaceInvocationTerminal,
    InterfaceScope,
};
use serde_json::{json, Value};

use crate::app_state::ApiState;

pub(crate) struct InterfaceRuleObserver(Weak<ApiState>);
impl InterfaceRuleObserver {
    pub(crate) fn new(state: &Arc<ApiState>) -> Arc<Self> {
        Arc::new(Self(Arc::downgrade(state)))
    }
}
impl InterfaceEventObserver for InterfaceRuleObserver {
    fn observe(&self, event: InterfaceEvent) -> InterfaceEventObserverFuture<'_> {
        Box::pin(async move {
            let Some(state) = self.0.upgrade() else {
                return Ok(());
            };
            let context = event.context();
            let workspace_id = match context.principal() {
                Some(InterfaceEventPrincipal::User(user)) => {
                    if context.definition().scope() == InterfaceScope::System
                        && !user.actor().is_root
                    {
                        return Ok(());
                    }
                    user.actor().current_workspace_id
                }
                Some(InterfaceEventPrincipal::Application(application)) => {
                    application.workspace_id()
                }
                // Authentication failure establishes neither a workspace nor permission to
                // disclose input. Public requests cannot supply their own workspace selector.
                _ => return Ok(()),
            };
            if !state.event_center.snapshot(workspace_id).configured {
                return Ok(());
            }
            let interface_id = context.definition().interface_id().as_str();
            let invocation_id = context.invocation_id().to_string();
            let (phase, terminal, payload) = envelope(&event);
            let definitions = state
                .store
                .list_event_definitions(workspace_id)
                .await
                .map_err(|_| Arc::<str>::from("interface-event-rules-unavailable"))?;
            let ancestors = state.store.workflow_ancestor_ids();
            for definition in definitions {
                if !definition.enabled
                    || !definition
                        .interface_event
                        .as_ref()
                        .is_some_and(|binding| binding.matches(interface_id, phase, terminal))
                {
                    continue;
                }
                if let Err(error) = state.event_center.publish_with_ancestors(
                    &definition,
                    payload.clone(),
                    ancestors.clone(),
                ) {
                    tracing::warn!(%error, %invocation_id, rule_id = %definition.id, "interface event rule observation failed");
                }
            }
            Ok(())
        })
    }
}

fn envelope(
    event: &InterfaceEvent,
) -> (InterfaceEventPhase, Option<InterfaceEventTerminal>, Value) {
    let context = event.context();
    let mut value = json!({
        "interface_id":context.definition().interface_id().as_str(),
        "invocation_id":context.invocation_id().to_string(),
    });
    match event {
        InterfaceEvent::Before { input, .. } => {
            value["phase"] = json!("before");
            value["input"] = input.clone();
            (InterfaceEventPhase::Before, None, value)
        }
        InterfaceEvent::Completion {
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
            value["phase"] = json!("completion");
            value["terminal"] = json!(terminal);
            if terminal == InterfaceEventTerminal::Completed {
                if let Some(output) = output {
                    value["output"] = output.clone();
                }
            }
            if let Some(classification) = classification {
                value["classification"] = json!(classification.as_ref());
            }
            (InterfaceEventPhase::Completion, Some(terminal), value)
        }
    }
}

/// Metadata comes from the exact compiled generation, with no invocation-value cache.
pub(crate) fn catalog(
    registry: &CompiledInterfaceRegistry,
    actor: &ActorContext,
) -> Vec<EventCatalogEntry> {
    registry
        .event_capabilities()
        .filter(|capability| {
            capability.definition().scope() != InterfaceScope::System || actor.is_root
        })
        .flat_map(|capability| {
            [InterfaceEventPhase::Before, InterfaceEventPhase::Completion]
                .map(|phase| capability_entry(capability, actor, phase))
        })
        .collect()
}

fn capability_entry(
    capability: &InterfaceEventCapability,
    actor: &ActorContext,
    phase: InterfaceEventPhase,
) -> EventCatalogEntry {
    let definition = capability.definition();
    let interface_id = definition.interface_id().as_str();
    let phase_name = match phase {
        InterfaceEventPhase::Before => "before",
        InterfaceEventPhase::Completion => "completion",
    };
    EventCatalogEntry {
        id: None, source: None, enabled: None,
        interface_event: Some(InterfaceEventBinding { interface_id: interface_id.into(), phase, results: Vec::new() }),
        group_path: vec!["interfaces".into(), definition.owner().as_str().into()],
        scope: match definition.scope() { InterfaceScope::System => EventSourceScope::System, InterfaceScope::Workspace => EventSourceScope::Workspace { workspace_id: actor.current_workspace_id } },
        owner_id: Some(definition.owner().as_str().into()),
        contract_id: format!("interface.{interface_id}.{phase_name}"),
        name: format!("{interface_id} · {phase_name}"),
        description: "Canonical interface lifecycle capability. Create a workspace rule to select this operation and its result.".into(),
        payload_schema: interface_event_schema(phase, capability.input_schema().clone(), capability.output_schema().clone()),
        origin: EventDefinitionOrigin::Core,
        supported_phases: vec![EventTriggerPhase::Published],
    }
}
