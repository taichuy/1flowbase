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
    ProviderDistributionInvocation, ProviderDistributionSelectionReceipt,
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
pub struct RuntimeExecutionPrincipal {
    pub workspace_id: String,
    pub actor_id: Option<String>,
    pub deadline_unix_ms: i64,
}

#[derive(Debug, Clone)]
pub struct RuntimeExecutionRequest {
    pub request_id: RuntimeRequestId,
    pub target: RuntimeTargetId,
    pub input: ProviderInvocationInput,
    pub principal: Option<RuntimeExecutionPrincipal>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeExecutionOutcome {
    pub events: Vec<ProviderStreamEvent>,
    pub result: ProviderInvocationResult,
}

#[derive(Debug, Clone)]
pub struct RuntimeProviderDistributionRequest {
    pub request_id: RuntimeRequestId,
    pub target: RuntimeTargetId,
    pub invocation: ProviderDistributionInvocation,
    pub principal: RuntimeExecutionPrincipal,
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
    Contract(Box<extension_contracts::error::ExtensionContractError>),
    #[error(transparent)]
    CountTokens(Box<ProviderCountTokensError>),
    #[error(transparent)]
    Compact(Box<ProviderCompactError>),
    #[error("runtime target {target_id} failed: {message}")]
    Execution { target_id: String, message: String },
}

impl From<extension_contracts::error::ExtensionContractError> for RuntimeBackendError {
    fn from(error: extension_contracts::error::ExtensionContractError) -> Self {
        Self::Contract(Box::new(error))
    }
}

impl From<ProviderCountTokensError> for RuntimeBackendError {
    fn from(error: ProviderCountTokensError) -> Self {
        Self::CountTokens(Box::new(error))
    }
}

impl From<ProviderCompactError> for RuntimeBackendError {
    fn from(error: ProviderCompactError) -> Self {
        Self::Compact(Box::new(error))
    }
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
    async fn activate_provider_distribution_rule(
        &self,
        request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError>;

    async fn deactivate_provider_distribution_rule(
        &self,
        plugin_id: &str,
    ) -> Result<(), RuntimeBackendError>;

    async fn select_provider_distribution(
        &self,
        request: RuntimeProviderDistributionRequest,
    ) -> Result<ProviderDistributionSelectionReceipt, RuntimeBackendError>;

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

/// Trusted installation/activation input. The composition owner supplies current authority;
/// the Runtime Host binds this identity to verified artifact bytes and a concrete generation.
#[derive(Debug, Clone)]
pub struct RuntimeManagedActivation {
    pub plugin_id: String,
    pub artifact: RuntimeArtifactReference,
    pub identity: extension_contracts::extension_bus::ManagedExecutionIdentity,
}

/// Execution whose generation admission has already completed. Dropping cancels its scope lease.
pub type AdmittedManagedExecution = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Value, RuntimeBackendError>> + Send + 'static>,
>;

/// An already-admitted typed Hook execution. Dropping cancels the worker and its mount lease.
pub type AdmittedManagedHook = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<extension_contracts::ManagedHookOutcome, RuntimeBackendError>,
            > + Send
            + 'static,
    >,
>;

/// Trusted invocation input. Runtime Host supplies the worker correlation and bound identity.
#[derive(Debug, Clone)]
pub struct RuntimeManagedHookRequest {
    pub handle: extension_contracts::extension_bus::ManagedExecutionHandle,
    pub principal: RuntimeExecutionPrincipal,
    pub invocation: extension_contracts::ManagedHookInvocation,
    pub input: RuntimeManagedHookInput,
}

/// Existing versioned Create peers remain a regression consumer; new canonical registrations
/// select an explicit interface protocol. No variant lets a worker supply trusted host identity.
#[derive(Debug, Clone)]
pub enum RuntimeManagedHookInput {
    LegacyCreate(extension_contracts::ManagedCreateHookInput),
    Interface {
        interface_id: String,
        interface_version: String,
        input: extension_contracts::ManagedInterfaceInput,
    },
    InterfaceReference {
        interface_id: String,
        interface_version: String,
        input: extension_contracts::ManagedInterfaceReferenceInput,
    },
}

impl From<extension_contracts::ManagedCreateHookInput> for RuntimeManagedHookInput {
    fn from(input: extension_contracts::ManagedCreateHookInput) -> Self {
        Self::LegacyCreate(input)
    }
}

pub type AdmittedManagedEvent = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<extension_contracts::ManagedEventOutcome, RuntimeBackendError>,
            > + Send
            + 'static,
    >,
>;

/// Host-bound subscriber service identity. No original actor is delegated to this request.
#[derive(Debug, Clone)]
pub struct RuntimeManagedEventRequest {
    pub handle: extension_contracts::extension_bus::ManagedExecutionHandle,
    pub graph_fingerprint: String,
    pub authority_revision: i64,
    pub deadline_unix_ms: i64,
    pub delivery: extension_contracts::ManagedEventDelivery,
}

/// Host-only request. Neither the handle nor principal is taken from worker JSON.
#[derive(Debug, Clone)]
pub struct RuntimeManagedCapabilityRequest {
    pub handle: extension_contracts::extension_bus::ManagedExecutionHandle,
    pub principal: RuntimeExecutionPrincipal,
    pub config_payload: Value,
    pub input_payload: Value,
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

#[async_trait]
/// Provider operations are mandatory for every Runtime Backend.
///
/// ```compile_fail
/// use runtime_core::runtime_backend::ProviderRuntimePort;
/// struct IncompleteProviderBackend;
/// impl ProviderRuntimePort for IncompleteProviderBackend {}
/// ```
pub trait ProviderRuntimePort: Send + Sync {
    async fn activate_provider(
        &self,
        request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError>;
    async fn deactivate_provider(&self, plugin_id: &str) -> Result<(), RuntimeBackendError>;

    async fn provider_validate(
        &self,
        target_id: &str,
        provider_config: Value,
    ) -> Result<Value, RuntimeBackendError>;

    async fn provider_authenticate(
        &self,
        target_id: &str,
        provider_config: Value,
        operation: ProviderAuthOperation,
    ) -> Result<ProviderAuthResult, RuntimeBackendError>;

    async fn provider_list_models(
        &self,
        target_id: &str,
        provider_config: Value,
    ) -> Result<Vec<ProviderModelDescriptor>, RuntimeBackendError>;

    async fn provider_get_balance(
        &self,
        target_id: &str,
        provider_config: Value,
    ) -> Result<ProviderBalanceResult, RuntimeBackendError>;

    async fn provider_get_usage_windows(
        &self,
        target_id: &str,
        provider_config: Value,
    ) -> Result<ProviderUsageWindowsResult, RuntimeBackendError>;

    async fn provider_reset_credit(
        &self,
        target_id: &str,
        provider_config: Value,
        operation: ProviderResetCreditOperation,
    ) -> Result<ProviderResetCreditResult, RuntimeBackendError>;

    async fn provider_count_tokens(
        &self,
        target_id: &str,
        input: ProviderCountTokensInput,
    ) -> Result<ProviderCountTokensResult, RuntimeBackendError>;

    async fn provider_compact(
        &self,
        target_id: &str,
        input: ProviderInvocationInput,
    ) -> Result<ProviderCompactResult, RuntimeBackendError>;
}

#[async_trait]
/// Data Source operations are mandatory for every Runtime Backend.
///
/// ```compile_fail
/// use runtime_core::runtime_backend::DataSourceRuntimePort;
/// struct IncompleteDataSourceBackend;
/// impl DataSourceRuntimePort for IncompleteDataSourceBackend {}
/// ```
pub trait DataSourceRuntimePort: Send + Sync {
    async fn activate_data_source(
        &self,
        request: RuntimePackageActivation,
    ) -> Result<Vec<DataModelTemplateDescriptor>, RuntimeBackendError>;
    async fn deactivate_data_source(&self, plugin_id: &str) -> Result<(), RuntimeBackendError>;
    async fn data_source_validate(
        &self,
        target_id: &str,
        input: DataSourceConfigInput,
    ) -> Result<Value, RuntimeBackendError>;

    async fn data_source_test_connection(
        &self,
        target_id: &str,
        input: DataSourceConfigInput,
    ) -> Result<Value, RuntimeBackendError>;

    async fn data_source_discover_catalog(
        &self,
        target_id: &str,
        input: DataSourceConfigInput,
    ) -> Result<Vec<DataSourceCatalogEntry>, RuntimeBackendError>;

    async fn data_source_describe_resource(
        &self,
        target_id: &str,
        input: DataSourceDescribeResourceInput,
    ) -> Result<DataSourceResourceDescriptor, RuntimeBackendError>;

    async fn data_source_preview_read(
        &self,
        target_id: &str,
        input: DataSourcePreviewReadInput,
    ) -> Result<DataSourcePreviewReadOutput, RuntimeBackendError>;

    async fn data_source_import_snapshot(
        &self,
        target_id: &str,
        input: DataSourceImportSnapshotInput,
    ) -> Result<DataSourceImportSnapshotOutput, RuntimeBackendError>;

    async fn data_source_list_records(
        &self,
        target_id: &str,
        input: DataSourceListRecordsInput,
    ) -> Result<DataSourceListRecordsOutput, RuntimeBackendError>;

    async fn data_source_get_record(
        &self,
        target_id: &str,
        input: DataSourceGetRecordInput,
    ) -> Result<DataSourceGetRecordOutput, RuntimeBackendError>;

    async fn data_source_create_record(
        &self,
        target_id: &str,
        input: DataSourceCreateRecordInput,
    ) -> Result<DataSourceCreateRecordOutput, RuntimeBackendError>;

    async fn data_source_update_record(
        &self,
        target_id: &str,
        input: DataSourceUpdateRecordInput,
    ) -> Result<DataSourceUpdateRecordOutput, RuntimeBackendError>;

    async fn data_source_delete_record(
        &self,
        target_id: &str,
        input: DataSourceDeleteRecordInput,
    ) -> Result<DataSourceDeleteRecordOutput, RuntimeBackendError>;

    async fn data_source_execute_sql(
        &self,
        target_id: &str,
        input: DataSourceExecuteSqlInput,
    ) -> Result<NativeSqlExecutionOutput, RuntimeBackendError>;

    async fn data_source_execute_model_operation(
        &self,
        target_id: &str,
        input: DataSourceExecuteModelOperationInput,
    ) -> Result<Value, RuntimeBackendError>;
}

/// Closes exact runtime generations until dropped. Dropping after timeout/cancellation reopens
/// only still-mounted scopes; disposal cannot be undone. No durable authority is implied.
pub trait RuntimeManagedDrain: Send + Sync {
    fn wait_drained(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), RuntimeBackendError>> + Send + '_>,
    >;
}

#[async_trait]
/// Capability operations are mandatory for every Runtime Backend.
///
/// ```compile_fail
/// use runtime_core::runtime_backend::CapabilityRuntimePort;
/// struct IncompleteCapabilityBackend;
/// impl CapabilityRuntimePort for IncompleteCapabilityBackend {}
/// ```
pub trait CapabilityRuntimePort: Send + Sync {
    async fn drain_managed_contributions(
        &self,
        handles: &[extension_contracts::ManagedExecutionHandle],
    ) -> Result<Box<dyn RuntimeManagedDrain>, RuntimeBackendError>;
    /// Bind a trusted installation identity to an exact contribution. This does not grant permission.
    async fn activate_managed_contribution(
        &self,
        request: RuntimeManagedActivation,
    ) -> Result<extension_contracts::extension_bus::ManagedExecutionHandle, RuntimeBackendError>;
    async fn deactivate_managed_contribution(
        &self,
        handle: &extension_contracts::extension_bus::ManagedExecutionHandle,
    ) -> Result<(), RuntimeBackendError>;
    /// Returns only after validating the handle/deadline and acquiring its execution scope lease.
    /// The caller holds its current authority lease until this returns, then releases it before
    /// awaiting the worker. This must not merely wrap a not-yet-admitted async call.
    async fn admit_managed_capability_execute(
        &self,
        request: RuntimeManagedCapabilityRequest,
    ) -> Result<AdmittedManagedExecution, RuntimeBackendError>;
    /// Holds current contribution authority until the exact Hook binding obtains its scope lease.
    async fn admit_managed_event(
        &self,
        request: RuntimeManagedEventRequest,
    ) -> Result<AdmittedManagedEvent, RuntimeBackendError>;

    async fn admit_managed_hook(
        &self,
        request: RuntimeManagedHookRequest,
    ) -> Result<AdmittedManagedHook, RuntimeBackendError>;
    /// The composition owner must admit each call against current contribution authority.
    /// Worker output is an opaque result and never grants credit or other host permissions.
    async fn managed_capability_execute(
        &self,
        request: RuntimeManagedCapabilityRequest,
    ) -> Result<Value, RuntimeBackendError>;

    async fn activate_capability(
        &self,
        request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError>;
    async fn deactivate_capability(&self, plugin_id: &str) -> Result<(), RuntimeBackendError>;
    async fn capability_validate(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: Value,
    ) -> Result<Value, RuntimeBackendError>;

    async fn capability_resolve_dynamic_options(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: Value,
    ) -> Result<Value, RuntimeBackendError>;

    async fn capability_resolve_output_schema(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: Value,
    ) -> Result<Value, RuntimeBackendError>;

    async fn capability_execute(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<RuntimeCapabilityExecutionOutcome, RuntimeBackendError>;
}

#[async_trait]
/// Network Egress operations are mandatory for every Runtime Backend.
///
/// ```compile_fail
/// use runtime_core::runtime_backend::NetworkEgressRuntimePort;
/// struct IncompleteNetworkEgressBackend;
/// impl NetworkEgressRuntimePort for IncompleteNetworkEgressBackend {}
/// ```
pub trait NetworkEgressRuntimePort: Send + Sync {
    async fn network_egress_preflight(
        &self,
        request: RuntimeNetworkEgressActivation,
    ) -> Result<(), RuntimeBackendError>;

    async fn network_egress_activate(
        &self,
        request: RuntimeNetworkEgressActivation,
    ) -> Result<(), RuntimeBackendError>;

    async fn network_egress_sync(
        &self,
        runtime_id: &str,
    ) -> Result<Vec<EgressDescriptor>, RuntimeBackendError>;

    async fn network_egress_resolve_http_forward_proxy(
        &self,
        runtime_id: &str,
        egress_key: &str,
    ) -> Result<ForwardProxyLease, RuntimeBackendError>;

    async fn network_egress_release_http_forward_proxy(
        &self,
        runtime_id: &str,
        lease_id: &str,
    ) -> Result<(), RuntimeBackendError>;

    async fn network_egress_deactivate(&self, runtime_id: &str) -> Result<(), RuntimeBackendError>;
}

pub trait RuntimeBackend:
    RuntimeExecutionPort
    + RuntimeObservationPort
    + ProviderRuntimePort
    + DataSourceRuntimePort
    + CapabilityRuntimePort
    + NetworkEgressRuntimePort
{
}

impl<T> RuntimeBackend for T where
    T: RuntimeExecutionPort
        + RuntimeObservationPort
        + ProviderRuntimePort
        + DataSourceRuntimePort
        + CapabilityRuntimePort
        + NetworkEgressRuntimePort
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
