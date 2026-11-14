use crate::{app_state::ApiState, error_response::ApiError};
pub use api_host_kernel::routes::console_interface::*;
use interface_runtime::{InterfaceContract, UserPrincipal};
use std::sync::Arc;

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
pub(crate) async fn invoke_with_principal<I, O>(
    state: Arc<ApiState>,
    binding_id: &str,
    principal: UserPrincipal,
    input: I,
) -> Result<O, ApiError>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    api_host_kernel::routes::console_interface::invoke_with_principal_prepared(
        state,
        binding_id,
        principal,
        input,
        crate::routes::model_definitions::interface::managed_hooks::ModelDefinitionPreparation,
    )
    .await
}
