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
    fn release(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;
}

#[async_trait]
pub trait PluginContributionAuthorityRepository: Send + Sync {
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
