use serde::{Deserialize, Serialize};

use crate::{ContractDescriptor, DescriptorValueError};

macro_rules! lifecycle_identity {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, DescriptorValueError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(DescriptorValueError::Empty {
                        kind: stringify!($name),
                    });
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

lifecycle_identity!(LifecycleFactId);
lifecycle_identity!(LifecycleTransactionId);
lifecycle_identity!(LifecycleOperationId);
lifecycle_identity!(LifecycleCommandId);
lifecycle_identity!(DiagnosticEventId);

pub trait LifecycleContract: Send + Sync + 'static {
    const CONTRACT_ID: &'static str;
    const CONTRACT_VERSION: &'static str;
}

fn contract_of<T: LifecycleContract>() -> ContractDescriptor {
    ContractDescriptor::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("lifecycle contract identity constants must not be empty")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
#[serde(deny_unknown_fields)]
pub struct AfterCommitFact<T> {
    fact_id: LifecycleFactId,
    transaction_id: LifecycleTransactionId,
    contract: ContractDescriptor,
    occurred_at_unix_ms: i64,
    payload: T,
}

impl<T: LifecycleContract> AfterCommitFact<T> {
    pub fn new(
        fact_id: LifecycleFactId,
        transaction_id: LifecycleTransactionId,
        occurred_at_unix_ms: i64,
        payload: T,
    ) -> Self {
        Self {
            fact_id,
            transaction_id,
            contract: contract_of::<T>(),
            occurred_at_unix_ms,
            payload,
        }
    }

    pub fn fact_id(&self) -> &LifecycleFactId {
        &self.fact_id
    }

    pub fn transaction_id(&self) -> &LifecycleTransactionId {
        &self.transaction_id
    }

    pub fn contract(&self) -> &ContractDescriptor {
        &self.contract
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionTerminal {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
#[serde(deny_unknown_fields)]
pub struct CompletionOutcome<T> {
    operation_id: LifecycleOperationId,
    contract: ContractDescriptor,
    terminal: CompletionTerminal,
    completed_at_unix_ms: i64,
    payload: T,
}

impl<T: LifecycleContract> CompletionOutcome<T> {
    pub fn new(
        operation_id: LifecycleOperationId,
        terminal: CompletionTerminal,
        completed_at_unix_ms: i64,
        payload: T,
    ) -> Self {
        Self {
            operation_id,
            contract: contract_of::<T>(),
            terminal,
            completed_at_unix_ms,
            payload,
        }
    }

    pub fn operation_id(&self) -> &LifecycleOperationId {
        &self.operation_id
    }

    pub fn terminal(&self) -> CompletionTerminal {
        self.terminal
    }

    pub fn contract(&self) -> &ContractDescriptor {
        &self.contract
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
#[serde(deny_unknown_fields)]
pub struct TypedCommand<T> {
    command_id: LifecycleCommandId,
    contract: ContractDescriptor,
    payload: T,
}

impl<T: LifecycleContract> TypedCommand<T> {
    pub fn new(command_id: LifecycleCommandId, payload: T) -> Self {
        Self {
            command_id,
            contract: contract_of::<T>(),
            payload,
        }
    }

    pub fn command_id(&self) -> &LifecycleCommandId {
        &self.command_id
    }

    pub fn contract(&self) -> &ContractDescriptor {
        &self.contract
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
#[serde(deny_unknown_fields)]
pub struct DiagnosticEvent<T> {
    event_id: DiagnosticEventId,
    contract: ContractDescriptor,
    payload: T,
}

impl<T: LifecycleContract> DiagnosticEvent<T> {
    pub fn new(event_id: DiagnosticEventId, payload: T) -> Self {
        Self {
            event_id,
            contract: contract_of::<T>(),
            payload,
        }
    }

    pub fn event_id(&self) -> &DiagnosticEventId {
        &self.event_id
    }

    pub fn contract(&self) -> &ContractDescriptor {
        &self.contract
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }
}
