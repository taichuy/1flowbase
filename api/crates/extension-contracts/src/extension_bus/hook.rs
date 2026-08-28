use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::ContractDescriptor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookPhase {
    Authorization,
    Admission,
    Before,
    After,
    Failure,
    Completion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookMutationCapability {
    ObserveOnly,
    AmendTypedContext,
    SelectTypedTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookPointContract {
    pub context: ContractDescriptor,
    pub decision: Option<ContractDescriptor>,
    pub phase: HookPhase,
    pub timeout_ms: NonZeroU64,
    pub mutation: HookMutationCapability,
}

impl HookPointContract {
    pub fn validate(&self) -> Result<(), HookContractError> {
        let requires_decision =
            matches!(self.phase, HookPhase::Authorization | HookPhase::Admission);
        if requires_decision != self.decision.is_some() {
            return Err(HookContractError::DecisionPhaseMismatch { phase: self.phase });
        }
        if matches!(
            self.phase,
            HookPhase::After | HookPhase::Failure | HookPhase::Completion
        ) && self.mutation != HookMutationCapability::ObserveOnly
        {
            return Err(HookContractError::TerminalMutation { phase: self.phase });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookHandlerContract {
    pub context: ContractDescriptor,
    pub decision: Option<ContractDescriptor>,
    pub phase: HookPhase,
}

impl HookHandlerContract {
    pub fn matches_point(&self, point: &HookPointContract) -> bool {
        self.context == point.context
            && self.decision == point.decision
            && self.phase == point.phase
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HookContractError {
    #[error("hook phase {phase:?} has an invalid decision contract")]
    DecisionPhaseMismatch { phase: HookPhase },
    #[error("terminal hook phase {phase:?} cannot mutate invocation state")]
    TerminalMutation { phase: HookPhase },
}
