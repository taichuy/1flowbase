use std::collections::{BTreeMap, BTreeSet};

use crate::error::{FrameworkResult, PluginFrameworkError};
use crate::host_extension_contribution::{
    AuthProviderContributionManifest, HostExtensionBootstrapPhase,
    HostInfrastructureProviderManifest,
};

#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredHostExtension {
    pub extension_id: String,
    pub bootstrap_phase: HostExtensionBootstrapPhase,
    pub infrastructure_providers: Vec<HostInfrastructureProviderManifest>,
    pub auth_providers: Vec<AuthProviderContributionManifest>,
    pub owned_resources: Vec<String>,
    pub extends_resources: Vec<String>,
    pub routes: Vec<String>,
    pub workers: Vec<String>,
    pub migrations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredInfrastructureProvider {
    pub extension_id: String,
    pub contract: String,
    pub provider_code: String,
    pub config_ref: String,
}

#[derive(Debug, Default)]
pub struct HostExtensionRegistry {
    extensions: BTreeMap<String, RegisteredHostExtension>,
    infrastructure_providers: BTreeMap<(String, String), RegisteredInfrastructureProvider>,
    auth_providers: BTreeMap<String, (String, AuthProviderContributionManifest)>,
    default_infrastructure_providers: BTreeMap<String, (String, String)>,
}

impl HostExtensionRegistry {
    pub fn register(&mut self, extension: RegisteredHostExtension) -> FrameworkResult<()> {
        if self.extensions.contains_key(&extension.extension_id) {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "duplicate host extension: {}",
                extension.extension_id
            )));
        }

        let mut declared_provider_keys = BTreeSet::new();
        let mut declared_default_contracts = BTreeSet::new();
        for provider in &extension.infrastructure_providers {
            let key = (provider.contract.clone(), provider.provider_code.clone());
            if self.infrastructure_providers.contains_key(&key)
                || !declared_provider_keys.insert(key.clone())
            {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "duplicate infrastructure provider: {}/{}",
                    provider.contract, provider.provider_code
                )));
            }
            if self
                .default_infrastructure_providers
                .contains_key(&provider.contract)
                || !declared_default_contracts.insert(provider.contract.clone())
            {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "default provider already registered for {}",
                    provider.contract
                )));
            }
        }
        for provider in &extension.auth_providers {
            if self.auth_providers.contains_key(&provider.auth_type) {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "duplicate auth provider: {}",
                    provider.auth_type
                )));
            }
        }

        for provider in &extension.infrastructure_providers {
            let key = (provider.contract.clone(), provider.provider_code.clone());
            self.default_infrastructure_providers
                .insert(provider.contract.clone(), key.clone());
            self.infrastructure_providers.insert(
                key,
                RegisteredInfrastructureProvider {
                    extension_id: extension.extension_id.clone(),
                    contract: provider.contract.clone(),
                    provider_code: provider.provider_code.clone(),
                    config_ref: provider.config_ref.clone(),
                },
            );
        }
        for provider in &extension.auth_providers {
            self.auth_providers.insert(
                provider.auth_type.clone(),
                (extension.extension_id.clone(), provider.clone()),
            );
        }

        self.extensions
            .insert(extension.extension_id.clone(), extension);

        Ok(())
    }

    pub fn infrastructure_provider(
        &self,
        contract: &str,
        provider_code: &str,
    ) -> Option<&RegisteredInfrastructureProvider> {
        self.infrastructure_providers
            .get(&(contract.to_string(), provider_code.to_string()))
    }

    pub fn providers_for_contract(&self, contract: &str) -> Vec<&RegisteredInfrastructureProvider> {
        self.infrastructure_providers
            .iter()
            .filter_map(|((provider_contract, _), provider)| {
                (provider_contract == contract).then_some(provider)
            })
            .collect()
    }

    pub fn extension(&self, extension_id: &str) -> Option<&RegisteredHostExtension> {
        self.extensions.get(extension_id)
    }

    pub fn auth_provider(
        &self,
        auth_type: &str,
    ) -> Option<(&str, &AuthProviderContributionManifest)> {
        self.auth_providers
            .get(auth_type)
            .map(|(extension_id, provider)| (extension_id.as_str(), provider))
    }

    pub fn auth_providers(
        &self,
    ) -> impl Iterator<Item = (&str, &AuthProviderContributionManifest)> {
        self.auth_providers
            .values()
            .map(|(extension_id, provider)| (extension_id.as_str(), provider))
    }
}
