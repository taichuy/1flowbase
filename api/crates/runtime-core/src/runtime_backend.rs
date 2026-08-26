use std::{fmt, sync::Arc};

use async_trait::async_trait;
use extension_contracts::provider_contract::{
    ProviderInvocationInput, ProviderInvocationResult, ProviderStreamEvent,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeRequestId(String);

impl RuntimeRequestId {
    pub fn new(value: impl Into<String>) -> Result<Self, RuntimeBackendError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RuntimeBackendError::InvalidRequest(
                "runtime request_id must be non-empty".to_string(),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuntimeRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeTargetId(String);

impl RuntimeTargetId {
    pub fn new(value: impl Into<String>) -> Result<Self, RuntimeBackendError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RuntimeBackendError::InvalidRequest(
                "runtime target_id must be non-empty".to_string(),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeExecutionRequest {
    pub request_id: RuntimeRequestId,
    pub target: RuntimeTargetId,
    pub input: ProviderInvocationInput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeExecutionOutcome {
    pub events: Vec<ProviderStreamEvent>,
    pub result: ProviderInvocationResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackendLifecycle {
    Starting,
    Ready,
    Draining,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRegistrySnapshot {
    pub providers: usize,
    pub data_sources: usize,
    pub capabilities: usize,
    pub network_egress_providers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBackendSnapshot {
    pub backend_kind: String,
    pub lifecycle: RuntimeBackendLifecycle,
    pub registries: RuntimeRegistrySnapshot,
    pub active_request_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCancelOutcome {
    Cancelled,
    NotFound,
}

#[derive(Debug, Error)]
pub enum RuntimeBackendError {
    #[error("invalid runtime request: {0}")]
    InvalidRequest(String),
    #[error("runtime backend slot already has a contribution")]
    DuplicateBackend,
    #[error("runtime backend slot has no contribution")]
    MissingBackend,
    #[error("runtime backend is not accepting execution in state {0:?}")]
    Unavailable(RuntimeBackendLifecycle),
    #[error("runtime request {0} is already active")]
    DuplicateRequest(RuntimeRequestId),
    #[error("runtime request {0} was cancelled")]
    Cancelled(RuntimeRequestId),
    #[error(transparent)]
    Contract(#[from] extension_contracts::error::ExtensionContractError),
    #[error("runtime target {target_id} failed: {message}")]
    Execution { target_id: String, message: String },
}

#[async_trait]
pub trait RuntimeStreamEventSink: Send + Sync {
    async fn emit(&self, event: ProviderStreamEvent) -> Result<(), RuntimeBackendError>;
}

#[derive(Clone, Default)]
pub struct RuntimeStreamSinks {
    pub required: Option<Arc<dyn RuntimeStreamEventSink>>,
    pub diagnostic: Option<Arc<dyn RuntimeStreamEventSink>>,
}

#[async_trait]
pub trait RuntimeExecutionPort: Send + Sync {
    async fn execute(
        &self,
        request: RuntimeExecutionRequest,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError>;

    async fn execute_stream(
        &self,
        request: RuntimeExecutionRequest,
        sinks: RuntimeStreamSinks,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError>;

    async fn cancel(
        &self,
        request_id: &RuntimeRequestId,
    ) -> Result<RuntimeCancelOutcome, RuntimeBackendError>;
}

#[async_trait]
pub trait RuntimeObservationPort: Send + Sync {
    async fn snapshot(&self) -> Result<RuntimeBackendSnapshot, RuntimeBackendError>;
}

pub trait RuntimeBackend: RuntimeExecutionPort + RuntimeObservationPort {}

impl<T> RuntimeBackend for T where T: RuntimeExecutionPort + RuntimeObservationPort {}

#[derive(Default)]
pub struct RuntimeBackendSlot {
    backend: Option<Arc<dyn RuntimeBackend>>,
}

impl RuntimeBackendSlot {
    pub fn bind(&mut self, backend: Arc<dyn RuntimeBackend>) -> Result<(), RuntimeBackendError> {
        if self.backend.is_some() {
            return Err(RuntimeBackendError::DuplicateBackend);
        }
        self.backend = Some(backend);
        Ok(())
    }

    pub fn backend(&self) -> Result<Arc<dyn RuntimeBackend>, RuntimeBackendError> {
        self.backend
            .as_ref()
            .cloned()
            .ok_or(RuntimeBackendError::MissingBackend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeBackend;

    #[async_trait]
    impl RuntimeExecutionPort for FakeBackend {
        async fn execute(
            &self,
            _request: RuntimeExecutionRequest,
        ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
            unreachable!("compile-time adapter fixture is not executed")
        }

        async fn execute_stream(
            &self,
            _request: RuntimeExecutionRequest,
            _sinks: RuntimeStreamSinks,
        ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
            unreachable!("compile-time adapter fixture is not executed")
        }

        async fn cancel(
            &self,
            _request_id: &RuntimeRequestId,
        ) -> Result<RuntimeCancelOutcome, RuntimeBackendError> {
            Ok(RuntimeCancelOutcome::NotFound)
        }
    }

    #[async_trait]
    impl RuntimeObservationPort for FakeBackend {
        async fn snapshot(&self) -> Result<RuntimeBackendSnapshot, RuntimeBackendError> {
            Ok(RuntimeBackendSnapshot {
                backend_kind: "fake_remote_adapter".to_string(),
                lifecycle: RuntimeBackendLifecycle::Ready,
                registries: RuntimeRegistrySnapshot {
                    providers: 0,
                    data_sources: 0,
                    capabilities: 0,
                    network_egress_providers: 0,
                },
                active_request_ids: Vec::new(),
            })
        }
    }

    #[test]
    fn d_010_future_adapter_only_replaces_the_runtime_backend_binding() {
        let mut slot = RuntimeBackendSlot::default();
        slot.bind(Arc::new(FakeBackend)).unwrap();
        assert!(slot.backend().is_ok());
    }

    #[test]
    fn d_001_runtime_backend_slot_rejects_a_second_contribution() {
        let mut slot = RuntimeBackendSlot::default();
        slot.bind(Arc::new(FakeBackend)).unwrap();
        let error = slot.bind(Arc::new(FakeBackend)).unwrap_err();
        assert!(matches!(error, RuntimeBackendError::DuplicateBackend));
    }
}
