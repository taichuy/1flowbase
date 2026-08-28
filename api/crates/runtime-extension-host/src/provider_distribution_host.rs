use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use extension_contracts::{
    PluginDataBinding, PluginDataPermission, PluginDataPort, PluginStorageBinding,
    ProviderDistributionDecision, ProviderDistributionInvocation,
    ProviderDistributionSelectionReceipt, RUNTIME_HOST_CALL_CAPABILITY_V1,
};
use extension_package_runtime::{
    error::{FrameworkResult, PluginFrameworkError},
    provider_contract::{ProviderStdioMethod, ProviderStdioRequest},
};
use runtime_core::runtime_backend::RuntimeExecutionPrincipal;
use tokio::sync::Mutex;

use crate::{
    package_loader::{LoadedProviderDistributionPackage, PackageLoader},
    stdio_runtime::{ProviderHostCallContext, ProviderWorker},
};

struct LoadedRule {
    package: LoadedProviderDistributionPackage,
    worker: Mutex<ProviderWorker>,
}

#[derive(Default)]
pub(crate) struct ProviderDistributionHost {
    loaded: HashMap<String, Arc<LoadedRule>>,
}

impl ProviderDistributionHost {
    pub(crate) fn load(
        &mut self,
        package_root: &str,
        expected_plugin_id: &str,
    ) -> FrameworkResult<()> {
        let package = PackageLoader::load_provider_distribution(package_root)?;
        let plugin_id = package.manifest.versioned_plugin_id()?;
        if plugin_id != expected_plugin_id {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "loaded distribution package id {plugin_id} does not match requested {expected_plugin_id}"
            )));
        }
        let worker = ProviderWorker::new(
            package.runtime_executable.clone(),
            package.manifest.runtime.limits.clone(),
        );
        let rule_id = package.manifest.provider_distribution_rules[0]
            .rule_id
            .clone();
        self.loaded.insert(
            rule_id,
            Arc::new(LoadedRule {
                package,
                worker: Mutex::new(worker),
            }),
        );
        Ok(())
    }

    pub(crate) async fn unload(&mut self, plugin_id: &str) -> FrameworkResult<()> {
        let rule_id = self.loaded.iter().find_map(|(rule_id, loaded)| {
            (loaded
                .package
                .manifest
                .versioned_plugin_id()
                .ok()
                .as_deref()
                == Some(plugin_id))
            .then(|| rule_id.clone())
        });
        if let Some(loaded) = rule_id.and_then(|rule_id| self.loaded.remove(&rule_id)) {
            loaded.worker.lock().await.stop().await;
        }
        Ok(())
    }

    pub(crate) async fn select(
        &self,
        plugin_id: &str,
        invocation: ProviderDistributionInvocation,
        principal: RuntimeExecutionPrincipal,
        plugin_data: Arc<dyn PluginDataPort>,
    ) -> FrameworkResult<ProviderDistributionSelectionReceipt> {
        invocation
            .validate()
            .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?;
        let loaded = self.loaded.get(plugin_id).cloned().ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(format!(
                "distribution runtime is not loaded: {plugin_id}"
            ))
        })?;
        let request = ProviderStdioRequest {
            method: ProviderStdioMethod::SelectDistribution,
            input: serde_json::to_value(&invocation).map_err(|error| {
                PluginFrameworkError::invalid_provider_package(error.to_string())
            })?,
        };
        let host_calls =
            distribution_host_calls(&loaded.package, &invocation, principal, plugin_data)?;
        let value = loaded
            .worker
            .lock()
            .await
            .call_with_host_calls(
                &request,
                &loaded.package.manifest.runtime.limits,
                host_calls,
            )
            .await?;
        let decision: ProviderDistributionDecision =
            serde_json::from_value(value).map_err(|error| {
                PluginFrameworkError::invalid_provider_package(format!(
                    "invalid distribution decision: {error}"
                ))
            })?;
        if let ProviderDistributionDecision::Select { target_id } = &decision {
            if !invocation
                .candidates
                .iter()
                .any(|candidate| candidate.ready && candidate.target_id == *target_id)
            {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "distribution runtime selected an ineligible target",
                ));
            }
        }
        Ok(ProviderDistributionSelectionReceipt {
            invocation_id: invocation.invocation_id,
            rule_id: invocation.rule_id,
            rule_version: invocation.rule_version,
            contract_version: invocation.contract_version,
            registry_fingerprint: invocation.registry_fingerprint,
            attempt: invocation.attempt,
            decision,
        })
    }
}

fn distribution_host_calls(
    package: &LoadedProviderDistributionPackage,
    invocation: &ProviderDistributionInvocation,
    principal: RuntimeExecutionPrincipal,
    plugin_data: Arc<dyn PluginDataPort>,
) -> FrameworkResult<Option<ProviderHostCallContext>> {
    let manifest = &package.manifest;
    if !manifest
        .runtime
        .capabilities
        .iter()
        .any(|capability| capability == RUNTIME_HOST_CALL_CAPABILITY_V1)
    {
        return Ok(None);
    }
    if manifest.data_models.len() != 1
        || manifest.data_models[0].storage_binding != PluginStorageBinding::Main
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "distribution runtime requires one main plugin data model binding",
        ));
    }
    Ok(Some(ProviderHostCallContext {
        binding: PluginDataBinding {
            publisher_namespace: manifest.publisher_namespace.clone(),
            plugin_code: manifest.plugin_code()?.to_string(),
            plugin_version: manifest.version.clone(),
            storage_binding: "main".to_string(),
            workspace_id: principal.workspace_id,
            actor_id: principal.actor_id,
            provider_instance_id: invocation.routing_policy_id.clone(),
            permissions: BTreeSet::from([PluginDataPermission::Read, PluginDataPermission::Write]),
            deadline_unix_ms: principal.deadline_unix_ms,
        },
        plugin_data,
    }))
}
