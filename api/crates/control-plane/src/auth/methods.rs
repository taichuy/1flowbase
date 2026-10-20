use super::*;
use crate::ports::{ExternalIdentityLifecycleRepository, PasswordAvailability};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ExternalAuthenticationMethod {
    pub identity_id: Uuid,
    pub connection_id: Uuid,
    pub authority_namespace: String,
    pub subject_type: String,
    pub subject_value: String,
    pub issuer: Option<String>,
    pub realm: Option<String>,
    pub auth_type: String,
    pub profile: serde_json::Value,
    pub bound_at: OffsetDateTime,
    pub unlinked_at: Option<OffsetDateTime>,
    pub state: String,
    pub provider_available: bool,
    pub can_unlink: bool,
    pub unlink_blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthenticationMethodsView {
    pub password_configured: bool,
    pub password_usable: bool,
    pub bindings: Vec<ExternalAuthenticationMethod>,
}

pub struct AuthenticationMethodsService<R> {
    repository: R,
}

/// Match the verifier's accepted Argon2 format without performing a password KDF.
/// Sentinel values, incomplete PHC records and invalid Argon2 parameters do not
/// provide an alternative login method.
fn usable_password_hash(value: &str) -> bool {
    let Ok(hash) = PasswordHash::new(value) else {
        return false;
    };
    matches!(hash.algorithm.as_str(), "argon2id" | "argon2i" | "argon2d")
        && hash
            .salt
            .as_ref()
            .is_some_and(|salt| salt.as_str().len() >= 11)
        && hash.hash.as_ref().is_some_and(|output| output.len() >= 4)
        && argon2::Params::try_from(&hash).is_ok()
        && hash
            .version
            .is_none_or(|version| version == 16 || version == 19)
}

impl<R: ExternalIdentityLifecycleRepository> AuthenticationMethodsService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn read(
        &self,
        user_id: Uuid,
        available_authorities: &[String],
    ) -> Result<AuthenticationMethodsView> {
        let snapshot = self
            .repository
            .authentication_methods_snapshot(user_id)
            .await?;
        let password_configured = usable_password_hash(&snapshot.password_hash);
        let password_usable = password_configured
            && snapshot.local_account_identity
            && snapshot.password_entry_enabled;
        let usable_external = snapshot
            .bindings
            .iter()
            .filter(|binding| {
                binding.state == "bound"
                    && binding.entry_enabled
                    && available_authorities.contains(&binding.authority_namespace)
            })
            .count();
        let bindings = snapshot
            .bindings
            .into_iter()
            .map(|binding| {
                let provider_available = binding.entry_enabled
                    && available_authorities.contains(&binding.authority_namespace);
                let can_unlink = binding.state == "bound"
                    && (password_usable || usable_external > usize::from(provider_available));
                let unlink_blocked_reason = if binding.state != "bound" {
                    Some("external_identity_unlinked".into())
                } else if !can_unlink {
                    Some("last_usable_authentication_method".into())
                } else {
                    None
                };
                ExternalAuthenticationMethod {
                    identity_id: binding.identity_id,
                    connection_id: binding.connection_id,
                    authority_namespace: binding.authority_namespace,
                    subject_type: binding.subject_type,
                    subject_value: binding.subject_value,
                    issuer: binding.issuer,
                    realm: binding.realm,
                    auth_type: binding.auth_type,
                    profile: binding.profile,
                    bound_at: binding.bound_at,
                    unlinked_at: binding.unlinked_at,
                    state: binding.state,
                    provider_available,
                    can_unlink,
                    unlink_blocked_reason,
                }
            })
            .collect();
        Ok(AuthenticationMethodsView {
            password_configured,
            password_usable,
            bindings,
        })
    }

    pub async fn unlink(
        &self,
        actor: &ActorContext,
        identity_id: Uuid,
        available_authorities: &[String],
    ) -> Result<()> {
        let snapshot = self
            .repository
            .authentication_methods_snapshot(actor.user_id)
            .await?;
        let audit = audit_log(
            Some(actor.current_workspace_id),
            Some(actor.user_id),
            "user_auth_identity",
            Some(identity_id),
            "user.external_identity_unlinked",
            serde_json::json!({}),
        );
        self.repository
            .unlink_external_identity_atomic(
                actor.user_id,
                identity_id,
                available_authorities,
                PasswordAvailability {
                    expected_hash: &snapshot.password_hash,
                    hash_usable: usable_password_hash(&snapshot.password_hash),
                },
                &audit,
            )
            .await
    }

    /// Host has already consumed a short-lived session-bound step-up proof.
    /// The transaction rechecks the proof identity, authority and credential state.
    pub async fn initialize_password(
        &self,
        actor: &ActorContext,
        verified_identity_id: Uuid,
        new_password_hash: &str,
        available_authorities: &[String],
    ) -> Result<()> {
        if !usable_password_hash(new_password_hash) {
            return Err(ControlPlaneError::InvalidInput("new_password_hash").into());
        }
        let snapshot = self
            .repository
            .authentication_methods_snapshot(actor.user_id)
            .await?;
        let audit = audit_log(
            Some(actor.current_workspace_id),
            Some(actor.user_id),
            "user",
            Some(actor.user_id),
            "user.password_initialized",
            serde_json::json!({ "verified_identity_id": verified_identity_id }),
        );
        self.repository
            .initialize_password_atomic(
                actor.user_id,
                verified_identity_id,
                new_password_hash,
                available_authorities,
                PasswordAvailability {
                    expected_hash: &snapshot.password_hash,
                    hash_usable: usable_password_hash(&snapshot.password_hash),
                },
                &audit,
            )
            .await
    }
}
