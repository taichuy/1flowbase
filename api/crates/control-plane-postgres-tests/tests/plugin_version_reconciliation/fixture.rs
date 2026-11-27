use super::*;
use anyhow::{bail, Result};
use control_plane::ports::{ProviderRuntimeInvocationOutput, ProviderRuntimePort};
use domain::{LocalPluginInstallationRecord, PluginInstallationRecord};
use plugin_framework::provider_contract::{ProviderInvocationInput, ProviderModelDescriptor};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use storage_durable_postgres::PgControlPlaneStore;

pub(super) const NODE: &str = "version-reconciliation";
pub(super) type State = (
    PluginDesiredState,
    PluginRuntimeStatus,
    PluginAvailabilityStatus,
);
pub(super) const ACTIVE: State = (
    PluginDesiredState::ActiveRequested,
    PluginRuntimeStatus::Active,
    PluginAvailabilityStatus::Available,
);
pub(super) const DISABLED: State = (
    PluginDesiredState::Disabled,
    PluginRuntimeStatus::Inactive,
    PluginAvailabilityStatus::Disabled,
);

pub(super) type LoadHook =
    Box<dyn FnOnce(Uuid) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send>;

#[derive(Clone, Default)]
pub(super) struct Runtime {
    pub(super) activated: Arc<Mutex<Vec<Uuid>>>,
    pub(super) retired: Arc<Mutex<Vec<Uuid>>>,
    pub(super) on_ensure_loaded: Arc<Mutex<Option<(Uuid, LoadHook)>>>,
}

#[async_trait::async_trait]
impl ProviderRuntimePort for Runtime {
    async fn reconcile_provider_worker_demand(
        &self,
        _: &PluginInstallationRecord,
        _: u64,
        _: Option<bool>,
    ) -> Result<()> {
        Ok(())
    }
    async fn ensure_loaded(
        &self,
        local: &LocalPluginInstallationRecord,
    ) -> Result<Option<domain::LoadedRuntimeArtifactIdentity>> {
        plugin_framework::provider_package::ProviderPackage::load_from_dir(
            local.local_path().expect("ready local package"),
        )?;
        self.activated.lock().unwrap().push(local.id);
        let hook = {
            let mut guard = self.on_ensure_loaded.lock().unwrap();
            if guard
                .as_ref()
                .is_some_and(|(target_id, _)| *target_id == local.id)
            {
                guard.take().map(|(_, hook)| hook)
            } else {
                None
            }
        };
        if let Some(hook) = hook {
            hook(local.id).await;
        }
        Ok(None)
    }
    async fn deactivate_plugin(&self, installation: &PluginInstallationRecord) -> Result<()> {
        self.retired.lock().unwrap().push(installation.id);
        Ok(())
    }
    async fn validate_provider(
        &self,
        _: &LocalPluginInstallationRecord,
        _: Value,
    ) -> Result<Value> {
        bail!("unexpected validation")
    }
    async fn list_models(
        &self,
        _: &LocalPluginInstallationRecord,
        _: Value,
    ) -> Result<Vec<ProviderModelDescriptor>> {
        bail!("unexpected discovery")
    }
    async fn invoke_stream(
        &self,
        _: &LocalPluginInstallationRecord,
        _: ProviderInvocationInput,
    ) -> Result<ProviderRuntimeInvocationOutput> {
        bail!("unexpected invocation")
    }
}

struct NoDownload;
#[async_trait::async_trait]
impl OfficialPluginSourcePort for NoDownload {
    async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
        bail!("offline fixture")
    }
    async fn download_plugin(
        &self,
        _: &OfficialPluginSourceEntry,
    ) -> Result<DownloadedOfficialPluginPackage> {
        bail!("test must not download")
    }
    fn trusted_public_keys(&self) -> Vec<extension_contracts::TrustedPublicKey> {
        Vec::new()
    }
}

pub(super) struct Fixture {
    pub(super) store: PgControlPlaneStore,
    pub(super) workspace: domain::WorkspaceRecord,
    pub(super) actor: Uuid,
    pub(super) runtime: Runtime,
    pub(super) service: PluginManagementService<PgControlPlaneStore, Runtime>,
    pub(super) root: PathBuf,
}
impl Fixture {
    pub(super) async fn new() -> Self {
        let (store, workspace, actor) = super::support::seed_store().await;
        let root = std::env::temp_dir().join(format!("plugin-version-pg-{}", Uuid::now_v7()));
        let runtime = Runtime::default();
        let service = PluginManagementService::new(
            store.clone(),
            runtime.clone(),
            Arc::new(NoDownload),
            root.join("artifacts"),
        )
        .with_node_id(NODE);
        Self {
            store,
            workspace,
            actor: actor.id,
            runtime,
            service,
            root,
        }
    }
    pub(super) async fn install(&self, code: &str, version: &str) -> Uuid {
        let root = self.root.join("packages").join(code).join(version);
        for dir in ["provider", "bin", "i18n"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        fs::write(
            root.join("manifest.yaml"),
            format!(
                r#"manifest_version: 1
plugin_id: {code}@{version}
version: {version}
publisher_namespace: 1flowbase-tests
vendor: 1flowbase tests
display_name: Fixture Provider
description: Fixture Provider
icon: icon.svg
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes: [model_provider]
binding_targets: [workspace]
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/provider
"#
            ),
        )
        .unwrap();
        fs::write(
            root.join("icon.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        )
        .unwrap();
        fs::write(root.join("bin/provider"), "#!/bin/sh\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.join("bin/provider"), fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        fs::write(
            root.join(format!("provider/{code}.yaml")),
            format!(
                r#"provider_code: {code}
display_name: Fixture Provider
protocol: openai_compatible
model_discovery: hybrid
parameter_form:
  schema_version: 1.0.0
  title: LLM Parameters
  fields:
    - key: temperature
      label: Temperature
      type: number
      send_mode: optional
      enabled_by_default: true
config_schema:
  - key: base_url
    type: string
    required: true
"#
            ),
        )
        .unwrap();
        fs::write(root.join("i18n/en_US.json"), "{}\n").unwrap();
        plugin_framework::provider_package::ProviderPackage::load_from_dir(&root).unwrap();
        self.service
            .install_plugin(InstallPluginCommand {
                actor_user_id: self.actor,
                package_root: root.display().to_string(),
            })
            .await
            .unwrap()
            .installation
            .id
    }
    pub(super) async fn enable(&self, id: Uuid) {
        self.service
            .enable_plugin(EnablePluginCommand {
                actor_user_id: self.actor,
                installation_id: id,
            })
            .await
            .unwrap();
    }
    pub(super) async fn switch(&self, actor: Uuid, id: Uuid) {
        self.service
            .switch_version(SwitchPluginVersionCommand {
                actor_user_id: actor,
                provider_code: "fixture_provider".into(),
                target_installation_id: id,
            })
            .await
            .unwrap();
    }
    pub(super) async fn assert_state(&self, id: Uuid, expected: State) {
        let local = self
            .store
            .get_local_installation(NODE, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                local.desired_state,
                local.artifact.runtime_status,
                local.artifact.availability_status
            ),
            expected
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
