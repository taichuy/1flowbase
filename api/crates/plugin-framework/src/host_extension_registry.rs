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
    pub provides_contracts: Vec<String>,
    pub overrides_contracts: Vec<String>,
    pub registers_slots: Vec<String>,
    pub registers_storage: Vec<(String, String)>,
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
    contracts: BTreeMap<String, String>,
    slots: BTreeMap<String, String>,
    storage: BTreeMap<String, String>,
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

        for contract in &extension.provides_contracts {
            if self.contracts.contains_key(contract) {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "host contract {contract} already registered"
                )));
            }
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

        for contract in &extension.provides_contracts {
            self.contracts
                .insert(contract.clone(), extension.extension_id.clone());
        }

        for contract in &extension.overrides_contracts {
            self.contracts
                .insert(contract.clone(), extension.extension_id.clone());
        }

        for slot in &extension.registers_slots {
            self.slots
                .insert(slot.clone(), extension.extension_id.clone());
        }

        for (kind, implementation) in &extension.registers_storage {
            self.storage.insert(kind.clone(), implementation.clone());
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

    pub fn contract_provider(&self, contract: &str) -> Option<&str> {
        self.contracts.get(contract).map(String::as_str)
    }

    pub fn slot_provider(&self, slot: &str) -> Option<&str> {
        self.slots.get(slot).map(String::as_str)
    }

    pub fn storage_implementation(&self, kind: &str) -> Option<&str> {
        self.storage.get(kind).map(String::as_str)
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
