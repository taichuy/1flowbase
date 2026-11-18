#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
use crate::{app_state::ApiState, error_response::ApiError};
#[cfg_attr(test, api_test_selection::group(core, contracts, routes))]
#[cfg(test)]
pub use api_host_kernel::routes::console_interface::*;
#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
use interface_runtime::InterfaceContract;
#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
use std::sync::Arc;

#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
pub(crate) async fn invoke<I, O>(
    state: Arc<ApiState>,
    binding_id: &str,
    credential: crate::extension_bus::ConsoleAuthenticationCredential,
    input: I,
) -> Result<O, ApiError>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    api_host_kernel::routes::console_interface::invoke_prepared(
        state,
        binding_id,
        credential,
        input,
        crate::routes::model_definitions::interface::managed_hooks::ModelDefinitionPreparation,
    )
    .await
}
