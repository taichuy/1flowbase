use std::collections::HashMap;

use plugin_framework::error::{FrameworkResult, PluginFrameworkError};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    capability_stdio::{call_executable, CapabilityStdioMethod, CapabilityStdioRequest},
    package_loader::{LoadedCapabilityPackage, PackageLoader},
};

#[derive(Debug, Clone, Serialize)]
pub struct LoadedCapabilitySummary {
    pub plugin_id: String,
    pub plugin_version: String,
    pub execution_mode: String,
    pub node_contribution_count: usize,
}

impl LoadedCapabilitySummary {
    fn from_loaded(loaded: &LoadedCapabilityPackage) -> Self {
        Self {
            plugin_id: loaded.identifier(),
            plugin_version: loaded.manifest.version.clone(),
            execution_mode: loaded.manifest.execution_mode.as_str().to_string(),
            node_contribution_count: loaded.manifest.node_contributions.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityValueOutput {
    pub output: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityExecutionOutput {
    pub output_payload: Value,
}

#[derive(Debug, Default)]
pub struct CapabilityHost {
    loaded_packages: HashMap<String, LoadedCapabilityPackage>,
}

impl CapabilityHost {
    pub fn load(
        &mut self,
        package_root: impl AsRef<std::path::Path>,
    ) -> FrameworkResult<LoadedCapabilitySummary> {
        let loaded = PackageLoader::load_capability(package_root)?;
        let summary = LoadedCapabilitySummary::from_loaded(&loaded);
        self.loaded_packages
            .insert(summary.plugin_id.clone(), loaded);
        Ok(summary)
    }

    pub fn load_legacy_installed(
        &mut self,
        package_root: impl AsRef<std::path::Path>,
        eligibility: &plugin_framework::LegacyInstalledManifestEligibility,
    ) -> FrameworkResult<LoadedCapabilitySummary> {
        let loaded = PackageLoader::load_legacy_installed_capability(package_root, eligibility)?;
        let summary = LoadedCapabilitySummary::from_loaded(&loaded);
        self.loaded_packages
            .insert(summary.plugin_id.clone(), loaded);
        Ok(summary)
    }

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
        let loaded = self.loaded_package(plugin_id)?;
        self.ensure_contribution(loaded, contribution_code)?;
        let loaded = loaded.clone();
        let plugin_id = plugin_id.to_string();
        let contribution_code = contribution_code.to_string();
        Ok(async move {
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
        let loaded = self.loaded_package(plugin_id)?;
        self.ensure_contribution(loaded, contribution_code)?;
        let loaded = loaded.clone();
        let plugin_id = plugin_id.to_string();
        let contribution_code = contribution_code.to_string();
        Ok(async move {
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
        let loaded = self.loaded_package(plugin_id)?;
        self.ensure_contribution(loaded, contribution_code)?;
        let loaded = loaded.clone();
        let plugin_id = plugin_id.to_string();
        let contribution_code = contribution_code.to_string();
        Ok(async move {
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
        let loaded = self.loaded_package(plugin_id)?;
        self.ensure_contribution(loaded, contribution_code)?;
        let loaded = loaded.clone();
        let plugin_id = plugin_id.to_string();
        let contribution_code = contribution_code.to_string();
        Ok(async move {
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

impl CapabilityHost {
    pub fn with_loaded_package(
        plugin_id: impl Into<String>,
        loaded: LoadedCapabilityPackage,
    ) -> Self {
        let mut loaded_packages = HashMap::new();
        loaded_packages.insert(plugin_id.into(), loaded);
        Self { loaded_packages }
    }
}
