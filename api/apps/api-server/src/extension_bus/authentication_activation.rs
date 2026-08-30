use std::{any::Any, collections::BTreeMap, sync::Arc};

use interface_runtime::{
    ActivatedAuthenticationAdapter, AuthenticationActivationIdentity,
    AuthenticationAdapterReference, CompiledInterfaceRegistry, InvocationPrincipal,
    PrincipalProfile,
};

trait ErasedAuthenticationAdapterFactory: Send + Sync {
    fn adapter(&self) -> &AuthenticationAdapterReference;
    fn principal_profile(&self) -> PrincipalProfile;
    fn establish(&self, principal: Box<dyn Any + Send>) -> anyhow::Result<Box<dyn Any + Send>>;
}

struct SealedPrincipalFactory<P>
where
    P: InvocationPrincipal,
{
    adapter: AuthenticationAdapterReference,
    marker: std::marker::PhantomData<fn(P)>,
}

impl<P> ErasedAuthenticationAdapterFactory for SealedPrincipalFactory<P>
where
    P: InvocationPrincipal,
{
    fn adapter(&self) -> &AuthenticationAdapterReference {
        &self.adapter
    }

    fn principal_profile(&self) -> PrincipalProfile {
        P::PROFILE
    }

    fn establish(&self, principal: Box<dyn Any + Send>) -> anyhow::Result<Box<dyn Any + Send>> {
        let principal = principal
            .downcast::<P>()
            .map_err(|_| anyhow::anyhow!("authentication principal contract mismatch"))?;
        Ok(principal)
    }
}

pub(crate) struct AuthenticationAdapterFactoryRegistry {
    factories:
        BTreeMap<AuthenticationActivationIdentity, Arc<dyn ErasedAuthenticationAdapterFactory>>,
}

impl AuthenticationAdapterFactoryRegistry {
    pub(crate) fn built_in() -> anyhow::Result<Self> {
        let mut registry = Self {
            factories: BTreeMap::new(),
        };
        registry.bind::<interface_runtime::PublicPrincipal>(
            "api-server.public",
            "api-server.public.activation.v1",
        )?;
        registry.bind::<interface_runtime::UserPrincipal>(
            "api-server.console.require-session",
            "api-server.console.require-session.activation.v1",
        )?;
        registry.bind::<interface_runtime::UserPrincipal>(
            "api-server.user-api-key",
            "api-server.user-api-key.activation.v1",
        )?;
        registry.bind::<interface_runtime::ApplicationPrincipal>(
            "api-server.application-api-key",
            "api-server.application-api-key.activation.v1",
        )?;
        Ok(registry)
    }

    fn bind<P>(&mut self, adapter: &str, activation: &str) -> anyhow::Result<()>
    where
        P: InvocationPrincipal,
    {
        let activation = AuthenticationActivationIdentity::new(activation)?;
        if self.factories.contains_key(&activation) {
            anyhow::bail!("authentication activation {activation} is duplicated");
        }
        self.factories.insert(
            activation,
            Arc::new(SealedPrincipalFactory::<P> {
                adapter: AuthenticationAdapterReference::new(adapter)?,
                marker: std::marker::PhantomData,
            }),
        );
        Ok(())
    }

    pub(crate) fn validate_registry(
        &self,
        registry: &CompiledInterfaceRegistry,
    ) -> anyhow::Result<()> {
        for binding in registry.bindings() {
            let activation = registry
                .authentication(binding.binding_id())
                .ok_or_else(|| anyhow::anyhow!("binding has no authentication activation"))?;
            self.factory(activation)?;
        }
        Ok(())
    }

    pub(crate) fn establish<P>(
        &self,
        activation: &ActivatedAuthenticationAdapter,
        principal: P,
    ) -> anyhow::Result<P>
    where
        P: InvocationPrincipal,
    {
        let factory = self.factory(activation)?;
        let principal = factory.establish(Box::new(principal))?;
        principal
            .downcast::<P>()
            .map(|principal| *principal)
            .map_err(|_| anyhow::anyhow!("activated authentication output contract mismatch"))
    }

    fn factory(
        &self,
        activation: &ActivatedAuthenticationAdapter,
    ) -> anyhow::Result<&Arc<dyn ErasedAuthenticationAdapterFactory>> {
        let factory = self.factories.get(activation.activation()).ok_or_else(|| {
            anyhow::anyhow!("authentication activation is not bound to a factory")
        })?;
        if factory.adapter() != activation.adapter()
            || factory.principal_profile() != activation.principal_profile()
        {
            anyhow::bail!("authentication activation factory identity mismatch");
        }
        Ok(factory)
    }
}
