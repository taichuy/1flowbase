use std::{
    fs,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::{bail, Result};
use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    plugin_management::{
        ExtensionBootstrapDisposition, ExtensionCatalogCategory, LockedExtensionBootstrapEntry,
        PluginManagementService,
    },
    ports::{
        DownloadedOfficialPluginPackage, OfficialPluginCatalogSnapshot, OfficialPluginSourceEntry,
        OfficialPluginSourcePort,
    },
};

use super::support::{
    actor_with_permissions, MemoryPluginManagementRepository, MemoryProviderRuntime,
};

#[derive(Clone, Default)]
struct FailingCountingSource {
    remote_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl OfficialPluginSourcePort for FailingCountingSource {
    async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
        self.remote_calls.fetch_add(1, Ordering::SeqCst);
        bail!("controlled bootstrap network failure")
    }

    async fn download_plugin(
        &self,
        _entry: &OfficialPluginSourceEntry,
    ) -> Result<DownloadedOfficialPluginPackage> {
        self.remote_calls.fetch_add(1, Ordering::SeqCst);
        bail!("controlled bootstrap download failure")
    }

    fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
        Vec::new()
    }
}

fn locked_entry() -> LockedExtensionBootstrapEntry {
    LockedExtensionBootstrapEntry {
        category: ExtensionCatalogCategory::RuntimeExtensions,
        artifact_kind: "model_provider".into(),
        id: "1flowbase.fixture_provider".into(),
        version: "0.1.0".into(),
        checksum: format!("sha256:{}", "a".repeat(64)),
        source: "official_registry".into(),
        artifact_url: "https://example.test/fixture.1flowbasepkg".into(),
        installed_path: "installed/fixture_provider/0.1.0".into(),
        bundled_path: "bootstrap/fixture-provider-linux-amd64.1flowbasepkg".into(),
        bootstrap: true,
    }
}

#[tokio::test]
async fn ac_boot_3_and_5_local_artifact_is_idempotent_and_never_fetches_or_repairs() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.configure.all"],
    ));
    let install_root = std::env::temp_dir().join(format!("bootstrap-local-{}", Uuid::now_v7()));
    let local_path = install_root.join("installed/fixture_provider/0.1.0");
    fs::create_dir_all(&local_path).unwrap();
    fs::write(local_path.join("developer-change.txt"), "do not replace").unwrap();
    let source = FailingCountingSource::default();
    let calls = Arc::clone(&source.remote_calls);
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(source),
        &install_root,
    );

    for _ in 0..2 {
        let result = service
            .bootstrap_locked_extensions(repository.actor.user_id, &[locked_entry()])
            .await;
        assert_eq!(
            result[0].disposition,
            ExtensionBootstrapDisposition::LocalArtifactPresent
        );
    }

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        fs::read_to_string(local_path.join("developer-change.txt")).unwrap(),
        "do not replace"
    );
    let _ = fs::remove_dir_all(install_root);
}

#[tokio::test]
async fn ac_boot_4_remote_failure_returns_structured_warning_instead_of_error() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryPluginManagementRepository::new(actor_with_permissions(
        workspace_id,
        &["plugin_config.configure.all"],
    ));
    let install_root = std::env::temp_dir().join(format!("bootstrap-warning-{}", Uuid::now_v7()));
    let source = FailingCountingSource::default();
    let calls = Arc::clone(&source.remote_calls);
    let service = PluginManagementService::new(
        repository.clone(),
        MemoryProviderRuntime::default(),
        Arc::new(source),
        &install_root,
    );

    let result = service
        .bootstrap_locked_extensions(repository.actor.user_id, &[locked_entry()])
        .await;

    assert_eq!(result[0].disposition, ExtensionBootstrapDisposition::Warned);
    assert_eq!(result[0].warning.as_ref().unwrap().stage, "remote_fetch");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let _ = fs::remove_dir_all(install_root);
}

#[test]
fn ac_boot_7_catalog_category_projects_binding_to_domain_owner() {
    let application = ExtensionCatalogCategory::RuntimeExtensions.application();
    assert!(application.installs_node_artifact);
    assert_eq!(
        application.binding_owner,
        crate::plugin_management::ExtensionDomainBindingOwner::RuntimeExtension
    );
    assert_eq!(
        ExtensionCatalogCategory::Mcp.application().binding_owner,
        crate::plugin_management::ExtensionDomainBindingOwner::Mcp
    );
}
