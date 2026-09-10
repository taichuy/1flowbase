//! Root 2014 AC-013: formal install/enable against the real PostgreSQL selection transaction.
use anyhow::Result;
use control_plane::{
    plugin_management::{
        DisablePluginCommand, EnablePluginCommand, InstallPluginCommand, PluginManagementService,
        SwitchPluginVersionCommand,
    },
    ports::{ProviderRuntimeInvocationOutput, ProviderRuntimePort},
};
use control_plane_contracts::ports::*;
use domain::{LocalPluginInstallationRecord, PluginDesiredState, PluginRuntimeStatus};
use plugin_framework::provider_contract::{ProviderInvocationInput, ProviderModelDescriptor};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

#[derive(Clone)]
struct RestartOnlyRuntime;
#[async_trait::async_trait]
impl ProviderRuntimePort for RestartOnlyRuntime {
    async fn ensure_loaded(&self, _: &LocalPluginInstallationRecord) -> Result<()> {
        anyhow::bail!("native selection must not load code")
    }
    async fn deactivate_plugin(&self, _: &domain::PluginInstallationRecord) -> Result<()> {
        anyhow::bail!("native selection must not unload code")
    }
    async fn validate_provider(
        &self,
        _: &LocalPluginInstallationRecord,
        _: Value,
    ) -> Result<Value> {
        anyhow::bail!("unexpected provider operation")
    }
    async fn list_models(
        &self,
        _: &LocalPluginInstallationRecord,
        _: Value,
    ) -> Result<Vec<ProviderModelDescriptor>> {
        anyhow::bail!("unexpected provider operation")
    }
    async fn invoke_stream(
        &self,
        _: &LocalPluginInstallationRecord,
        _: ProviderInvocationInput,
    ) -> Result<ProviderRuntimeInvocationOutput> {
        anyhow::bail!("unexpected provider operation")
    }
}
struct NoDownload;
#[async_trait::async_trait]
impl OfficialPluginSourcePort for NoDownload {
    async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
        anyhow::bail!("selection must not request latest")
    }
    async fn download_plugin(
        &self,
        _: &OfficialPluginSourceEntry,
    ) -> Result<DownloadedOfficialPluginPackage> {
        anyhow::bail!("selection must not download")
    }
    fn trusted_public_keys(&self) -> Vec<extension_contracts::TrustedPublicKey> {
        Vec::new()
    }
}
type Service = PluginManagementService<PgControlPlaneStore, RestartOnlyRuntime>;
struct Fixture {
    store: PgControlPlaneStore,
    service: Service,
    actor: Uuid,
    root: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Fixture {
    async fn new() -> Self {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
        let schema = postgres_test_support::PostgresTestSchema::create(&url)
            .await
            .unwrap();
        let pool = schema.connect().await.unwrap();
        run_migrations(&pool).await.unwrap();
        let store = PgControlPlaneStore::new(pool);
        let tenant = store.upsert_root_tenant().await.unwrap();
        let workspace = store
            .upsert_workspace(tenant.id, "1flowbase")
            .await
            .unwrap();
        control_plane_test_support::upsert_permission_catalog(&store)
            .await
            .unwrap();
        control_plane_test_support::upsert_builtin_roles(&store, workspace.id)
            .await
            .unwrap();
        store
            .upsert_login_entry(&domain::LoginEntryRecord {
                id: domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
                connection_id: domain::PASSWORD_LOCAL_CONNECTION_ID,
                auth_type: "password-local".into(),
                title: "Password".into(),
                enabled: true,
                is_builtin: true,
                sort_order: 0,
                public_ui_block: String::new(),
                options: serde_json::json!({}),
            })
            .await
            .unwrap();
        let actor = store
            .upsert_root_user(
                workspace.id,
                "root",
                "root@example.com",
                "$argon2id$v=19$m=19456,t=2,p=1$test$test",
                "Root",
                "Root",
            )
            .await
            .unwrap();
        let root = std::env::temp_dir().join(format!("root-2014-native-{}", Uuid::now_v7()));
        let service = PluginManagementService::new(
            store.clone(),
            RestartOnlyRuntime,
            Arc::new(NoDownload),
            root.join("installed-root"),
        )
        .with_node_id("native-node");
        Self {
            store,
            service,
            actor: actor.id,
            root,
        }
    }
    async fn install(&self, version: &str) -> domain::PluginInstallationRecord {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../plugins/fixtures/northwind.settings-page");
        let package = self.root.join(version);
        std::fs::create_dir_all(&package).unwrap();
        for file in ["manifest.yaml", "host-extension.yaml", "settings.tsx"] {
            let content = std::fs::read_to_string(source.join(file))
                .unwrap()
                .replace("1.0.0", version);
            std::fs::write(package.join(file), content).unwrap();
        }
        self.service
            .install_plugin(InstallPluginCommand {
                actor_user_id: self.actor,
                package_root: package.display().to_string(),
            })
            .await
            .unwrap()
            .installation
    }
    async fn enable(&self, id: Uuid) {
        self.service
            .enable_plugin(EnablePluginCommand {
                actor_user_id: self.actor,
                installation_id: id,
            })
            .await
            .unwrap();
    }
    async fn target(&self) -> domain::NativePluginTarget {
        self.store
            .list_native_plugin_targets()
            .await
            .unwrap()
            .pop()
            .unwrap()
    }
}

#[tokio::test]
async fn root_2014_ac_013_native_selection_unique_exact_target() {
    let f = Fixture::new().await;
    let v1 = f.install("1.0.0").await;
    let v2 = f.install("2.0.0").await;
    assert_eq!(v1.desired_state, PluginDesiredState::Disabled);
    assert!(f
        .store
        .list_native_plugin_targets()
        .await
        .unwrap()
        .is_empty());
    f.enable(v1.id).await;
    let first = f.target().await;
    f.store
        .complete_native_plugin_startup(&first, "native-node", PluginRuntimeStatus::Active, None)
        .await
        .unwrap();
    f.service
        .disable_plugin(DisablePluginCommand {
            actor_user_id: f.actor,
            installation_id: v1.id,
        })
        .await
        .unwrap();
    assert_eq!(
        f.store
            .get_artifact_instance("native-node", v1.id)
            .await
            .unwrap()
            .unwrap()
            .runtime_status,
        PluginRuntimeStatus::Active
    );
    f.enable(v1.id).await;
    assert_eq!(
        f.target().await.application_generation,
        first.application_generation
    );
    f.service
        .switch_version(SwitchPluginVersionCommand {
            actor_user_id: f.actor,
            provider_code: v2.provider_code.clone(),
            target_installation_id: v2.id,
        })
        .await
        .unwrap();
    let switched = f.target().await;
    assert_eq!(switched.installation_id, v2.id);
    assert_eq!(
        switched.application_generation,
        first.application_generation + 1
    );
    assert_eq!(
        f.store
            .get_artifact_instance("native-node", v1.id)
            .await
            .unwrap()
            .unwrap()
            .runtime_status,
        PluginRuntimeStatus::Active
    );
    f.service
        .disable_plugin(DisablePluginCommand {
            actor_user_id: f.actor,
            installation_id: v2.id,
        })
        .await
        .unwrap();
    f.enable(v2.id).await;
    let reenabled = f.target().await;
    assert_eq!(
        reenabled.application_generation,
        switched.application_generation
    );
    assert!(reenabled.selection_revision > switched.selection_revision);
    assert_eq!(f.store.list_installations().await.unwrap().len(), 2);
    assert!(f
        .store
        .get_artifact_instance("native-node", v1.id)
        .await
        .unwrap()
        .is_some());
    // Explicit older installation is also a version switch, never an automatic rollback.
    f.enable(v1.id).await;
    assert_eq!(
        f.target().await.application_generation,
        switched.application_generation + 1
    );
}

#[tokio::test]
async fn root_2014_ac_013_concurrent_native_selection_rejects_stale_revision() {
    let f = Fixture::new().await;
    let v1 = f.install("1.0.0").await;
    let v2 = f.install("2.0.0").await;
    f.enable(v1.id).await;
    let stale = f.target().await;
    let barrier = tokio::sync::Barrier::new(2);
    let ((), ()) = tokio::join!(
        async {
            barrier.wait().await;
            f.enable(v1.id).await;
        },
        async {
            barrier.wait().await;
            f.enable(v2.id).await;
        }
    );
    let rows = f.store.list_native_plugin_targets().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].selection_revision, stale.selection_revision + 2);
    let error = f
        .store
        .complete_native_plugin_startup(&stale, "native-node", PluginRuntimeStatus::Active, None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("native_plugin_target_changed"));
    assert_eq!(f.target().await, rows[0]);
    assert_eq!(
        f.store
            .get_artifact_instance("native-node", v1.id)
            .await
            .unwrap()
            .unwrap()
            .runtime_status,
        PluginRuntimeStatus::Inactive
    );
    assert_eq!(f.store.list_installations().await.unwrap().len(), 2);
    f.store
        .complete_native_plugin_startup(&rows[0], "native-node", PluginRuntimeStatus::Active, None)
        .await
        .unwrap();
}
