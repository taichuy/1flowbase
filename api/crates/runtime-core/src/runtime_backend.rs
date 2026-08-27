use std::{collections::BTreeSet, fmt, sync::Arc};

use async_trait::async_trait;
use extension_contracts::provider_contract::{
    ProviderAuthOperation, ProviderAuthResult, ProviderBalanceResult, ProviderCompactError,
    ProviderCompactResult, ProviderCountTokensError, ProviderCountTokensInput,
    ProviderCountTokensResult, ProviderInvocationInput, ProviderInvocationResult,
    ProviderModelDescriptor, ProviderResetCreditOperation, ProviderResetCreditResult,
    ProviderStreamEvent, ProviderUsageWindowsResult,
};
use extension_contracts::{
    DataModelTemplateDescriptor, DataSourceCatalogEntry, DataSourceConfigInput,
    DataSourceCreateRecordInput, DataSourceCreateRecordOutput, DataSourceDeleteRecordInput,
    DataSourceDeleteRecordOutput, DataSourceDescribeResourceInput,
    DataSourceExecuteModelOperationInput, DataSourceExecuteSqlInput, DataSourceGetRecordInput,
    DataSourceGetRecordOutput, DataSourceImportSnapshotInput, DataSourceImportSnapshotOutput,
    DataSourceListRecordsInput, DataSourceListRecordsOutput, DataSourcePreviewReadInput,
    DataSourcePreviewReadOutput, DataSourceResourceDescriptor, DataSourceUpdateRecordInput,
    DataSourceUpdateRecordOutput, EgressDescriptor, ForwardProxyLease, NativeSqlExecutionOutput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    #[error("runtime backend does not implement operation {0}")]
    UnsupportedOperation(&'static str),
    #[error(transparent)]
    Contract(#[from] extension_contracts::error::ExtensionContractError),
    #[error(transparent)]
    CountTokens(#[from] ProviderCountTokensError),
    #[error(transparent)]
    Compact(#[from] ProviderCompactError),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLegacyManifestEligibility {
    pub expected_publisher_namespace: String,
    pub expected_versioned_plugin_id: String,
    pub expected_raw_manifest_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeArtifactReference(String);

impl RuntimeArtifactReference {
    pub fn new(value: impl Into<String>) -> Result<Self, RuntimeBackendError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RuntimeBackendError::InvalidRequest(
                "runtime artifact reference must be non-empty".to_string(),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct RuntimePackageActivation {
    pub plugin_id: String,
    pub artifact: RuntimeArtifactReference,
    pub source_identity: Option<String>,
    pub legacy_eligibility: Option<RuntimeLegacyManifestEligibility>,
}

#[derive(Debug, Clone)]
pub struct RuntimeNetworkEgressActivation {
    pub runtime_id: String,
    pub plugin_id: String,
    pub artifact: RuntimeArtifactReference,
    pub source_identity: String,
    pub secret_json: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCapabilityExecutionOutcome {
    pub output_payload: Value,
    pub granted_credit_permissions: BTreeSet<String>,
}

/// Typed RuntimeExtension operations consumed by Backend business paths.
///
/// The exhaustive methods deliberately avoid exposing Host registries. A future Backend adapter
/// implements this Port and is selected only by the composition-root binding.
#[async_trait]
pub trait RuntimeExtensionPort: Send + Sync {
    async fn activate_provider(
        &self,
        _request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "activate_provider",
        ))
    }

    async fn activate_data_source(
        &self,
        _request: RuntimePackageActivation,
    ) -> Result<Vec<DataModelTemplateDescriptor>, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "activate_data_source",
        ))
    }

    async fn activate_capability(
        &self,
        _request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "activate_capability",
        ))
    }

    async fn deactivate_provider(&self, _plugin_id: &str) -> Result<(), RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "deactivate_provider",
        ))
    }

    async fn deactivate_data_source(&self, _plugin_id: &str) -> Result<(), RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "deactivate_data_source",
        ))
    }

    async fn deactivate_capability(&self, _plugin_id: &str) -> Result<(), RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "deactivate_capability",
        ))
    }

    async fn provider_validate(
        &self,
        _target_id: &str,
        _provider_config: Value,
    ) -> Result<Value, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "provider_validate",
        ))
    }

    async fn provider_authenticate(
        &self,
        _target_id: &str,
        _provider_config: Value,
        _operation: ProviderAuthOperation,
    ) -> Result<ProviderAuthResult, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "provider_authenticate",
        ))
    }

    async fn provider_list_models(
        &self,
        _target_id: &str,
        _provider_config: Value,
    ) -> Result<Vec<ProviderModelDescriptor>, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "provider_list_models",
        ))
    }

    async fn provider_get_balance(
        &self,
        _target_id: &str,
        _provider_config: Value,
    ) -> Result<ProviderBalanceResult, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "provider_get_balance",
        ))
    }

    async fn provider_get_usage_windows(
        &self,
        _target_id: &str,
        _provider_config: Value,
    ) -> Result<ProviderUsageWindowsResult, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "provider_get_usage_windows",
        ))
    }

    async fn provider_reset_credit(
        &self,
        _target_id: &str,
        _provider_config: Value,
        _operation: ProviderResetCreditOperation,
    ) -> Result<ProviderResetCreditResult, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "provider_reset_credit",
        ))
    }

    async fn provider_count_tokens(
        &self,
        _target_id: &str,
        _input: ProviderCountTokensInput,
    ) -> Result<ProviderCountTokensResult, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "provider_count_tokens",
        ))
    }

    async fn provider_compact(
        &self,
        _target_id: &str,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderCompactResult, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "provider_compact",
        ))
    }

    async fn data_source_validate(
        &self,
        _target_id: &str,
        _input: DataSourceConfigInput,
    ) -> Result<Value, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_validate",
        ))
    }

    async fn data_source_test_connection(
        &self,
        _target_id: &str,
        _input: DataSourceConfigInput,
    ) -> Result<Value, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_test_connection",
        ))
    }

    async fn data_source_discover_catalog(
        &self,
        _target_id: &str,
        _input: DataSourceConfigInput,
    ) -> Result<Vec<DataSourceCatalogEntry>, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_discover_catalog",
        ))
    }

    async fn data_source_describe_resource(
        &self,
        _target_id: &str,
        _input: DataSourceDescribeResourceInput,
    ) -> Result<DataSourceResourceDescriptor, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_describe_resource",
        ))
    }

    async fn data_source_preview_read(
        &self,
        _target_id: &str,
        _input: DataSourcePreviewReadInput,
    ) -> Result<DataSourcePreviewReadOutput, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_preview_read",
        ))
    }

    async fn data_source_import_snapshot(
        &self,
        _target_id: &str,
        _input: DataSourceImportSnapshotInput,
    ) -> Result<DataSourceImportSnapshotOutput, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_import_snapshot",
        ))
    }

    async fn data_source_list_records(
        &self,
        _target_id: &str,
        _input: DataSourceListRecordsInput,
    ) -> Result<DataSourceListRecordsOutput, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_list_records",
        ))
    }

    async fn data_source_get_record(
        &self,
        _target_id: &str,
        _input: DataSourceGetRecordInput,
    ) -> Result<DataSourceGetRecordOutput, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_get_record",
        ))
    }

    async fn data_source_create_record(
        &self,
        _target_id: &str,
        _input: DataSourceCreateRecordInput,
    ) -> Result<DataSourceCreateRecordOutput, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_create_record",
        ))
    }

    async fn data_source_update_record(
        &self,
        _target_id: &str,
        _input: DataSourceUpdateRecordInput,
    ) -> Result<DataSourceUpdateRecordOutput, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_update_record",
        ))
    }

    async fn data_source_delete_record(
        &self,
        _target_id: &str,
        _input: DataSourceDeleteRecordInput,
    ) -> Result<DataSourceDeleteRecordOutput, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_delete_record",
        ))
    }

    async fn data_source_execute_sql(
        &self,
        _target_id: &str,
        _input: DataSourceExecuteSqlInput,
    ) -> Result<NativeSqlExecutionOutput, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_execute_sql",
        ))
    }

    async fn data_source_execute_model_operation(
        &self,
        _target_id: &str,
        _input: DataSourceExecuteModelOperationInput,
    ) -> Result<Value, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "data_source_execute_model_operation",
        ))
    }

    async fn capability_validate(
        &self,
        _target_id: &str,
        _contribution_code: &str,
        _config_payload: Value,
    ) -> Result<Value, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "capability_validate",
        ))
    }

    async fn capability_resolve_dynamic_options(
        &self,
        _target_id: &str,
        _contribution_code: &str,
        _config_payload: Value,
    ) -> Result<Value, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "capability_resolve_dynamic_options",
        ))
    }

    async fn capability_resolve_output_schema(
        &self,
        _target_id: &str,
        _contribution_code: &str,
        _config_payload: Value,
    ) -> Result<Value, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "capability_resolve_output_schema",
        ))
    }

    async fn capability_execute(
        &self,
        _target_id: &str,
        _contribution_code: &str,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<RuntimeCapabilityExecutionOutcome, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "capability_execute",
        ))
    }

    async fn network_egress_preflight(
        &self,
        _request: RuntimeNetworkEgressActivation,
    ) -> Result<(), RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "network_egress_preflight",
        ))
    }

    async fn network_egress_activate(
        &self,
        _request: RuntimeNetworkEgressActivation,
    ) -> Result<(), RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "network_egress_activate",
        ))
    }

    async fn network_egress_sync(
        &self,
        _runtime_id: &str,
    ) -> Result<Vec<EgressDescriptor>, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "network_egress_sync",
        ))
    }

    async fn network_egress_resolve_http_forward_proxy(
        &self,
        _runtime_id: &str,
        _egress_key: &str,
    ) -> Result<ForwardProxyLease, RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "network_egress_resolve_http_forward_proxy",
        ))
    }

    async fn network_egress_release_http_forward_proxy(
        &self,
        _runtime_id: &str,
        _lease_id: &str,
    ) -> Result<(), RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "network_egress_release_http_forward_proxy",
        ))
    }

    async fn network_egress_deactivate(
        &self,
        _runtime_id: &str,
    ) -> Result<(), RuntimeBackendError> {
        Err(RuntimeBackendError::UnsupportedOperation(
            "network_egress_deactivate",
        ))
    }
}

pub trait RuntimeBackend:
    RuntimeExecutionPort + RuntimeObservationPort + RuntimeExtensionPort
{
}

impl<T> RuntimeBackend for T where
    T: RuntimeExecutionPort + RuntimeObservationPort + RuntimeExtensionPort
{
}

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

    impl RuntimeExtensionPort for FakeBackend {}

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
