use std::{
    any::{Any, TypeId},
    collections::{BTreeMap, BTreeSet},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
};

use anyhow::{bail, Context, Result};
use axum::http::HeaderMap;
use control_plane::application_public_api::api_keys::ApplicationApiKeyService;
use interface_runtime::{
    ActivatedAuthenticationAdapter, AuthenticationActivationIdentity,
    AuthenticationAdapterReference, CompiledInterfaceRegistry, InterfaceExtensionTier,
    InvocationPrincipal, PluginIdentity, PrincipalProfile,
};
use plugin_framework::{
    HostExtensionContributionManifest, HostExtensionInterfaceAuthenticationManifest,
    HostExtensionInterfaceAuthenticationPrincipalProfile, PluginManifestV1,
};

use crate::{
    app_state::ApiState,
    middleware::require_session::{require_session, RequestCredential},
};

type ErasedAuthenticationFuture = Pin<Box<dyn Future<Output = Result<Box<dyn Any + Send>>> + Send>>;

trait ErasedAuthenticationAdapterFactory: Send + Sync {
    fn plugin(&self) -> &PluginIdentity;
    fn tier(&self) -> InterfaceExtensionTier;
    fn adapter(&self) -> &AuthenticationAdapterReference;
    fn activation(&self) -> &AuthenticationActivationIdentity;
    fn principal_profile(&self) -> PrincipalProfile;
    fn credential_contract(&self) -> TypeId;
    fn authenticate(&self, credential: Box<dyn Any + Send>) -> ErasedAuthenticationFuture;
}

struct TypedAuthenticationAdapterFactory<C, P, F> {
    activation: ActivatedAuthenticationAdapter,
    authenticate: F,
    marker: PhantomData<fn(C) -> P>,
}

impl<C, P, F, Fut> ErasedAuthenticationAdapterFactory for TypedAuthenticationAdapterFactory<C, P, F>
where
    C: Any + Send + 'static,
    P: InvocationPrincipal,
    F: Fn(C) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<P>> + Send + 'static,
{
    fn plugin(&self) -> &PluginIdentity {
        self.activation.plugin()
    }

    fn tier(&self) -> InterfaceExtensionTier {
        self.activation.tier()
    }

    fn adapter(&self) -> &AuthenticationAdapterReference {
        self.activation.adapter()
    }

    fn activation(&self) -> &AuthenticationActivationIdentity {
        self.activation.activation()
    }

    fn principal_profile(&self) -> PrincipalProfile {
        P::PROFILE
    }

    fn credential_contract(&self) -> TypeId {
        TypeId::of::<C>()
    }

    fn authenticate(&self, credential: Box<dyn Any + Send>) -> ErasedAuthenticationFuture {
        let credential = credential.downcast::<C>();
        match credential {
            Ok(credential) => {
                let future = (self.authenticate)(*credential);
                Box::pin(async move {
                    future
                        .await
                        .map(|principal| Box::new(principal) as Box<dyn Any + Send>)
                })
            }
            Err(_) => Box::pin(async {
                Err(anyhow::anyhow!(
                    "authentication credential contract mismatch"
                ))
            }),
        }
    }
}

pub(crate) struct AuthenticationAdapterFactoryBinding {
    factory: Arc<dyn ErasedAuthenticationAdapterFactory>,
}

impl AuthenticationAdapterFactoryBinding {
    pub(crate) fn typed<C, P, F, Fut>(
        activation: ActivatedAuthenticationAdapter,
        authenticate: F,
    ) -> Result<Self>
    where
        C: Any + Send + 'static,
        P: InvocationPrincipal,
        F: Fn(C) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<P>> + Send + 'static,
    {
        if !matches!(
            activation.tier(),
            InterfaceExtensionTier::BuiltIn | InterfaceExtensionTier::HostExtension
        ) {
            bail!("only built-in or trusted HostExtension authentication factories are allowed");
        }
        if activation.principal_profile() != P::PROFILE {
            bail!("authentication factory principal profile does not match its activation");
        }
        Ok(Self {
            factory: Arc::new(TypedAuthenticationAdapterFactory::<C, P, F> {
                activation,
                authenticate,
                marker: PhantomData,
            }),
        })
    }
}

#[derive(Default)]
pub(crate) struct AuthenticationAdapterFactoryRegistry {
    factories:
        BTreeMap<AuthenticationActivationIdentity, Arc<dyn ErasedAuthenticationAdapterFactory>>,
}

impl AuthenticationAdapterFactoryRegistry {
    pub(crate) fn built_in() -> Result<Self> {
        let mut registry = Self::default();
        for binding in built_in_authentication_factories()? {
            registry.register(binding)?;
        }
        Ok(registry)
    }

    pub(crate) fn extend(
        &mut self,
        bindings: impl IntoIterator<Item = AuthenticationAdapterFactoryBinding>,
    ) -> Result<()> {
        for binding in bindings {
            self.register(binding)?;
        }
        Ok(())
    }

    pub(crate) fn activate_host_extensions(
        &mut self,
        bindings: impl IntoIterator<Item = AuthenticationAdapterFactoryBinding>,
    ) -> Result<()> {
        self.extend(bindings)
    }

    fn register(&mut self, binding: AuthenticationAdapterFactoryBinding) -> Result<()> {
        let activation = binding.factory.activation().clone();
        if self
            .factories
            .insert(activation.clone(), binding.factory)
            .is_some()
        {
            bail!("authentication activation {activation} is duplicated");
        }
        Ok(())
    }

    pub(crate) fn validate_registry(&self, registry: &CompiledInterfaceRegistry) -> Result<()> {
        let mut expected = BTreeMap::new();
        for binding in registry.bindings() {
            let activation = registry
                .authentication(binding.binding_id())
                .ok_or_else(|| anyhow::anyhow!("binding has no authentication activation"))?;
            if let Some(existing) = expected.insert(activation.activation().clone(), activation) {
                if existing != activation {
                    bail!("authentication activation identity is bound inconsistently");
                }
            }
        }
        for activation in expected.values() {
            self.factory(activation)?;
        }
        let expected_ids = expected.keys().collect::<BTreeSet<_>>();
        for activation in self.factories.keys() {
            if !expected_ids.contains(activation) {
                bail!("authentication factory has no compiled registration: {activation}");
            }
        }
        Ok(())
    }

    pub(crate) async fn authenticate<C, P>(
        &self,
        activation: &ActivatedAuthenticationAdapter,
        credential: C,
    ) -> Result<P>
    where
        C: Any + Send + 'static,
        P: InvocationPrincipal,
    {
        let factory = self.factory(activation)?;
        if factory.credential_contract() != TypeId::of::<C>() {
            bail!("authentication credential contract mismatch");
        }
        let principal = factory.authenticate(Box::new(credential)).await?;
        principal
            .downcast::<P>()
            .map(|principal| *principal)
            .map_err(|_| anyhow::anyhow!("activated authentication output contract mismatch"))
    }

    fn factory(
        &self,
        activation: &ActivatedAuthenticationAdapter,
    ) -> Result<&Arc<dyn ErasedAuthenticationAdapterFactory>> {
        let factory = self.factories.get(activation.activation()).ok_or_else(|| {
            anyhow::anyhow!("authentication activation is not bound to a factory")
        })?;
        if factory.plugin() != activation.plugin()
            || factory.tier() != activation.tier()
            || factory.adapter() != activation.adapter()
            || factory.principal_profile() != activation.principal_profile()
        {
            bail!("authentication activation factory identity mismatch");
        }
        Ok(factory)
    }
}

pub(crate) struct PublicAuthenticationCredential;

pub(crate) const CONSOLE_SESSION_CREDENTIAL_CONTRACT_ID: &str =
    "api-server.console-session-credential";
pub(crate) const CONSOLE_SESSION_CREDENTIAL_CONTRACT_VERSION: &str = "1";

pub(crate) enum ConsoleAuthenticationCredential {
    Protocol {
        state: Arc<ApiState>,
        headers: HeaderMap,
    },
    ProtocolWithCsrf {
        state: Arc<ApiState>,
        headers: HeaderMap,
    },
    CookieSessionWithCsrf {
        state: Arc<ApiState>,
        headers: HeaderMap,
    },
    ServerDelegation(domain::ActorContext),
}

pub(crate) struct McpUserApiKeyAuthenticationCredential {
    pub(crate) state: Arc<ApiState>,
    pub(crate) headers: HeaderMap,
}

pub(crate) struct ApplicationApiKeyAuthenticationCredential {
    pub(crate) state: Arc<ApiState>,
    pub(crate) bearer_token: String,
}

pub(crate) struct RuntimeModelAuthenticationCredential {
    pub(crate) state: Arc<ApiState>,
    pub(crate) headers: HeaderMap,
    pub(crate) method: plugin_framework::DataModelOperationMethod,
    pub(crate) model_code: String,
    pub(crate) path: String,
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

fn built_in_authentication_factories() -> Result<Vec<AuthenticationAdapterFactoryBinding>> {
    Ok(vec![
        AuthenticationAdapterFactoryBinding::typed(
            activation(
                "api-server.public-authentication",
                InterfaceExtensionTier::BuiltIn,
                "api-server.public",
                "api-server.public.activation.v1",
                PrincipalProfile::Public,
            )?,
            |_credential: PublicAuthenticationCredential| async {
                Ok(interface_runtime::PublicPrincipal::new())
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
                    crate::middleware::require_csrf::require_csrf(&credential.headers, &context)
                        .map_err(|error| error.0)?;
                }
                Ok(context.interface_principal())
            },
        )?,
        AuthenticationAdapterFactoryBinding::typed(
            activation(
                "api-server.console-authentication",
                InterfaceExtensionTier::BuiltIn,
                "api-server.console.require-session",
                "api-server.console.require-session.activation.v1",
                PrincipalProfile::User,
            )?,
            |credential: ConsoleAuthenticationCredential| async move {
                match credential {
                    ConsoleAuthenticationCredential::Protocol { state, headers } => {
                        let context = require_session(&state, &headers)
                            .await
                            .map_err(|error| error.0)?;
                        Ok(context.interface_principal())
                    }
                    ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers } => {
                        let context = require_session(&state, &headers)
                            .await
                            .map_err(|error| error.0)?;
                        if matches!(context.credential, RequestCredential::CookieSession(_)) {
                            crate::middleware::require_csrf::require_csrf(&headers, &context)
                                .map_err(|error| error.0)?;
                        }
                        Ok(context.interface_principal())
                    }
                    ConsoleAuthenticationCredential::CookieSessionWithCsrf { state, headers } => {
                        let context = require_session(&state, &headers)
                            .await
                            .map_err(|error| error.0)?;
                        context.cookie_session()?;
                        crate::middleware::require_csrf::require_csrf(&headers, &context)
                            .map_err(|error| error.0)?;
                        Ok(context.interface_principal())
                    }
                    ConsoleAuthenticationCredential::ServerDelegation(actor) => {
                        Ok(interface_runtime::UserPrincipal::server_delegation(actor))
                    }
                }
            },
        )?,
        AuthenticationAdapterFactoryBinding::typed(
            activation(
                "api-server.mcp-authentication",
                InterfaceExtensionTier::BuiltIn,
                "api-server.user-api-key",
                "api-server.user-api-key.activation.v1",
                PrincipalProfile::User,
            )?,
            |credential: McpUserApiKeyAuthenticationCredential| async move {
                let context = require_session(&credential.state, &credential.headers)
                    .await
                    .map_err(|error| error.0)?;
                if !matches!(context.credential, RequestCredential::UserApiKey { .. }) {
                    bail!("MCP interface authentication requires a user API key");
                }
                Ok(context.interface_principal())
            },
        )?,
        AuthenticationAdapterFactoryBinding::typed(
            activation(
                "api-server.application-authentication",
                InterfaceExtensionTier::BuiltIn,
                "api-server.application-api-key",
                "api-server.application-api-key.activation.v1",
                PrincipalProfile::Application,
            )?,
            |credential: ApplicationApiKeyAuthenticationCredential| async move {
                let actor = ApplicationApiKeyService::new(credential.state.store.clone())
                    .with_last_used_cache(credential.state.infrastructure.cache_store())
                    .authenticate_bearer_token(&credential.bearer_token)
                    .await
                    .context("application API key authentication failed")?;
                interface_runtime::ApplicationPrincipal::new(
                    actor.application_id,
                    actor.api_key_id,
                    actor.workspace_id,
                    actor.actor,
                )
                .map_err(anyhow::Error::from)
            },
        )?,
    ])
}

type HostExtensionAuthenticationFactory = Arc<
    dyn Fn(&HostExtensionContributionManifest) -> Result<Vec<AuthenticationAdapterFactoryBinding>>
        + Send
        + Sync,
>;

#[derive(Default)]
pub(crate) struct HostExtensionAuthenticationFactoryCatalog {
    factories: BTreeMap<(String, String), HostExtensionAuthenticationFactory>,
}

impl HostExtensionAuthenticationFactoryCatalog {
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "production registration Port; the baseline installs no authentication HostExtension"
        )
    )]
    pub(crate) fn register(
        &mut self,
        library: impl Into<String>,
        entry_symbol: impl Into<String>,
        factory: HostExtensionAuthenticationFactory,
    ) -> Result<()> {
        let key = (library.into(), entry_symbol.into());
        if self.factories.insert(key.clone(), factory).is_some() {
            bail!(
                "duplicate HostExtension authentication factory {}::{}",
                key.0,
                key.1
            );
        }
        Ok(())
    }

    pub(crate) fn activate(
        &self,
        active_extensions: &[(PluginManifestV1, HostExtensionContributionManifest)],
    ) -> Result<Vec<AuthenticationAdapterFactoryBinding>> {
        let mut bindings = Vec::new();
        for (_, contribution) in active_extensions {
            if contribution.interface_authentication_adapters.is_empty() {
                continue;
            }
            let key = (
                contribution.native.library.clone(),
                contribution.native.entry_symbol.clone(),
            );
            let factory = self.factories.get(&key).ok_or_else(|| {
                anyhow::anyhow!(
                    "active HostExtension {} has Interface Authentication contributions but no activation factory for {}::{}",
                    contribution.extension_id,
                    key.0,
                    key.1
                )
            })?;
            bindings.extend(factory(contribution)?);
        }
        Ok(bindings)
    }
}

pub(crate) fn production_host_extension_authentication_factories(
) -> HostExtensionAuthenticationFactoryCatalog {
    let mut catalog = HostExtensionAuthenticationFactoryCatalog::default();
    catalog
        .register(
            "builtin://official.identity-host",
            "builtin_identity_host",
            Arc::new(identity_host_authentication_factories),
        )
        .expect("built-in identity HostExtension Authentication factory must be unique");
    catalog
}

pub(crate) fn activated_host_authentication(
    extension_id: &str,
    descriptor: &HostExtensionInterfaceAuthenticationManifest,
) -> Result<ActivatedAuthenticationAdapter> {
    let profile = match descriptor.principal_profile {
        HostExtensionInterfaceAuthenticationPrincipalProfile::Public => PrincipalProfile::Public,
        HostExtensionInterfaceAuthenticationPrincipalProfile::User => PrincipalProfile::User,
        HostExtensionInterfaceAuthenticationPrincipalProfile::Application => {
            PrincipalProfile::Application
        }
    };
    activation(
        extension_id,
        InterfaceExtensionTier::HostExtension,
        &descriptor.adapter_id,
        &descriptor.activation_id,
        profile,
    )
}

fn identity_host_authentication_factories(
    contribution: &HostExtensionContributionManifest,
) -> Result<Vec<AuthenticationAdapterFactoryBinding>> {
    contribution
        .interface_authentication_adapters
        .iter()
        .map(|descriptor| {
            if descriptor.credential.contract_id != CONSOLE_SESSION_CREDENTIAL_CONTRACT_ID
                || descriptor.credential.contract_version
                    != CONSOLE_SESSION_CREDENTIAL_CONTRACT_VERSION
                || descriptor.principal_profile
                    != HostExtensionInterfaceAuthenticationPrincipalProfile::User
            {
                bail!(
                    "identity HostExtension does not implement requested Authentication credential contract"
                );
            }
            AuthenticationAdapterFactoryBinding::typed(
                activated_host_authentication(&contribution.extension_id, descriptor)?,
                |credential: ConsoleAuthenticationCredential| async move {
                    match credential {
                        ConsoleAuthenticationCredential::Protocol { state, headers } => {
                            let context = require_session(&state, &headers)
                                .await
                                .map_err(|error| error.0)?;
                            Ok(context.interface_principal())
                        }
                        ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers } => {
                            let context = require_session(&state, &headers)
                                .await
                                .map_err(|error| error.0)?;
                            if matches!(context.credential, RequestCredential::CookieSession(_)) {
                                crate::middleware::require_csrf::require_csrf(&headers, &context)
                                    .map_err(|error| error.0)?;
                            }
                            Ok(context.interface_principal())
                        }
                        ConsoleAuthenticationCredential::CookieSessionWithCsrf {
                            state,
                            headers,
                        } => {
                            let context = require_session(&state, &headers)
                                .await
                                .map_err(|error| error.0)?;
                            context.cookie_session()?;
                            crate::middleware::require_csrf::require_csrf(&headers, &context)
                                .map_err(|error| error.0)?;
                            Ok(context.interface_principal())
                        }
                        ConsoleAuthenticationCredential::ServerDelegation(actor) => {
                            Ok(interface_runtime::UserPrincipal::server_delegation(actor))
                        }
                    }
                },
            )
        })
        .collect()
}
