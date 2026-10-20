//! Atomic identity lifecycle operations. A retained unlinked identity still owns its subject.
use async_trait::async_trait;
use domain::{AuditLogRecord, VerifiedExternalIdentity};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct ExternalIdentityBinding {
    pub identity_id: Uuid,
    pub user_id: Uuid,
    pub connection_id: Uuid,
    pub authority_namespace: String,
    pub subject_type: String,
    pub subject_value: String,
    pub issuer: Option<String>,
    pub realm: Option<String>,
    pub auth_type: String,
    pub profile: serde_json::Value,
    pub bound_at: time::OffsetDateTime,
    pub unlinked_at: Option<time::OffsetDateTime>,
    pub state: String,
    pub entry_enabled: bool,
}

/// Internal credential snapshot, never an HTTP response.
#[derive(Debug, Clone)]
pub struct AuthenticationMethodsSnapshot {
    pub password_hash: String,
    pub local_account_identity: bool,
    pub password_entry_enabled: bool,
    pub bindings: Vec<ExternalIdentityBinding>,
}

/// The application validates the PHC hash. The adapter compares the exact snapshot
/// after taking the user lock, so a concurrent credential edit cannot stale that result.
pub struct PasswordAvailability<'a> {
    pub expected_hash: &'a str,
    pub hash_usable: bool,
}

#[async_trait]
pub trait ExternalIdentityLifecycleRepository: Send + Sync {
    async fn find_external_identity_binding(
        &self,
        identity: &VerifiedExternalIdentity,
    ) -> anyhow::Result<Option<ExternalIdentityBinding>>;
    async fn authentication_methods_snapshot(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<AuthenticationMethodsSnapshot>;
    async fn unlink_external_identity_atomic(
        &self,
        user_id: Uuid,
        identity_id: Uuid,
        available_authorities: &[String],
        password: PasswordAvailability<'_>,
        audit: &AuditLogRecord,
    ) -> anyhow::Result<()>;
    async fn initialize_password_atomic(
        &self,
        user_id: Uuid,
        verified_identity_id: Uuid,
        new_password_hash: &str,
        available_authorities: &[String],
        password: PasswordAvailability<'_>,
        audit: &AuditLogRecord,
    ) -> anyhow::Result<()>;
}
