use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::provider_contract::{
    ProviderCompactProfile, ProviderCompactResult, ProviderCountTokensResult, ProviderWireOperation,
};

pub const COMPACT_OPERATION_TERMINAL_KIND: &str = "compact";
pub const COUNT_TOKENS_TERMINAL_KIND: &str = "count_tokens";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountTokensReceipt {
    result: ProviderCountTokensResult,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeOperationTerminal {
    CountTokens(CountTokensReceipt),
    Compact(CompactOperationReceipt),
}

impl NativeOperationTerminal {
    pub fn from_payload(payload: &Value) -> Result<Option<Self>> {
        match payload.get("semantic_terminal").and_then(Value::as_str) {
            Some(COUNT_TOKENS_TERMINAL_KIND) => CountTokensReceipt::from_payload(payload)
                .map(Self::CountTokens)
                .map(Some),
            Some(COMPACT_OPERATION_TERMINAL_KIND) => CompactOperationReceipt::from_payload(payload)
                .map(Self::Compact)
                .map(Some),
            _ => Ok(None),
        }
    }

    pub fn as_payload(&self) -> Result<Value> {
        match self {
            Self::CountTokens(receipt) => receipt.as_payload(),
            Self::Compact(receipt) => receipt.as_payload(),
        }
    }
}

impl Serialize for NativeOperationTerminal {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.as_payload()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NativeOperationTerminal {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let payload = Value::deserialize(deserializer)?;
        Self::from_payload(&payload)
            .map_err(serde::de::Error::custom)?
            .ok_or_else(|| serde::de::Error::custom("payload is not a Native operation terminal"))
    }
}

impl CountTokensReceipt {
    pub fn new(result: ProviderCountTokensResult) -> Result<Self> {
        if result.operation != ProviderWireOperation::CountTokens {
            bail!("CountTokens receipt requires a typed CountTokens provider result");
        }
        Ok(Self { result })
    }

    pub fn input_tokens(&self) -> u64 {
        self.result.input_tokens
    }

    pub fn result(&self) -> &ProviderCountTokensResult {
        &self.result
    }

    pub fn as_payload(&self) -> Result<Value> {
        Ok(json!({
            "semantic_terminal": COUNT_TOKENS_TERMINAL_KIND,
            "result": serde_json::to_value(&self.result)
                .map_err(|error| anyhow!("could not serialize CountTokens receipt: {error}"))?,
        }))
    }

    pub fn from_payload(payload: &Value) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Payload {
            semantic_terminal: String,
            result: ProviderCountTokensResult,
        }

        let payload: Payload = serde_json::from_value(payload.clone())
            .map_err(|error| anyhow!("invalid CountTokens receipt payload: {error}"))?;
        if payload.semantic_terminal != COUNT_TOKENS_TERMINAL_KIND {
            bail!("invalid CountTokens semantic terminal");
        }
        Self::new(payload.result)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompactOperationReceipt {
    result: ProviderCompactResult,
}

impl CompactOperationReceipt {
    pub fn from_provider_result(result: ProviderCompactResult) -> Result<Self> {
        if !result.satisfies_profile(result.profile()) {
            bail!("Compact receipt requires a typed provider Compact result");
        }
        Ok(Self { result })
    }

    pub fn profile(&self) -> ProviderCompactProfile {
        self.result.profile()
    }

    pub fn result(&self) -> &ProviderCompactResult {
        &self.result
    }

    pub fn as_payload(&self) -> Result<Value> {
        Ok(json!({
            "semantic_terminal": COMPACT_OPERATION_TERMINAL_KIND,
            "result": serde_json::to_value(&self.result)
                .map_err(|error| anyhow!("could not serialize Compact receipt: {error}"))?,
        }))
    }

    pub fn from_payload(payload: &Value) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Payload {
            semantic_terminal: String,
            result: ProviderCompactResult,
        }

        let payload: Payload = serde_json::from_value(payload.clone())
            .map_err(|error| anyhow!("invalid Compact receipt payload: {error}"))?;
        if payload.semantic_terminal != COMPACT_OPERATION_TERMINAL_KIND {
            bail!("invalid Compact semantic terminal");
        }
        Self::from_provider_result(payload.result)
    }
}

pub fn count_tokens_input_tokens_from_output_payload(payload: &Value) -> Result<Option<i64>> {
    let Some(terminal) = NativeOperationTerminal::from_payload(payload)? else {
        return Ok(None);
    };
    let NativeOperationTerminal::CountTokens(receipt) = terminal else {
        return Ok(None);
    };
    Ok(Some(i64::try_from(receipt.input_tokens()).map_err(
        |_| anyhow!("CountTokens result exceeds the application log numeric range"),
    )?))
}
