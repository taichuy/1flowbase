use crate::{
    AuthenticationActivationIdentity, AuthenticationAdapterReference, InterfaceExtensionTier,
    PluginIdentity, PrincipalProfile,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivatedAuthenticationAdapter {
    plugin: PluginIdentity,
    tier: InterfaceExtensionTier,
    adapter: AuthenticationAdapterReference,
    activation: AuthenticationActivationIdentity,
    principal_profile: PrincipalProfile,
}

impl ActivatedAuthenticationAdapter {
    pub fn new(
        plugin: PluginIdentity,
        tier: InterfaceExtensionTier,
        adapter: AuthenticationAdapterReference,
        activation: AuthenticationActivationIdentity,
        principal_profile: PrincipalProfile,
    ) -> Self {
        Self {
            plugin,
            tier,
            adapter,
            activation,
            principal_profile,
        }
    }

    pub fn plugin(&self) -> &PluginIdentity {
        &self.plugin
    }

    pub fn tier(&self) -> InterfaceExtensionTier {
        self.tier
    }

    pub fn adapter(&self) -> &AuthenticationAdapterReference {
        &self.adapter
    }

    pub fn activation(&self) -> &AuthenticationActivationIdentity {
        &self.activation
    }

    pub fn principal_profile(&self) -> PrincipalProfile {
        self.principal_profile
    }
}
