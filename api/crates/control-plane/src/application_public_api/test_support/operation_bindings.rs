use super::*;
use crate::application_public_api::operation_bindings::{
    ApplicationOperationBindingCapabilitySupport, ApplicationOperationBindingOperation,
};

#[async_trait]
impl ApplicationOperationBindingCapabilityRepository for ApplicationPublicApiTestRepository {
    async fn application_operation_binding_capability(
        &self,
        _workspace_id: Uuid,
        _runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        operation: ApplicationOperationBindingOperation,
    ) -> Result<ApplicationOperationBindingCapabilitySupport> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .operation_binding_capability_supports
            .get(&operation)
            .copied()
            .unwrap_or(ApplicationOperationBindingCapabilitySupport::Supported))
    }
}
