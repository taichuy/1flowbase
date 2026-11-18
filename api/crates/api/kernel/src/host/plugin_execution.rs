//! Concrete plugin lifecycle codegen shared by HTTP domains and host startup.
//! Callers retain service construction, policy and operation ownership.

use control_plane::plugin_management::{
    AssignPluginCommand, DisablePluginCommand, EnablePluginCommand, EnsureBuiltinPluginCommand,
    InstallCurrentNodePluginArtifactCommand, InstallExtensionNodePluginCommand,
    InstallPluginCommand, InstallPluginResult, InstallResolvedOfficialPluginCommand,
    InstallUploadedPluginCommand, PluginManagementService, RefreshCurrentNodePluginArtifactCommand,
    SwitchPluginVersionCommand, UpgradeLatestPluginFamilyCommand,
};
use storage_durable_postgres::MainDurableStore;

use super::provider_runtime::ApiProviderRuntime;

type PluginService = PluginManagementService<MainDurableStore, ApiProviderRuntime>;

pub fn with_managed_activation(service: PluginService) -> PluginService {
    service.with_managed_activation()
}

pub async fn ensure_builtin_plugin(
    service: &PluginService,
    command: EnsureBuiltinPluginCommand,
) -> anyhow::Result<domain::PluginInstallationRecord> {
    service.ensure_builtin_plugin(command).await
}

pub async fn reconcile_all_installations(service: &PluginService) -> anyhow::Result<()> {
    service.reconcile_all_installations().await
}

pub async fn install_plugin(
    service: &PluginService,
    command: InstallPluginCommand,
) -> anyhow::Result<InstallPluginResult> {
    service.install_plugin(command).await
}

pub async fn install_uploaded_plugin(
    service: &PluginService,
    command: InstallUploadedPluginCommand,
) -> anyhow::Result<InstallPluginResult> {
    service.install_uploaded_plugin(command).await
}

pub async fn install_resolved_official_plugin(
    service: &PluginService,
    command: InstallResolvedOfficialPluginCommand,
) -> anyhow::Result<InstallPluginResult> {
    service.install_resolved_official_plugin(command).await
}

pub async fn install_current_node_artifact(
    service: &PluginService,
    command: InstallCurrentNodePluginArtifactCommand,
) -> anyhow::Result<domain::PluginArtifactInstanceRecord> {
    service.install_current_node_artifact(command).await
}

pub async fn refresh_current_node_artifact(
    service: &PluginService,
    command: RefreshCurrentNodePluginArtifactCommand,
) -> anyhow::Result<domain::PluginArtifactInstanceRecord> {
    service.refresh_current_node_artifact(command).await
}

pub async fn upgrade_latest(
    service: &PluginService,
    command: UpgradeLatestPluginFamilyCommand,
) -> anyhow::Result<domain::PluginTaskRecord> {
    service.upgrade_latest(command).await
}

pub async fn switch_version(
    service: &PluginService,
    command: SwitchPluginVersionCommand,
) -> anyhow::Result<domain::PluginTaskRecord> {
    service.switch_version(command).await
}

pub async fn enable_plugin(
    service: &PluginService,
    command: EnablePluginCommand,
) -> anyhow::Result<domain::PluginTaskRecord> {
    service.enable_plugin(command).await
}

pub async fn disable_plugin(
    service: &PluginService,
    command: DisablePluginCommand,
) -> anyhow::Result<domain::PluginTaskRecord> {
    service.disable_plugin(command).await
}

pub async fn assign_plugin(
    service: &PluginService,
    command: AssignPluginCommand,
) -> anyhow::Result<domain::PluginTaskRecord> {
    service.assign_plugin(command).await
}

pub async fn install_uploaded_model_provider(
    service: &PluginService,
    command: InstallUploadedPluginCommand,
) -> anyhow::Result<InstallPluginResult> {
    service.install_uploaded_model_provider(command).await
}

pub async fn install_uploaded_network_egress_provider(
    service: &PluginService,
    command: InstallUploadedPluginCommand,
) -> anyhow::Result<InstallPluginResult> {
    service
        .install_uploaded_network_egress_provider(command)
        .await
}

pub async fn install_extension_node_plugin(
    service: &PluginService,
    command: InstallExtensionNodePluginCommand,
) -> anyhow::Result<InstallPluginResult> {
    service.install_extension_node_plugin(command).await
}
