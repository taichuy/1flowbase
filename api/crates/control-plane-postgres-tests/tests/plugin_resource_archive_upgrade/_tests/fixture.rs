use super::inputs::{sha256, Archive};
use anyhow::{bail, ensure, Context, Result};
use control_plane::{plugin_management::*, ports::*};
use domain::{ActorContext, PluginDesiredState, PluginRuntimeStatus};
use plugin_framework::{parse_plugin_manifest, provider_contract::*, PluginManifestV1};
use sqlx::{postgres::PgConnectOptions, PgPool};
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf, str::FromStr, sync::Arc};
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

pub const NODE: &str = "archive-upgrade-fixture";

/// An intentionally unavailable port, NOT a fake runtime. Upload and explicit preparation
/// do not need it; crossing this boundary fails rather than inventing worker/registry state.
pub struct RuntimeUnavailable;
#[async_trait::async_trait]
impl ProviderRuntimePort for RuntimeUnavailable {
    async fn reconcile_provider_worker_demand(
        &self,
        _: &domain::PluginInstallationRecord,
        _: u64,
        _: Option<bool>,
    ) -> Result<()> {
        bail!("fixture_runtime_unavailable")
    }
    async fn ensure_loaded(&self, _: &domain::LocalPluginInstallationRecord) -> Result<()> {
        bail!("fixture_runtime_unavailable")
    }
    async fn deactivate_plugin(&self, _: &domain::PluginInstallationRecord) -> Result<()> {
        bail!("fixture_runtime_unavailable")
    }
    async fn validate_provider(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: serde_json::Value,
    ) -> Result<serde_json::Value> {
        bail!("fixture_runtime_unavailable")
    }
    async fn list_models(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: serde_json::Value,
    ) -> Result<Vec<ProviderModelDescriptor>> {
        bail!("fixture_runtime_unavailable")
    }
    async fn invoke_stream(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: ProviderInvocationInput,
    ) -> Result<ProviderRuntimeInvocationOutput> {
        bail!("fixture_runtime_unavailable")
    }
}

struct NoDownload;
#[async_trait::async_trait]
impl OfficialPluginSourcePort for NoDownload {
    async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
        bail!("fixture must not request a release catalog")
    }
    async fn download_plugin(
        &self,
        _: &OfficialPluginSourceEntry,
    ) -> Result<DownloadedOfficialPluginPackage> {
        bail!("fixture must not download or fabricate releases")
    }
    fn trusted_public_keys(&self) -> Vec<extension_contracts::TrustedPublicKey> {
        Vec::new()
    }
}

pub struct Installed {
    pub result: InstallPluginResult,
    pub manifest: PluginManifestV1,
    pub runtime_sha256: String,
}

pub struct Fixture {
    pub pool: PgPool,
    pub store: PgControlPlaneStore,
    pub root_actor: ActorContext,
    pub member: ActorContext,
    pub service: PluginManagementService<PgControlPlaneStore, RuntimeUnavailable>,
    pub activation: ManagedPluginActivationService<PgControlPlaneStore>,
    root: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    pub async fn new() -> Result<Self> {
        let url = std::env::var("DATABASE_URL")
            .context("explicit local DATABASE_URL required; never use a production service")?;
        let options = PgConnectOptions::from_str(&url)
            .map_err(|_| anyhow::anyhow!("invalid local DATABASE_URL"))?;
        ensure!(
            matches!(options.get_host(), "127.0.0.1" | "localhost" | "::1"),
            "fixture database must be loopback-local"
        );
        let database = postgres_test_support::PostgresTestSchema::create(&url).await?;
        let pool = database.connect().await?;
        run_migrations(&pool).await?;
        let store = PgControlPlaneStore::new(pool.clone());
        let tenant = store.upsert_root_tenant().await?;
        let workspace = store
            .upsert_workspace(tenant.id, "Archive Upgrade Fixture")
            .await?;
        control_plane_test_support::upsert_permission_catalog(&store).await?;
        control_plane_test_support::upsert_builtin_roles(&store, workspace.id).await?;
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
            .await?;
        let user = store
            .upsert_root_user(
                workspace.id,
                "root",
                "root@example.invalid",
                "$argon2id$v=19$m=19456,t=2,p=1$test$test",
                "Root",
                "Root",
            )
            .await?;
        let member = store
            .create_member_with_default_role(&CreateMemberInput {
                actor_user_id: user.id,
                workspace_id: workspace.id,
                account: "fixture-member".into(),
                email: "fixture-member@example.invalid".into(),
                phone: None,
                password_hash: "$argon2id$v=19$m=19456,t=2,p=1$test$test".into(),
                name: "Fixture Member".into(),
                nickname: "Fixture Member".into(),
                introduction: String::new(),
                email_login_enabled: true,
                phone_login_enabled: false,
            })
            .await?;
        let root_actor =
            AuthRepository::load_actor_context(&store, user.id, tenant.id, workspace.id, None)
                .await?;
        let member =
            AuthRepository::load_actor_context(&store, member.id, tenant.id, workspace.id, None)
                .await?;
        ensure!(
            root_actor.is_root && !member.is_root,
            "fixture actors must come from real repository identities"
        );
        let root = std::env::temp_dir()
            .canonicalize()?
            .join(format!("ssh-archive-upgrade-{}", Uuid::now_v7()));
        fs::DirBuilder::new().mode(0o700).create(&root)?;
        let install_root = root.join("plugin-store");
        let service = PluginManagementService::new(
            store.clone(),
            RuntimeUnavailable,
            Arc::new(NoDownload),
            &install_root,
        )
        .with_node_id(NODE)
        .with_managed_activation();
        let activation =
            ManagedPluginActivationService::new(store.clone(), NODE.into(), install_root);
        Ok(Self {
            pool,
            store,
            root_actor,
            member,
            service,
            activation,
            root,
        })
    }

    pub fn authority(&self) -> PluginContributionAuthorityService<PgControlPlaneStore> {
        PluginContributionAuthorityService::new(
            self.store.clone(),
            HostContributionGrantPolicy::root_composition(),
        )
        .with_node_id(NODE)
    }

    pub async fn upload(&self, archive: &Archive, version: &str) -> Result<Installed> {
        self.upload_named(archive, "taichuy", "ssh", version).await
    }

    pub async fn upload_named(
        &self,
        archive: &Archive,
        publisher: &str,
        code: &str,
        version: &str,
    ) -> Result<Installed> {
        let result = self
            .service
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: self.root_actor.user_id,
                file_name: archive.filename.clone(),
                package_bytes: archive.bytes.clone(),
            })
            .await
            .context("production uploaded-archive installation failed")?;
        ensure!(
            result.installation.organization == publisher
                && result.installation.provider_code == code
                && result.installation.plugin_version == version,
            "unexpected installed package identity"
        );
        ensure!(
            result.installation.desired_state == PluginDesiredState::Disabled,
            "upload unexpectedly enabled the service"
        );
        ensure!(
            result.local_artifact.runtime_status == PluginRuntimeStatus::Inactive,
            "upload unexpectedly activated a runtime"
        );
        ensure!(
            result.installation.expected_checksum.as_deref()
                == Some(format!("sha256:{}", archive.sha256).as_str()),
            "installed checksum differs from archive bytes"
        );
        let local = PathBuf::from(
            result
                .local_artifact
                .local_path
                .as_ref()
                .context("installed artifact path missing")?,
        )
        .canonicalize()?;
        ensure!(
            local.starts_with(&self.root),
            "installation escaped private fixture root"
        );
        let manifest = parse_plugin_manifest(&fs::read_to_string(local.join("manifest.yaml"))?)?;
        ensure!(
            manifest.publisher_namespace == publisher
                && manifest.plugin_code()? == code
                && manifest.version == version,
            "archive manifest identity mismatch"
        );
        let runtime_sha256 = sha256(&fs::read(local.join(&manifest.runtime.entry))?)?;
        let authority = self
            .authority()
            .query(&self.root_actor, result.installation.id)
            .await?;
        ensure!(
            authority.authorizations.is_empty(),
            "upload alone minted contribution authority"
        );
        Ok(Installed {
            result,
            manifest,
            runtime_sha256,
        })
    }

    pub async fn assert_not_runtime_active(&self, id: Uuid) -> Result<()> {
        let installation = self
            .store
            .get_installation(id)
            .await?
            .context("installation missing")?;
        let artifact = self
            .store
            .get_artifact_instance(NODE, id)
            .await?
            .context("artifact missing")?;
        ensure!(
            installation.desired_state == PluginDesiredState::Disabled
                && artifact.runtime_status == PluginRuntimeStatus::Inactive,
            "preparation must not be mistaken for worker activation"
        );
        Ok(())
    }
}

/// Negative paths must fail for the intended authority reason, not a SQL/setup error.
pub fn assert_permission_denied(result: Result<()>, expected: &'static str) -> Result<()> {
    let error = result.err().context("operation unexpectedly succeeded")?;
    ensure!(
        matches!(error.downcast_ref::<control_plane_contracts::ControlPlaneContractError>(),
        Some(control_plane_contracts::ControlPlaneContractError::PermissionDenied(code)) if *code == expected),
        "operation did not return the expected authority denial ({expected})"
    );
    Ok(())
}
