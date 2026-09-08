//! Finite Create-only worker protocol. Host metadata is outbound context, never worker authority.
use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::extension_bus::{HookPhase, ManagedExecutionIdentity};

pub const MANAGED_HOOK_PROTOCOL_V1: &str = "1flowbase.managed-hook/v1";
pub const MANAGED_HOOK_MAX_FRAME_BYTES: usize = 65_536;

/// The opened Create view deliberately excludes credentials, arbitrary operation enums and patches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedCreateView {
    pub code: String,
    pub template_provider: String,
    pub template_code: String,
    pub template_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedHookTerminal {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedCreateHookInput {
    Authorization { create: ManagedCreateView },
    Admission { create: ManagedCreateView },
    Before { create: ManagedCreateView },
    After { model_id: String },
    Failure { classification: String },
    Completion { terminal: ManagedHookTerminal },
}

impl ManagedCreateHookInput {
    pub fn phase(&self) -> HookPhase {
        match self {
            Self::Authorization { .. } => HookPhase::Authorization,
            Self::Admission { .. } => HookPhase::Admission,
            Self::Before { .. } => HookPhase::Before,
            Self::After { .. } => HookPhase::After,
            Self::Failure { .. } => HookPhase::Failure,
            Self::Completion { .. } => HookPhase::Completion,
        }
    }

    pub fn point_id(&self) -> &'static str {
        match self {
            Self::Authorization { .. } => "1flowbase.model-definitions.create.authorization",
            Self::Admission { .. } => "1flowbase.model-definitions.create.admission",
            Self::Before { .. } => "1flowbase.model-definitions.create.before",
            Self::After { .. } => "1flowbase.model-definitions.create.after",
            Self::Failure { .. } => "1flowbase.model-definitions.create.failure",
            Self::Completion { .. } => "1flowbase.model-definitions.create.completion",
        }
    }

    pub fn is_observer(&self) -> bool {
        matches!(
            self,
            Self::After { .. } | Self::Failure { .. } | Self::Completion { .. }
        )
    }
}

/// Supplied by the trusted invocation adapter, frozen once for the invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedHookInvocation {
    pub invocation_id: String,
    pub registry_fingerprint: String,
    pub graph_fingerprint: String,
    pub authority_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedHookHostContext {
    pub invocation: ManagedHookInvocation,
    pub execution_identity: ManagedExecutionIdentity,
    pub generation: NonZeroU64,
    pub deadline_unix_ms: i64,
    pub actor_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedHookHostFrame {
    pub protocol: String,
    pub call_id: String,
    pub handler: String,
    pub context: ManagedHookHostContext,
    pub input: ManagedCreateHookInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedHookOutcome {
    Continue,
    Deny { classification: String },
    Observed,
    Failed { classification: String },
}

/// Workers can echo correlation, never execution identity, actor, scope, target or patched input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedHookWorkerFrame {
    pub protocol: String,
    pub call_id: String,
    pub phase: HookPhase,
    pub outcome: ManagedHookOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("managed hook protocol violation: {0}")]
pub struct ManagedHookContractError(pub &'static str);

impl ManagedHookHostFrame {
    pub fn validate(&self) -> Result<(), ManagedHookContractError> {
        let invocation = &self.context.invocation;
        if self.protocol != MANAGED_HOOK_PROTOCOL_V1
            || [
                &self.call_id,
                &self.handler,
                &invocation.invocation_id,
                &invocation.registry_fingerprint,
                &invocation.graph_fingerprint,
            ]
            .iter()
            .any(|value| value.trim().is_empty() || value.len() > 512)
            || invocation.authority_revision < 0
            || self.context.deadline_unix_ms <= 0
        {
            return Err(ManagedHookContractError("invalid host context"));
        }
        Ok(())
    }
}

impl ManagedHookWorkerFrame {
    pub fn validate_for(
        &self,
        request: &ManagedHookHostFrame,
    ) -> Result<(), ManagedHookContractError> {
        if self.protocol != MANAGED_HOOK_PROTOCOL_V1
            || self.call_id != request.call_id
            || self.phase != request.input.phase()
        {
            return Err(ManagedHookContractError("uncorrelated response"));
        }
        let valid = match &self.outcome {
            ManagedHookOutcome::Continue => !request.input.is_observer(),
            ManagedHookOutcome::Deny { classification } => {
                !request.input.is_observer() && valid_classification(classification)
            }
            ManagedHookOutcome::Observed => request.input.is_observer(),
            ManagedHookOutcome::Failed { classification } => valid_classification(classification),
        };
        if !valid {
            return Err(ManagedHookContractError(
                "outcome is not permitted for this phase",
            ));
        }
        Ok(())
    }
}

fn valid_classification(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}
