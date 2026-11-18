use crate::middleware::require_session::{require_session, RequestCredential};
use anyhow::Result;
pub use api_host_kernel::extension_bus::authentication_activation::*;
use interface_runtime::*;
pub(crate) trait AuthenticationFactoryAssembly {
    fn built_in() -> Result<AuthenticationAdapterFactoryRegistry>;
}
impl AuthenticationFactoryAssembly for AuthenticationAdapterFactoryRegistry {
    fn built_in() -> Result<Self> {
        Self::built_in_ready(vec![
            AuthenticationAdapterFactoryBinding::typed(
                activation(
                    crate::routes::external_auth::interface::CALLBACK_AUTH_PLUGIN,
                    InterfaceExtensionTier::BuiltIn,
                    crate::routes::external_auth::interface::CALLBACK_AUTH_ADAPTER,
                    crate::routes::external_auth::interface::CALLBACK_AUTH_ACTIVATION,
                    PrincipalProfile::Public,
                )?,
                |credential: crate::routes::external_auth::ExternalCallbackCredential| async move {
                    credential.authenticate().await.map_err(|error| error.0)
                },
            )?,
            AuthenticationAdapterFactoryBinding::typed(
                activation(
                    "api-server.runtime-model-authentication",
                    InterfaceExtensionTier::BuiltIn,
                    "api-server.runtime-user",
                    "api-server.runtime-user.activation.v1",
                    PrincipalProfile::User,
                )?,
                |credential: RuntimeModelAuthenticationCredential| async move {
                    let context = require_session(&credential.state, &credential.headers)
                        .await
                        .map_err(|error| error.0)?;
                    if crate::routes::runtime_models::runtime_operation_requires_csrf(
                        credential.state.as_ref(),
                        &context.actor,
                        credential.method,
                        &credential.model_code,
                        &credential.path,
                    ) && matches!(context.credential, RequestCredential::CookieSession(_))
                    {
                        crate::middleware::require_csrf::require_csrf(
                            &credential.headers,
                            &context,
                        )
                        .map_err(|error| error.0)?;
                    }
                    Ok(context.interface_principal())
                },
            )?,
        ])
    }
}
fn activation(
    plugin: &str,
    tier: InterfaceExtensionTier,
    adapter: &str,
    activation: &str,
    profile: PrincipalProfile,
) -> Result<ActivatedAuthenticationAdapter> {
    Ok(ActivatedAuthenticationAdapter::new(
        PluginIdentity::new(plugin)?,
        tier,
        AuthenticationAdapterReference::new(adapter)?,
        AuthenticationActivationIdentity::new(activation)?,
        profile,
    ))
}
