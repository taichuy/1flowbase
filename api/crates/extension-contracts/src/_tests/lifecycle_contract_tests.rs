use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::{
    AfterCommitFact, CompletionOutcome, CompletionTerminal, ContractDescriptor, DiagnosticEvent,
    DiagnosticEventId, HookHandlerContract, HookMutationCapability, HookPhase, HookPointContract,
    LifecycleCommandId, LifecycleContract, LifecycleFactId, LifecycleOperationId,
    LifecycleTransactionId, TypedCommand,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ModelDefinitionCommitted {
    definition_id: String,
}

impl LifecycleContract for ModelDefinitionCommitted {
    const CONTRACT_ID: &'static str = "model_definition.committed";
    const CONTRACT_VERSION: &'static str = "v1";
}

fn contract(id: &str) -> ContractDescriptor {
    ContractDescriptor::new(id, "v1").unwrap()
}

#[test]
fn lcf_001_hook_contract_is_typed_and_phase_checked() {
    let point = HookPointContract {
        context: contract("model_definition.create.context"),
        decision: Some(contract("model_definition.create.admission")),
        phase: HookPhase::Admission,
        timeout_ms: NonZeroU64::new(1_000).unwrap(),
        mutation: HookMutationCapability::AmendTypedContext,
    };
    point.validate().unwrap();

    let handler = HookHandlerContract {
        context: point.context.clone(),
        decision: point.decision.clone(),
        phase: point.phase,
    };
    assert!(handler.matches_point(&point));

    let invalid_terminal = HookPointContract {
        phase: HookPhase::Completion,
        decision: None,
        mutation: HookMutationCapability::SelectTypedTarget,
        ..point
    };
    assert!(invalid_terminal.validate().is_err());
}

#[test]
fn lcf_006_lcf_007_facts_outcomes_commands_and_diagnostics_are_distinct_typed_contracts() {
    let payload = ModelDefinitionCommitted {
        definition_id: "model-1".to_string(),
    };
    let fact = AfterCommitFact::new(
        LifecycleFactId::new("fact-1").unwrap(),
        LifecycleTransactionId::new("tx-1").unwrap(),
        1_000,
        payload.clone(),
    );
    assert_eq!(
        fact.contract().contract_id.as_str(),
        "model_definition.committed"
    );

    let outcome = CompletionOutcome::new(
        LifecycleOperationId::new("operation-1").unwrap(),
        CompletionTerminal::Succeeded,
        1_001,
        payload.clone(),
    );
    assert_eq!(outcome.terminal(), CompletionTerminal::Succeeded);

    let command = TypedCommand::new(
        LifecycleCommandId::new("command-1").unwrap(),
        payload.clone(),
    );
    let diagnostic = DiagnosticEvent::new(DiagnosticEventId::new("diagnostic-1").unwrap(), payload);
    assert_ne!(
        command.command_id().as_str(),
        diagnostic.event_id().as_str()
    );
}
