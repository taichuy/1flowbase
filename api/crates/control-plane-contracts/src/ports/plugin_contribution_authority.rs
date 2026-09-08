use anyhow::Result;
use async_trait::async_trait;
use domain::{AuditLogRecord, ContributionResourceScope, PluginContributionAuthoritySnapshot};
use extension_contracts::extension_bus::ManagedContributionSubject;
use std::{future::Future, pin::Pin};
use uuid::Uuid;

pub struct GrantContributionAuthorizationInput {
    pub expected_installation_updated_at: time::OffsetDateTime,
    pub installation_id: Uuid,
    pub workspace_id: Uuid,
    pub contribution_id: String,
    pub point_id: String,
    pub permission: String,
    pub resource_scope: ContributionResourceScope,
    pub permission_contract_id: String,
    pub permission_contract_version: String,
    pub actor_user_id: Uuid,
    pub audit_log: AuditLogRecord,
}

pub struct RevokeContributionAuthorizationInput {
    pub installation_id: Uuid,
    pub workspace_id: Uuid,
    pub authorization_id: Uuid,
    pub expected_revision: i64,
    pub actor_user_id: Uuid,
    pub audit_log: AuditLogRecord,
}

/// Holds the same serialization lock as grant/revoke until release or drop. The caller must
/// keep this lease through actual admission (or execution when no separate admission exists).
/// Reading snapshot().revision and releasing before asynchronous admission is not sufficient.
/// A lease carries current facts, not an automatic permission decision.
pub trait ContributionAuthorityLease: Send {
    fn snapshot(&self) -> &PluginContributionAuthoritySnapshot;
    fn snapshots(&self) -> &[PluginContributionAuthoritySnapshot];
    fn installation(&self, installation_id: Uuid) -> Option<&domain::PluginInstallationRecord>;
    /// Commit the sealed subscriber effect and its receipt under the current authority lock.
    fn commit_processed_model(
        self: Box<Self>,
        subject: ManagedContributionSubject,
        event_id: Uuid,
        effect: extension_contracts::ManagedEventPayload,
        deadline_unix_ms: i64,
    ) -> Pin<Box<dyn Future<Output = Result<extension_contracts::PluginDataResponse>> + Send>>;
    /// Commit a validated derived fact using the connection already owned by this lease.
    fn commit_derived_lifecycle_fact(
        self: Box<Self>,
        input: super::RecordLifecycleFactInput,
    ) -> Pin<Box<dyn Future<Output = Result<super::LifecycleOutboxRecord>> + Send>>;
    fn release(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;
}

#[async_trait]
pub trait PluginContributionAuthorityRepository: Send + Sync {
    /// Sorted, deduplicated scopes share one transaction; an empty batch is invalid.
    async fn lock_contribution_authority_batch(
        &self,
        scopes: &[(Uuid, Uuid)],
    ) -> Result<Box<dyn ContributionAuthorityLease>>;
    async fn contribution_authority_workspaces(&self, installation_id: Uuid) -> Result<Vec<Uuid>>;
    /// One lock per installation/workspace provides all contribution facts without self-deadlock.
    async fn lock_installation_contribution_authority(
        &self,
        installation_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Box<dyn ContributionAuthorityLease>>;
    async fn grant_contribution_authorization(
        &self,
        input: &GrantContributionAuthorizationInput,
    ) -> Result<PluginContributionAuthoritySnapshot>;
    async fn revoke_contribution_authorization(
        &self,
        input: &RevokeContributionAuthorizationInput,
    ) -> Result<PluginContributionAuthoritySnapshot>;
    async fn query_contribution_authority(
        &self,
        installation_id: Uuid,
        workspace_id: Uuid,
        audit_log: &AuditLogRecord,
    ) -> Result<PluginContributionAuthoritySnapshot>;
    async fn lock_contribution_authority(
        &self,
        subject: &ManagedContributionSubject,
    ) -> Result<Box<dyn ContributionAuthorityLease>>;
}
