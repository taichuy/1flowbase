use std::{collections::HashMap, sync::Arc};

use extension_package_runtime::error::{FrameworkResult, PluginFrameworkError};
use runtime_core::runtime_backend::LoadedRuntimeArtifactIdentity;
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    capability_stdio::{call_executable, CapabilityStdioMethod, CapabilityStdioRequest},
    package_loader::{
        cached_identity_matches, verify_package_activation, LoadedCapabilityPackage, PackageLoader,
    },
    plugin_scope::PluginScope,
};

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityValueOutput {
    pub output: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityExecutionOutput {
    pub output_payload: Value,
    pub granted_credit_permissions: std::collections::BTreeSet<String>,
}

#[derive(Debug, Default)]
pub struct CapabilityHost {
    loaded_packages: HashMap<String, LoadedCapabilityPackage>,
    scopes: HashMap<String, Arc<PluginScope>>,
    next_generation: HashMap<String, u64>,
}

impl CapabilityHost {
    pub fn loaded_count(&self) -> usize {
        self.loaded_packages.len()
    }

    pub async fn stop_all(&mut self) -> FrameworkResult<()> {
        let plugin_ids = self.loaded_packages.keys().cloned().collect::<Vec<_>>();
        for plugin_id in plugin_ids {
            self.unload(&plugin_id).await?;
        }
        Ok(())
    }

    pub async fn unload(&mut self, plugin_id: &str) -> FrameworkResult<()> {
        let scope = self.scopes.remove(plugin_id);
        if let Some(scope) = scope {
            scope.dispose().await?;
        }
        self.loaded_packages.remove(plugin_id);
        Ok(())
    }

    pub(crate) async fn load_if_needed(
        &mut self,
        package_root: &std::path::Path,
        plugin_id: &str,
        expected_fingerprint: Option<&str>,
        legacy_eligibility: Option<&extension_package_runtime::LegacyInstalledManifestEligibility>,
    ) -> FrameworkResult<LoadedRuntimeArtifactIdentity> {
        let package_root = std::fs::canonicalize(package_root)
            .map_err(|error| PluginFrameworkError::io(Some(package_root), error.to_string()))?;
        if let Some(loaded) = self.loaded_packages.get(plugin_id) {
            if loaded.package_root == package_root
                && cached_identity_matches(&package_root, &loaded.identity, expected_fingerprint)?
            {
                return Ok(loaded.identity.clone());
            }
        }
        let loaded = match legacy_eligibility {
            Some(eligibility) => {
                PackageLoader::load_legacy_installed_capability(&package_root, eligibility)?
            }
            None => PackageLoader::load_capability(&package_root)?,
        };
        verify_package_activation(
            &loaded.identifier(),
            plugin_id,
            &loaded.identity,
            expected_fingerprint,
        )?;
        let identity = loaded.identity.clone();
        self.replace_loaded(plugin_id.to_owned(), loaded).await?;
        Ok(identity)
    }

    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "internal test convenience wrapper; production uses the admitted operation future"
    )]
    pub async fn validate_config(
        &self,
        plugin_id: &str,
        contribution_code: &str,
        config_payload: Value,
    ) -> FrameworkResult<CapabilityValueOutput> {
        self.validate_config_operation(plugin_id, contribution_code, config_payload)?
            .await
    }

    pub fn validate_config_operation(
        &self,
        plugin_id: &str,
        contribution_code: &str,
        config_payload: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<CapabilityValueOutput>> + Send + 'static,
    > {
        let lease = self.scope(plugin_id)?.admit()?;
        let loaded = self.loaded_package(plugin_id)?;
        self.ensure_contribution(loaded, contribution_code)?;
        let loaded = loaded.clone();
        let plugin_id = plugin_id.to_string();
        let contribution_code = contribution_code.to_string();
        Ok(async move {
            let _lease = lease;
            let output = Self::call_runtime_loaded(
                loaded,
                CapabilityStdioMethod::ValidateConfig,
                json!({
                    "plugin_id": plugin_id,
                    "contribution_code": contribution_code,
                    "config_payload": config_payload,
                }),
            )
            .await?;
            Ok(CapabilityValueOutput { output })
        })
    }

    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "internal test convenience wrapper; production uses the admitted operation future"
    )]
    pub async fn resolve_dynamic_options(
        &self,
        plugin_id: &str,
        contribution_code: &str,
        config_payload: Value,
    ) -> FrameworkResult<CapabilityValueOutput> {
        self.resolve_dynamic_options_operation(plugin_id, contribution_code, config_payload)?
            .await
    }

    pub fn resolve_dynamic_options_operation(
        &self,
        plugin_id: &str,
        contribution_code: &str,
        config_payload: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<CapabilityValueOutput>> + Send + 'static,
    > {
        let lease = self.scope(plugin_id)?.admit()?;
        let loaded = self.loaded_package(plugin_id)?;
        self.ensure_contribution(loaded, contribution_code)?;
        let loaded = loaded.clone();
        let plugin_id = plugin_id.to_string();
        let contribution_code = contribution_code.to_string();
        Ok(async move {
            let _lease = lease;
            let output = Self::call_runtime_loaded(
                loaded,
                CapabilityStdioMethod::ResolveDynamicOptions,
                json!({
                    "plugin_id": plugin_id,
                    "contribution_code": contribution_code,
                    "config_payload": config_payload,
                }),
            )
            .await?;
            Ok(CapabilityValueOutput { output })
        })
    }

    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "internal test convenience wrapper; production uses the admitted operation future"
    )]
    pub async fn resolve_output_schema(
        &self,
        plugin_id: &str,
        contribution_code: &str,
        config_payload: Value,
    ) -> FrameworkResult<CapabilityValueOutput> {
        self.resolve_output_schema_operation(plugin_id, contribution_code, config_payload)?
            .await
    }

    pub fn resolve_output_schema_operation(
        &self,
        plugin_id: &str,
        contribution_code: &str,
        config_payload: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<CapabilityValueOutput>> + Send + 'static,
    > {
        let lease = self.scope(plugin_id)?.admit()?;
        let loaded = self.loaded_package(plugin_id)?;
        self.ensure_contribution(loaded, contribution_code)?;
        let loaded = loaded.clone();
        let plugin_id = plugin_id.to_string();
        let contribution_code = contribution_code.to_string();
        Ok(async move {
            let _lease = lease;
            let output = Self::call_runtime_loaded(
                loaded,
                CapabilityStdioMethod::ResolveOutputSchema,
                json!({
                    "plugin_id": plugin_id,
                    "contribution_code": contribution_code,
                    "config_payload": config_payload,
                }),
            )
            .await?;
            Ok(CapabilityValueOutput { output })
        })
    }

    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "internal test convenience wrapper; production uses the admitted operation future"
    )]
    pub async fn execute(
        &self,
        plugin_id: &str,
        contribution_code: &str,
        config_payload: Value,
        input_payload: Value,
    ) -> FrameworkResult<CapabilityExecutionOutput> {
        self.execute_operation(plugin_id, contribution_code, config_payload, input_payload)?
            .await
    }

    pub fn execute_operation(
        &self,
        plugin_id: &str,
        contribution_code: &str,
        config_payload: Value,
        input_payload: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<CapabilityExecutionOutput>> + Send + 'static,
    > {
        let lease = self.scope(plugin_id)?.admit()?;
        let loaded = self.loaded_package(plugin_id)?;
        self.ensure_contribution(loaded, contribution_code)?;
        let loaded = loaded.clone();
        let plugin_id = plugin_id.to_string();
        let contribution_code = contribution_code.to_string();
        Ok(async move {
            let _lease = lease;
            let granted_credit_permissions =
                loaded.manifest.permissions.credit.iter().cloned().collect();
            let output = Self::call_runtime_loaded(
                loaded,
                CapabilityStdioMethod::Execute,
                json!({
                    "plugin_id": plugin_id,
                    "contribution_code": contribution_code,
                    "config_payload": config_payload,
                    "input_payload": input_payload,
                }),
            )
            .await?;
            Ok(CapabilityExecutionOutput {
                output_payload: output,
                granted_credit_permissions,
            })
        })
    }

    fn loaded_package(&self, plugin_id: &str) -> FrameworkResult<&LoadedCapabilityPackage> {
        self.loaded_packages.get(plugin_id).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(format!(
                "capability package is not loaded: {plugin_id}"
            ))
        })
    }

    async fn replace_loaded(
        &mut self,
        plugin_id: String,
        loaded: LoadedCapabilityPackage,
    ) -> FrameworkResult<()> {
        self.unload(&plugin_id).await?;
        let next_generation = self.next_generation.entry(plugin_id.clone()).or_insert(0);
        *next_generation = next_generation.saturating_add(1);
        self.scopes
            .insert(plugin_id.clone(), PluginScope::mounted(*next_generation));
        self.loaded_packages.insert(plugin_id, loaded);
        Ok(())
    }

    fn scope(&self, plugin_id: &str) -> FrameworkResult<&Arc<PluginScope>> {
        self.scopes.get(plugin_id).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(format!(
                "capability scope is not mounted: {plugin_id}"
            ))
        })
    }

    fn ensure_contribution(
        &self,
        loaded: &LoadedCapabilityPackage,
        contribution_code: &str,
    ) -> FrameworkResult<()> {
        if loaded
            .manifest
            .node_contributions
            .iter()
            .any(|contribution| contribution.contribution_code == contribution_code)
        {
            return Ok(());
        }

        Err(PluginFrameworkError::invalid_provider_package(format!(
            "capability contribution is not loaded: {contribution_code}"
        )))
    }

    async fn call_runtime_loaded(
        loaded: LoadedCapabilityPackage,
        method: CapabilityStdioMethod,
        input: Value,
    ) -> FrameworkResult<Value> {
        let request = CapabilityStdioRequest { method, input };
        call_executable(
            &loaded.runtime_executable,
            &request,
            &loaded.manifest.runtime.limits,
        )
        .await
    }
}
