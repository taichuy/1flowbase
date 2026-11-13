//! Canonical invocation facts projected into workspace-owned interface rules.
use std::sync::{Arc, Weak};

use control_plane_contracts::ports::EventCenterRepository;
use domain::{ActorContext, event_center::*};
use interface_runtime::{
    CompiledInterfaceRegistry, InterfaceEvent, InterfaceEventCapability, InterfaceEventObserver,
    InterfaceEventObserverFuture, InterfaceEventPrincipal, InterfaceInvocationTerminal,
    InterfaceScope,
};
use serde_json::{Value, json};

use crate::app_state::ApiState;

pub(crate) struct InterfaceRuleObserver(Weak<ApiState>);
impl InterfaceRuleObserver {
    pub(crate) fn new(state: &Arc<ApiState>) -> Arc<Self> {
        Arc::new(Self(Arc::downgrade(state)))
    }
}
impl InterfaceEventObserver for InterfaceRuleObserver {
    fn capture_context(&self) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
        Some(Arc::new(self.0.upgrade()?.store.workflow_ancestor_ids()))
    }
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
            if !state.event_center.configured() {
                return Ok(());
            }
            let interface_id = context.definition().interface_id().as_str();
            let invocation_id = context.invocation_id().value().to_string();
            let (phase, terminal, payload) = envelope(&event);
            observe_application_operation(&state, &event, phase, &payload);
            let definitions = state
                .store
                .list_event_definitions(workspace_id)
                .await
                .map_err(|_| Arc::<str>::from("interface-event-rules-unavailable"))?;
            let Some(ancestors) = context.observer_context::<Vec<uuid::Uuid>>() else {
                return Ok(());
            };
            let ancestors = ancestors.clone();
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
        "invocation_id":context.invocation_id().value().to_string(),
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
    let mut entries = Vec::new();
    for capability in registry.event_capabilities().filter(|capability| {
        capability.definition().scope() != InterfaceScope::System || actor.is_root
    }) {
        for phase in [InterfaceEventPhase::Before, InterfaceEventPhase::Completion] {
            entries.push(capability_entry(capability, actor, phase));
            if let Some(entry) =
                application_operation_entry(capability, actor.current_workspace_id, phase)
            {
                entries.push(entry);
            }
        }
    }
    entries
}

fn application_operation_entry(
    capability: &InterfaceEventCapability,
    workspace_id: uuid::Uuid,
    phase: InterfaceEventPhase,
) -> Option<EventCatalogEntry> {
    use crate::routes::application_public_api::agent_logs::event_projection::OPERATIONS;
    let operation = capability.definition().interface_id().as_str();
    if !OPERATIONS.contains(&operation) {
        return None;
    }
    let phase_name = match phase {
        InterfaceEventPhase::Before => "before",
        InterfaceEventPhase::Completion => "completion",
    };
    let mut schema = interface_event_schema(
        phase,
        capability.input_schema().clone(),
        capability.output_schema().clone(),
    );
    schema["properties"]["application_id"] = json!({"type":"string","format":"uuid"});
    schema["properties"]["workspace_id"] = json!({"type":"string","format":"uuid"});
    schema["required"]
        .as_array_mut()?
        .extend([json!("application_id"), json!("workspace_id")]);
    let mut entry: EventCatalogEntry = super::business::descriptor(
        &format!("host.application.operation.{operation}.{phase_name}"),
        schema,
        EventSourceScope::Workspace { workspace_id },
        "applications",
    )
    .into();
    entry.group_path = vec!["applications".into(), "agent_logs".into()];
    entry.description = "Agent Logs reception/import operation. Completion ends this API invocation, independently of external task completion or native application run status.".into();
    Some(entry)
}

fn observe_application_operation(
    state: &ApiState,
    event: &InterfaceEvent,
    phase: InterfaceEventPhase,
    payload: &Value,
) {
    use crate::routes::application_public_api::agent_logs::event_projection::application_operation_context;
    let input = match event {
        InterfaceEvent::Before { input, .. } => Some(input),
        InterfaceEvent::Completion { input, .. } => input.as_ref(),
    };
    let Some(subject) = application_operation_context(event.context(), input) else {
        return;
    };
    let Some(registry) = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|boot| boot.interface_registry())
        .map(|registry| registry.snapshot())
    else {
        return;
    };
    let Some(capability) = registry.event_capability(event.context().definition().interface_id())
    else {
        return;
    };
    let Some(entry) = application_operation_entry(capability, subject.workspace_id, phase) else {
        return;
    };
    let contract = entry.contract_id;
    let source = super::business::descriptor(
        &contract,
        entry.payload_schema,
        EventSourceScope::Workspace {
            workspace_id: subject.workspace_id,
        },
        "applications",
    );
    let Some(ancestors) = event.context().observer_context::<Vec<uuid::Uuid>>() else {
        return;
    };
    super::business::publish(
        &state.event_center,
        source,
        format!("{}/{:?}", subject.invocation_id.value(), phase),
        (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64,
        ancestors.clone(),
        Default::default(),
        || {
            let mut value = payload.clone();
            value["application_id"] = json!(subject.application_id);
            value["workspace_id"] = json!(subject.workspace_id);
            Some(value)
        },
    );
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
        created_at: None,
        updated_at: None,
        event_kind: None,
        id: None, source: None, enabled: None,
        interface_event: Some(InterfaceEventBinding::legacy(interface_id, phase)),
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
