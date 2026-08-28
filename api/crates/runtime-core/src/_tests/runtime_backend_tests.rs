use std::sync::Arc;

use async_trait::async_trait;
use extension_contracts::provider_contract::{
    ProviderAuthOperation, ProviderAuthResult, ProviderBalanceResult, ProviderCompactResult,
    ProviderCountTokensInput, ProviderCountTokensResult, ProviderInvocationInput,
    ProviderModelDescriptor, ProviderResetCreditOperation, ProviderResetCreditResult,
    ProviderUsageWindowsResult,
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
use serde_json::Value;

use crate::runtime_backend::{
    CapabilityRuntimePort, DataSourceRuntimePort, NetworkEgressRuntimePort, ProviderRuntimePort,
    RuntimeBackend, RuntimeBackendError, RuntimeBackendLifecycle, RuntimeBackendSlot,
    RuntimeBackendSnapshot, RuntimeCancelOutcome, RuntimeCapabilityExecutionOutcome,
    RuntimeExecutionOutcome, RuntimeExecutionPort, RuntimeExecutionRequest,
    RuntimeNetworkEgressActivation, RuntimeObservationPort, RuntimePackageActivation,
    RuntimeProviderDistributionRequest, RuntimeRegistrySnapshot, RuntimeRequestId,
    RuntimeStreamSinks,
};

struct CompleteFakeBackend;

#[async_trait]
impl RuntimeExecutionPort for CompleteFakeBackend {
    async fn activate_provider_distribution_rule(
        &self,
        _request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError> {
        Ok(())
    }

    async fn deactivate_provider_distribution_rule(
        &self,
        _plugin_id: &str,
    ) -> Result<(), RuntimeBackendError> {
        Ok(())
    }

    async fn select_provider_distribution(
        &self,
        _request: RuntimeProviderDistributionRequest,
    ) -> Result<extension_contracts::ProviderDistributionSelectionReceipt, RuntimeBackendError>
    {
        unreachable!("compile fixture is not executed")
    }

    async fn execute(
        &self,
        _request: RuntimeExecutionRequest,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn execute_stream(
        &self,
        _request: RuntimeExecutionRequest,
        _sinks: RuntimeStreamSinks,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn cancel(
        &self,
        _request_id: &RuntimeRequestId,
    ) -> Result<RuntimeCancelOutcome, RuntimeBackendError> {
        Ok(RuntimeCancelOutcome::NotFound)
    }
}

#[async_trait]
impl RuntimeObservationPort for CompleteFakeBackend {
    async fn snapshot(&self) -> Result<RuntimeBackendSnapshot, RuntimeBackendError> {
        Ok(RuntimeBackendSnapshot {
            backend_kind: "complete_fake".to_string(),
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

#[async_trait]
impl ProviderRuntimePort for CompleteFakeBackend {
    async fn activate_provider(
        &self,
        _request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError> {
        Ok(())
    }

    async fn deactivate_provider(&self, _plugin_id: &str) -> Result<(), RuntimeBackendError> {
        Ok(())
    }

    async fn provider_validate(
        &self,
        _target_id: &str,
        _provider_config: Value,
    ) -> Result<Value, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn provider_authenticate(
        &self,
        _target_id: &str,
        _provider_config: Value,
        _operation: ProviderAuthOperation,
    ) -> Result<ProviderAuthResult, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn provider_list_models(
        &self,
        _target_id: &str,
        _provider_config: Value,
    ) -> Result<Vec<ProviderModelDescriptor>, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn provider_get_balance(
        &self,
        _target_id: &str,
        _provider_config: Value,
    ) -> Result<ProviderBalanceResult, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn provider_get_usage_windows(
        &self,
        _target_id: &str,
        _provider_config: Value,
    ) -> Result<ProviderUsageWindowsResult, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn provider_reset_credit(
        &self,
        _target_id: &str,
        _provider_config: Value,
        _operation: ProviderResetCreditOperation,
    ) -> Result<ProviderResetCreditResult, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn provider_count_tokens(
        &self,
        _target_id: &str,
        _input: ProviderCountTokensInput,
    ) -> Result<ProviderCountTokensResult, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn provider_compact(
        &self,
        _target_id: &str,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderCompactResult, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }
}

#[async_trait]
impl DataSourceRuntimePort for CompleteFakeBackend {
    async fn activate_data_source(
        &self,
        _request: RuntimePackageActivation,
    ) -> Result<Vec<DataModelTemplateDescriptor>, RuntimeBackendError> {
        Ok(Vec::new())
    }

    async fn deactivate_data_source(&self, _plugin_id: &str) -> Result<(), RuntimeBackendError> {
        Ok(())
    }

    async fn data_source_validate(
        &self,
        _target_id: &str,
        _input: DataSourceConfigInput,
    ) -> Result<Value, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_test_connection(
        &self,
        _target_id: &str,
        _input: DataSourceConfigInput,
    ) -> Result<Value, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_discover_catalog(
        &self,
        _target_id: &str,
        _input: DataSourceConfigInput,
    ) -> Result<Vec<DataSourceCatalogEntry>, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_describe_resource(
        &self,
        _target_id: &str,
        _input: DataSourceDescribeResourceInput,
    ) -> Result<DataSourceResourceDescriptor, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_preview_read(
        &self,
        _target_id: &str,
        _input: DataSourcePreviewReadInput,
    ) -> Result<DataSourcePreviewReadOutput, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_import_snapshot(
        &self,
        _target_id: &str,
        _input: DataSourceImportSnapshotInput,
    ) -> Result<DataSourceImportSnapshotOutput, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_list_records(
        &self,
        _target_id: &str,
        _input: DataSourceListRecordsInput,
    ) -> Result<DataSourceListRecordsOutput, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_get_record(
        &self,
        _target_id: &str,
        _input: DataSourceGetRecordInput,
    ) -> Result<DataSourceGetRecordOutput, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_create_record(
        &self,
        _target_id: &str,
        _input: DataSourceCreateRecordInput,
    ) -> Result<DataSourceCreateRecordOutput, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_update_record(
        &self,
        _target_id: &str,
        _input: DataSourceUpdateRecordInput,
    ) -> Result<DataSourceUpdateRecordOutput, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_delete_record(
        &self,
        _target_id: &str,
        _input: DataSourceDeleteRecordInput,
    ) -> Result<DataSourceDeleteRecordOutput, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_execute_sql(
        &self,
        _target_id: &str,
        _input: DataSourceExecuteSqlInput,
    ) -> Result<NativeSqlExecutionOutput, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn data_source_execute_model_operation(
        &self,
        _target_id: &str,
        _input: DataSourceExecuteModelOperationInput,
    ) -> Result<Value, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }
}

#[async_trait]
impl CapabilityRuntimePort for CompleteFakeBackend {
    async fn activate_capability(
        &self,
        _request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError> {
        Ok(())
    }

    async fn deactivate_capability(&self, _plugin_id: &str) -> Result<(), RuntimeBackendError> {
        Ok(())
    }

    async fn capability_validate(
        &self,
        _target_id: &str,
        _contribution_code: &str,
        _config_payload: Value,
    ) -> Result<Value, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn capability_resolve_dynamic_options(
        &self,
        _target_id: &str,
        _contribution_code: &str,
        _config_payload: Value,
    ) -> Result<Value, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn capability_resolve_output_schema(
        &self,
        _target_id: &str,
        _contribution_code: &str,
        _config_payload: Value,
    ) -> Result<Value, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn capability_execute(
        &self,
        _target_id: &str,
        _contribution_code: &str,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<RuntimeCapabilityExecutionOutcome, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }
}

#[async_trait]
impl NetworkEgressRuntimePort for CompleteFakeBackend {
    async fn network_egress_preflight(
        &self,
        _request: RuntimeNetworkEgressActivation,
    ) -> Result<(), RuntimeBackendError> {
        Ok(())
    }

    async fn network_egress_activate(
        &self,
        _request: RuntimeNetworkEgressActivation,
    ) -> Result<(), RuntimeBackendError> {
        Ok(())
    }

    async fn network_egress_sync(
        &self,
        _runtime_id: &str,
    ) -> Result<Vec<EgressDescriptor>, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn network_egress_resolve_http_forward_proxy(
        &self,
        _runtime_id: &str,
        _egress_key: &str,
    ) -> Result<ForwardProxyLease, RuntimeBackendError> {
        unreachable!("compile fixture is not executed")
    }

    async fn network_egress_release_http_forward_proxy(
        &self,
        _runtime_id: &str,
        _lease_id: &str,
    ) -> Result<(), RuntimeBackendError> {
        Ok(())
    }

    async fn network_egress_deactivate(
        &self,
        _runtime_id: &str,
    ) -> Result<(), RuntimeBackendError> {
        Ok(())
    }
}

fn require_complete_backend<T: RuntimeBackend>() {}

#[test]
fn complete_fake_backend_implements_all_six_ports() {
    require_complete_backend::<CompleteFakeBackend>();
    let backend: Arc<dyn RuntimeBackend> = Arc::new(CompleteFakeBackend);
    let mut slot = RuntimeBackendSlot::default();
    slot.bind(backend).unwrap();
    assert!(slot.backend().is_ok());
}

#[test]
fn runtime_backend_slot_rejects_a_second_contribution() {
    let mut slot = RuntimeBackendSlot::default();
    slot.bind(Arc::new(CompleteFakeBackend)).unwrap();
    let error = slot.bind(Arc::new(CompleteFakeBackend)).unwrap_err();
    assert!(matches!(error, RuntimeBackendError::DuplicateBackend));
}
