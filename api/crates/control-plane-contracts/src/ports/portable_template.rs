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

/// Journal CAS is independent from editor CAS. Owners must guard the mutation against
/// intent.expected_fingerprint and record its receipt in their own write transaction.
/// Holding the template install lock alone does NOT exclude ordinary editor writes.
#[async_trait::async_trait]
pub trait PortableTemplateBaselineRepository: Send + Sync {
    async fn load_template_baselines(
        &self,
        scope: &crate::portable_template::TemplateBaselineScope,
    ) -> anyhow::Result<Vec<crate::portable_template::TemplateResourceBaseline>>;

    /// expected_generation=None inserts a previously unseen key; Some requires exact CAS.
    /// Existing pending intent is never overwritten (including when its content matches).
    async fn prepare_template_write(
        &self,
        scope: &crate::portable_template::TemplateBaselineScope,
        key: &crate::portable_template::TemplateResourceKey,
        expected_generation: Option<i64>,
        intent: &crate::portable_template::TemplateWriteIntent,
    ) -> anyhow::Result<bool>;

    /// Advances only a write with an atomic owner receipt. Never reads/adopts live content.
    async fn finalize_template_write(
        &self,
        scope: &crate::portable_template::TemplateBaselineScope,
        key: &crate::portable_template::TemplateResourceKey,
        operation_id: Uuid,
    ) -> anyhow::Result<bool>;

    /// Owner proved that its transaction did not commit; do not use after an unknown outcome.
    async fn abandon_template_write(
        &self,
        scope: &crate::portable_template::TemplateBaselineScope,
        key: &crate::portable_template::TemplateResourceKey,
        operation_id: Uuid,
    ) -> anyhow::Result<bool>;
}

/// A repository bound to a single atomic native-owner batch. Implementations must
/// exclude concurrent editor writes to the projected business records until completion.
pub struct PortableTemplateTransaction<R> {
    pub repository: R,
    pub guard: Box<dyn PortableTemplateTransactionGuard>,
}
#[async_trait::async_trait]
pub trait PortableTemplateTransactionGuard: Send {
    async fn commit(&mut self) -> anyhow::Result<()>;
    async fn rollback(&mut self) -> anyhow::Result<()>;
}
/// A binding-only identity transition. The source key is stable; old target,
/// generation and applied fingerprint must all match before moving the native row.
#[derive(Debug, Clone)]
pub struct TemplateMcpBindingRetarget {
    pub key: crate::portable_template::TemplateResourceKey,
    pub expected_target_id: String,
    pub expected_generation: i64,
    pub intent: crate::portable_template::TemplateWriteIntent,
    pub actor_user_id: Uuid,
}

#[async_trait::async_trait]
pub trait PortableTemplateTransactionRepository: Sized + Send + Sync {
    async fn begin_portable_template_transaction(
        &self,
    ) -> anyhow::Result<PortableTemplateTransaction<Self>>;

    /// Requires the existing native-owner transaction/locks. Atomically preserve the
    /// binding row identity while moving its tool reference and preparing the baseline
    /// at the new target. False performs no writes; caller rolls back on any error.
    async fn retarget_template_mcp_binding(
        &self,
        scope: &crate::portable_template::TemplateBaselineScope,
        command: &TemplateMcpBindingRetarget,
    ) -> anyhow::Result<bool>;

    /// Called only on the transaction-bound repository after the owner writes and
    /// canonical post-image read. Receipt and resource changes commit atomically.
    async fn acknowledge_portable_template_write(
        &self,
        scope: &crate::portable_template::TemplateBaselineScope,
        key: &crate::portable_template::TemplateResourceKey,
        intent: &crate::portable_template::TemplateWriteIntent,
        actual_fingerprint: &str,
    ) -> anyhow::Result<bool>;
}
