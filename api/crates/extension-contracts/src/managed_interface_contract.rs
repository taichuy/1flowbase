//! One bounded lifecycle exchange for every registered canonical business interface.
//! A schema describes an explicitly safe projection, never permission to serialize a raw DTO.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    extension_bus::HookPhase, ManagedHookHostContext, ManagedHookOutcome, ManagedHookTerminal,
};

pub const MANAGED_INTERFACE_PROTOCOL_V1: &str = "1flowbase.managed-interface/v1";
pub const MANAGED_INTERFACE_MAX_FRAME_BYTES: usize = 65_536;
pub const MANAGED_INTERFACE_MAX_SCHEMA_BYTES: usize = 32_768;
pub const MANAGED_INTERFACE_MAX_DEPTH: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("managed interface contract violation: {0}")]
pub struct ManagedInterfaceContractError(pub &'static str);

type ContractResult<T = ()> = Result<T, ManagedInterfaceContractError>;

/// JSON Schema using a closed, bounded vocabulary; validation delegates to the standard engine.
/// References and unknown keywords are rejected before compilation, so no remote resolution occurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedProjectionContract {
    pub contract_id: String,
    pub contract_version: String,
    pub schema: Value,
}

impl ManagedProjectionContract {
    pub fn compile(&self) -> ContractResult<CompiledManagedProjection> {
        if !identity(&self.contract_id) || !identity(&self.contract_version) {
            return Err(ManagedInterfaceContractError("invalid projection identity"));
        }
        if serde_json::to_vec(&self.schema)
            .map_err(|_| invalid_schema())?
            .len()
            > MANAGED_INTERFACE_MAX_SCHEMA_BYTES
        {
            return Err(invalid_schema());
        }
        validate_schema_shape(&self.schema, 0)?;
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&self.schema)
            .map_err(|_| invalid_schema())?;
        let mut canonical = self.schema.clone();
        canonical.sort_all_objects();
        let fingerprint = format!(
            "sha256:{:x}",
            Sha256::digest(
                serde_json::to_vec(&(&self.contract_id, &self.contract_version, canonical,))
                    .map_err(|_| invalid_schema())?
            )
        );
        Ok(CompiledManagedProjection {
            validator,
            fingerprint,
        })
    }
}

pub struct CompiledManagedProjection {
    validator: jsonschema::Validator,
    fingerprint: String,
}

impl CompiledManagedProjection {
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn validate(&self, value: &Value) -> ContractResult {
        if !bounded_value(value, 0)
            || serde_json::to_vec(value)
                .map_err(|_| invalid_value())?
                .len()
                > MANAGED_INTERFACE_MAX_FRAME_BYTES
            || !self.validator.is_valid(value)
        {
            return Err(invalid_value());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedInterfaceView {
    pub contract: ManagedProjectionContract,
    pub value: Value,
}

impl ManagedInterfaceView {
    pub fn validate(&self) -> ContractResult {
        self.contract.compile()?.validate(&self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedInterfaceInput {
    Authorization { input: ManagedInterfaceView },
    Admission { input: ManagedInterfaceView },
    Before { input: ManagedInterfaceView },
    After { output: ManagedInterfaceView },
    Failure { classification: String },
    Completion { terminal: ManagedHookTerminal },
}

impl ManagedInterfaceInput {
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

    pub fn validate(&self) -> ContractResult {
        match self {
            Self::Authorization { input } | Self::Admission { input } | Self::Before { input } => {
                input.validate()
            }
            Self::After { output } => output.validate(),
            Self::Failure { classification } if !classification_code(classification) => {
                Err(invalid_value())
            }
            Self::Failure { .. } | Self::Completion { .. } => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedInterfaceHostFrame {
    pub protocol: String,
    pub call_id: String,
    pub handler: String,
    pub interface_id: String,
    pub interface_version: String,
    pub context: ManagedHookHostContext,
    pub input: ManagedInterfaceInput,
}

impl ManagedInterfaceHostFrame {
    pub fn validate(&self) -> ContractResult {
        let invocation = &self.context.invocation;
        if self.protocol != MANAGED_INTERFACE_PROTOCOL_V1
            || [
                &self.call_id,
                &self.handler,
                &self.interface_id,
                &self.interface_version,
                &invocation.invocation_id,
                &invocation.registry_fingerprint,
                &invocation.graph_fingerprint,
            ]
            .iter()
            .any(|s| !identity(s))
            || invocation.authority_revision < 0
            || self.context.deadline_unix_ms <= 0
            || serde_json::to_vec(self).map_err(|_| invalid_value())?.len()
                > MANAGED_INTERFACE_MAX_FRAME_BYTES
        {
            return Err(ManagedInterfaceContractError(
                "invalid host context or frame budget",
            ));
        }
        self.input.validate()
    }
}

/// The response has no actor, workspace, execution identity, patched input or replacement result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedInterfaceWorkerFrame {
    pub protocol: String,
    pub call_id: String,
    pub phase: HookPhase,
    pub outcome: ManagedHookOutcome,
}

impl ManagedInterfaceWorkerFrame {
    pub fn validate_for(&self, request: &ManagedInterfaceHostFrame) -> ContractResult {
        if self.protocol != MANAGED_INTERFACE_PROTOCOL_V1
            || self.call_id != request.call_id
            || self.phase != request.input.phase()
        {
            return Err(ManagedInterfaceContractError("uncorrelated response"));
        }
        let veto_phase = matches!(self.phase, HookPhase::Authorization | HookPhase::Admission);
        let permitted = match &self.outcome {
            ManagedHookOutcome::Continue => veto_phase,
            ManagedHookOutcome::Deny { classification } => {
                veto_phase && classification_code(classification)
            }
            ManagedHookOutcome::Observed => !veto_phase,
            ManagedHookOutcome::Failed { classification } => classification_code(classification),
        };
        if !permitted {
            return Err(ManagedInterfaceContractError(
                "outcome is not permitted for this phase",
            ));
        }
        Ok(())
    }
}

fn invalid_schema() -> ManagedInterfaceContractError {
    ManagedInterfaceContractError("schema must be closed and bounded")
}
fn invalid_value() -> ManagedInterfaceContractError {
    ManagedInterfaceContractError("projection violates schema or resource budget")
}
fn identity(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 512
}
fn classification_code(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}
fn bounded_value(value: &Value, depth: usize) -> bool {
    if depth > MANAGED_INTERFACE_MAX_DEPTH {
        return false;
    }
    match value {
        Value::Array(a) => a.len() <= 256 && a.iter().all(|v| bounded_value(v, depth + 1)),
        Value::Object(o) => {
            o.len() <= 128
                && o.iter()
                    .all(|(k, v)| k.len() <= 128 && bounded_value(v, depth + 1))
        }
        Value::String(s) => s.len() <= 16_384,
        _ => true,
    }
}
fn validate_schema_shape(schema: &Value, depth: usize) -> ContractResult {
    if depth > MANAGED_INTERFACE_MAX_DEPTH {
        return Err(invalid_schema());
    }
    let object = schema.as_object().ok_or_else(invalid_schema)?;
    const KEYS: &[&str] = &[
        "type",
        "properties",
        "required",
        "additionalProperties",
        "items",
        "maxItems",
        "minItems",
        "maxLength",
        "minLength",
        "enum",
        "const",
        "oneOf",
        "anyOf",
        "minimum",
        "maximum",
    ];
    if object.keys().any(|k| !KEYS.contains(&k.as_str())) {
        return Err(invalid_schema());
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(branches) = object.get(key) {
            let branches = branches.as_array().ok_or_else(invalid_schema)?;
            if object.len() != 1 || branches.is_empty() || branches.len() > 128 {
                return Err(invalid_schema());
            }
            for branch in branches {
                validate_schema_shape(branch, depth + 1)?;
            }
            return Ok(());
        }
    }
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(invalid_schema)?;
    let applicable: &[&str] = match kind {
        "object" => &[
            "type",
            "properties",
            "required",
            "additionalProperties",
            "enum",
            "const",
        ],
        "array" => &["type", "items", "maxItems", "minItems", "enum", "const"],
        "string" => &["type", "maxLength", "minLength", "enum", "const"],
        "integer" | "number" => &["type", "minimum", "maximum", "enum", "const"],
        "null" | "boolean" => &["type", "enum", "const"],
        _ => return Err(invalid_schema()),
    };
    // JSON Schema compiles even inapplicable applicators. Do not let e.g. `items` on a
    // string conceal a reference outside the recursively checked schema vocabulary.
    if object.keys().any(|key| !applicable.contains(&key.as_str())) {
        return Err(invalid_schema());
    }
    match kind {
        "object" => {
            if object.get("additionalProperties") != Some(&Value::Bool(false)) {
                return Err(invalid_schema());
            }
            let properties = object
                .get("properties")
                .and_then(Value::as_object)
                .ok_or_else(invalid_schema)?;
            if properties.len() > 128 {
                return Err(invalid_schema());
            }
            for (name, child) in properties {
                if name.len() > 128 {
                    return Err(invalid_schema());
                }
                validate_schema_shape(child, depth + 1)?;
            }
        }
        "array" => {
            if !object
                .get("maxItems")
                .and_then(Value::as_u64)
                .is_some_and(|n| n <= 256)
            {
                return Err(invalid_schema());
            }
            validate_schema_shape(object.get("items").ok_or_else(invalid_schema)?, depth + 1)?;
        }
        "string" => {
            if !object
                .get("maxLength")
                .and_then(Value::as_u64)
                .is_some_and(|n| n <= 16_384)
            {
                return Err(invalid_schema());
            }
        }
        "null" | "boolean" | "integer" | "number" => {}
        _ => return Err(invalid_schema()),
    }
    Ok(())
}
