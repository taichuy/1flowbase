use super::*;
use crate::ports::{CreateExternalRegisteredMemberInput, ExternalRegistrationRepository};

pub struct CompleteExternalLoginCommand {
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
                        nickname: external_registration_nickname(command.identity.profile()),
                        avatar_url: external_registration_avatar(command.identity.profile()),
                        identity: command.identity,
                        email,
                        audit,
                    })
                    .await?
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
        self.finish_login(user).await
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

fn external_registration_nickname(profile: &serde_json::Value) -> Option<String> {
    let nickname = profile.get("nickname")?.as_str()?.trim();
    (!nickname.is_empty()
        && nickname.chars().count() <= 128
        && !nickname.chars().any(char::is_control))
    .then(|| nickname.to_owned())
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
