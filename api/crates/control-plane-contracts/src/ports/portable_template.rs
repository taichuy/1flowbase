use crate::portable_template::PortableTemplatePackage;
use std::collections::BTreeMap;
use uuid::Uuid;
/// Read projection scoped by the authenticated target/source workspace. It never writes editor state.
#[async_trait::async_trait]
pub trait PortableTemplateReadRepository: Send + Sync {
    async fn portable_template_snapshot(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> anyhow::Result<PortableTemplatePackage>;
}

#[async_trait::async_trait]
pub trait PortableTemplateIdentityRepository: Send + Sync {
    async fn lock_application_template_install(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Box<dyn ApplicationTemplateInstallGuard>>;

    async fn load_application_template_releases(
        &self,
        workspace_id: Uuid,
        template_id: &str,
    ) -> anyhow::Result<Vec<ApplicationTemplateReleaseRecord>>;

    /// Reserves the immutable digest before writes; only complete installs become successful.
    async fn record_application_template_release(
        &self,
        workspace_id: Uuid,
        template_id: &str,
        release_version: u64,
        checksum: &str,
        successful: bool,
    ) -> anyhow::Result<()>;

    async fn load_portable_template_identity_map(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<BTreeMap<String, String>>;

    async fn record_portable_template_identity(
        &self,
        workspace_id: Uuid,
        kind: &str,
        source_id: &str,
        target_id: &str,
    ) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApplicationTemplateReleaseRecord {
    pub release_version: u64,
    pub checksum: String,
    pub successful: bool,
}

/// Held until the complete cross-owner installation finishes (including failure).
pub trait ApplicationTemplateInstallGuard: Send {}
