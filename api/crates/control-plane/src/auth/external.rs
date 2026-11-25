use super::*;
use crate::ports::{CreateExternalRegisteredMemberInput, ExternalRegistrationRepository};

pub struct CompleteExternalLoginCommand {
    pub invitation_token: Option<String>,
    pub login_entry_id: Uuid,
    pub identity: domain::VerifiedExternalIdentity,
    pub registration_eligible: bool,
}

impl<
        R: AuthRepository
            + ExternalRegistrationRepository
            + crate::ports::ExternalIdentityLifecycleRepository,
        S: SessionStore,
    > AuthKernel<R, S>
{
    pub async fn complete_external_login(
        &self,
        command: CompleteExternalLoginCommand,
    ) -> Result<LoginResult> {
        let entry = self
            .repository
            .find_login_entry(command.login_entry_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("login_entry"))?;
        if !entry.enabled {
            return Err(ControlPlaneError::PermissionDenied("login_entry_disabled").into());
        }
        let connection = self
            .repository
            .find_authentication_connection(entry.connection_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("authentication_connection"))?;
        if entry.auth_type == "password-local"
            || connection.auth_type != entry.auth_type
            || command.identity.connection_id() != entry.connection_id
        {
            return Err(ControlPlaneError::NotAuthenticated.into());
        }
        let invitation = validate_invitation_token(command.invitation_token.as_deref())?;
        let mut registered_scope = None;
        let user = match self
            .repository
            .find_external_identity_binding(&command.identity)
            .await?
        {
            Some(binding) if binding.state == "unlinked" => {
                return Err(
                    ControlPlaneError::PermissionDenied("external_identity_unlinked").into(),
                );
            }
            Some(binding) => self
                .repository
                .find_user_by_id(binding.user_id)
                .await?
                .ok_or(ControlPlaneError::NotAuthenticated)?,
            None => {
                let profile = command.identity.profile();
                let mut registered = None;
                if let Some(policy) = &self.workspace_policy {
                    // Provider eligibility and verified email are independent of the login entry toggle.
                    if command.registration_eligible {
                        let email = verified_registration_email(&command.identity)?;
                        if let Some(decision) = policy
                            .resolve_registration(
                                RegistrationHints {
                                    account: external_registration_account(profile),
                                    email: email.clone(),
                                    name: external_registration_text(profile, "name"),
                                    nickname: external_registration_text(profile, "nickname"),
                                },
                                invitation,
                            )
                            .await?
                        {
                            let account = external_registration_account(profile)
                                .unwrap_or_else(|| format!("external-{}", Uuid::now_v7()));
                            let name = external_registration_text(profile, "name")
                                .unwrap_or_else(|| account.clone());
                            let nickname = external_registration_text(profile, "nickname")
                                .unwrap_or_else(|| name.clone());
                            let (id, scope) = policy.register(decision, control_plane_contracts::ports::NewGovernanceUser {
                                account, email, name, nickname, avatar_url: external_registration_avatar(profile),
                                credential: control_plane_contracts::ports::GovernanceUserCredential::External(command.identity.clone()),
                            }).await?;
                            registered_scope = Some(scope);
                            registered = Some(
                                self.repository
                                    .find_user_by_id(id)
                                    .await?
                                    .ok_or(ControlPlaneError::NotAuthenticated)?,
                            );
                        }
                    }
                }
                if let Some(user) = registered {
                    user
                } else {
                    if invitation.is_some() {
                        return Err(
                            ControlPlaneError::PermissionDenied("invitation_unsupported").into(),
                        );
                    }
                    let email = external_registration_email(
                        &entry,
                        &command.identity,
                        command.registration_eligible,
                    )?;
                    let audit = audit_log(
                        None,
                        None,
                        "user",
                        None,
                        "member.external_registered",
                        serde_json::json!({
                            "connection_id": entry.connection_id,
                            "authority_namespace": command.identity.authority_namespace(),
                            "subject_type": command.identity.subject_type(),
                            "subject_value": command.identity.subject_value(),
                            "issuer": command.identity.issuer(), "realm": command.identity.realm(),
                        }),
                    );
                    self.repository
                        .create_external_registered_member(&CreateExternalRegisteredMemberInput {
                            account: external_registration_account(command.identity.profile()),
                            name: external_registration_text(command.identity.profile(), "name"),
                            nickname: external_registration_text(
                                command.identity.profile(),
                                "nickname",
                            ),
                            avatar_url: external_registration_avatar(command.identity.profile()),
                            identity: command.identity,
                            email,
                            audit,
                        })
                        .await?
                }
            }
        };
        // An entry may have been disabled while provider verification or registration ran.
        let latest = self
            .repository
            .find_login_entry(command.login_entry_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("login_entry"))?;
        if !latest.enabled {
            return Err(ControlPlaneError::PermissionDenied("login_entry_disabled").into());
        }
        if latest.connection_id != entry.connection_id || latest.auth_type != entry.auth_type {
            return Err(ControlPlaneError::NotAuthenticated.into());
        }
        let user = self
            .repository
            .find_user_by_id(user.id)
            .await?
            .ok_or(ControlPlaneError::NotAuthenticated)?;
        if registered_scope.is_some() {
            self.finish_login_in_scope(user, registered_scope).await
        } else {
            self.finish_login_with_invitation(user, invitation).await
        }
    }
}

fn external_registration_email(
    entry: &domain::LoginEntryRecord,
    identity: &domain::VerifiedExternalIdentity,
    eligible: bool,
) -> Result<String> {
    if entry
        .options
        .pointer("/extension_config/self_registration_enabled")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err(ControlPlaneError::PermissionDenied("self_registration_disabled").into());
    }
    if !eligible {
        return Err(ControlPlaneError::PermissionDenied("external_registration_ineligible").into());
    }
    verified_registration_email(identity)
}

fn verified_registration_email(identity: &domain::VerifiedExternalIdentity) -> Result<String> {
    let email = identity
        .profile()
        .get("email")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if identity
        .profile()
        .get("email_verified")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
        || !valid_external_email(&email)
    {
        return Err(ControlPlaneError::InvalidInput("external_registration_email_required").into());
    }
    Ok(email)
}

// Provider-specific keys (GitHub login/name, etc.) are normalized by the provider.
// These optional hints only initialize a new user, never update a bound account.
fn external_registration_account(profile: &serde_json::Value) -> Option<String> {
    let account = profile.get("account")?.as_str()?.trim();
    (!account.is_empty()
        && account.len() <= 64
        && account
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte)))
    .then(|| account.to_owned())
}

fn external_registration_text(profile: &serde_json::Value, field: &str) -> Option<String> {
    let value = profile.get(field)?.as_str()?.trim();
    (!value.is_empty() && value.chars().count() <= 128 && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

fn external_registration_avatar(profile: &serde_json::Value) -> Option<String> {
    let value = profile.get("avatar_url")?.as_str()?.trim();
    if value.len() > 2048 || value.chars().any(char::is_control) {
        return None;
    }
    let url = url::Url::parse(value).ok()?;
    (url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| value.to_owned())
}

fn valid_external_email(email: &str) -> bool {
    let Some((local, host)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && local
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&byte))
        && host.contains('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
        && email.len() <= 254
}

#[cfg(test)]
#[path = "../_tests/external_registration_policy.rs"]
mod tests;
