use super::*;
use control_plane::{
    auth::CompleteExternalLoginCommand,
    ports::{CreateExternalRegisteredMemberInput, ExternalRegistrationRepository},
};

#[async_trait]
impl ExternalRegistrationRepository for InstanceAwareAuthRepository {
    async fn create_external_registered_member(
        &self,
        _input: &CreateExternalRegisteredMemberInput,
    ) -> Result<UserRecord> {
        anyhow::bail!("registration must not be reached by bound or rejected identities")
    }
}

fn identity(connection_id: Uuid) -> VerifiedExternalIdentity {
    VerifiedExternalIdentity::try_from(ExternalIdentityClaim {
        connection_id,
        subject_type: "github".into(),
        subject_value: "42".into(),
        issuer: Some("https://github.com".into()),
        realm: None,
        profile: serde_json::json!({}),
        verified: true,
        metadata: serde_json::json!({}),
    })
    .unwrap()
}

#[tokio::test]
async fn bound_external_identity_needs_neither_email_registration_nor_eligibility() {
    let entry = authenticator(PARTNER_EXTERNAL_AUTHENTICATOR_ID, "GitHub", "github");
    let user = test_user();
    let repository = InstanceAwareAuthRepository::new(vec![entry.clone()], user.clone())
        .with_bound_external_identity(entry.connection_id, "42");
    let kernel = AuthKernel::new(
        repository.clone(),
        SessionIssuer::new(MemorySessionStore::default(), 7),
    );
    let result = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: identity(entry.connection_id),
            registration_eligible: false,
        })
        .await
        .unwrap();
    assert_eq!(result.actor.user_id, user.id);
    assert_eq!(result.session.user_id, user.id);
    assert!(repository.password_login_calls().is_empty());
}

#[tokio::test]
async fn external_completion_rejects_unknown_closed_registration_and_ineligible_identity() {
    let mut entry = authenticator(PARTNER_EXTERNAL_AUTHENTICATOR_ID, "GitHub", "github");
    for (enabled, eligible, expected) in [
        (false, true, "self_registration_disabled"),
        (true, false, "external_registration_ineligible"),
        (true, true, "external_registration_email_required"),
    ] {
        entry.options =
            serde_json::json!({"extension_config":{"self_registration_enabled":enabled}});
        let repository = InstanceAwareAuthRepository::new(vec![entry.clone()], test_user());
        let kernel = AuthKernel::new(
            repository,
            SessionIssuer::new(MemorySessionStore::default(), 7),
        );
        let error = kernel
            .complete_external_login(CompleteExternalLoginCommand {
                login_entry_id: entry.id,
                identity: identity(entry.connection_id),
                registration_eligible: eligible,
            })
            .await
            .err()
            .expect("unknown identity must be rejected");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[tokio::test]
async fn external_completion_rejects_connection_mismatch_and_disabled_entry_or_user() {
    let mut entry = authenticator(PARTNER_EXTERNAL_AUTHENTICATOR_ID, "GitHub", "github");
    let kernel = AuthKernel::new(
        InstanceAwareAuthRepository::new(vec![entry.clone()], test_user()),
        SessionIssuer::new(MemorySessionStore::default(), 7),
    );
    let error = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: identity(OTHER_EXTERNAL_AUTHENTICATOR_ID),
            registration_eligible: true,
        })
        .await
        .err()
        .unwrap();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotAuthenticated)
    ));
    entry.enabled = false;
    let user = test_user();
    let kernel = AuthKernel::new(
        InstanceAwareAuthRepository::new(vec![entry.clone()], user.clone())
            .with_bound_external_identity(entry.connection_id, "42"),
        SessionIssuer::new(MemorySessionStore::default(), 7),
    );
    let error = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: identity(entry.connection_id),
            registration_eligible: true,
        })
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("login_entry_disabled"));
    entry.enabled = true;
    let mut user = user;
    user.status = UserStatus::Disabled;
    let kernel = AuthKernel::new(
        InstanceAwareAuthRepository::new(vec![entry.clone()], user)
            .with_bound_external_identity(entry.connection_id, "42"),
        SessionIssuer::new(MemorySessionStore::default(), 7),
    );
    let error = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: identity(entry.connection_id),
            registration_eligible: true,
        })
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("user_disabled"));
}

#[async_trait]
impl control_plane::ports::ExternalIdentityLifecycleRepository for InstanceAwareAuthRepository {
    async fn find_external_identity_binding(
        &self,
        identity: &VerifiedExternalIdentity,
    ) -> Result<Option<control_plane::ports::ExternalIdentityBinding>> {
        let user = self
            .find_user_for_verified_external_identity(identity)
            .await?;
        Ok(
            user.map(|user| control_plane::ports::ExternalIdentityBinding {
                identity_id: Uuid::now_v7(),
                user_id: user.id,
                connection_id: identity.connection_id(),
                authority_namespace: identity.authority_namespace().into(),
                subject_type: identity.subject_type().into(),
                subject_value: identity.subject_value().into(),
                issuer: identity.issuer().map(str::to_owned),
                realm: identity.realm().map(str::to_owned),
                state: "bound".into(),
                auth_type: "github".into(),
                profile: identity.profile().clone(),
                bound_at: time::OffsetDateTime::now_utc(),
                unlinked_at: None,
                entry_enabled: true,
            }),
        )
    }
    async fn authentication_methods_snapshot(
        &self,
        _: Uuid,
    ) -> Result<control_plane::ports::AuthenticationMethodsSnapshot> {
        anyhow::bail!("unused fixture operation")
    }
    async fn unlink_external_identity_atomic(
        &self,
        _: Uuid,
        _: Uuid,
        _: &[String],
        _: control_plane::ports::PasswordAvailability<'_>,
        _: &AuditLogRecord,
    ) -> Result<()> {
        anyhow::bail!("unused fixture operation")
    }
    async fn initialize_password_atomic(
        &self,
        _: Uuid,
        _: Uuid,
        _: &str,
        _: &[String],
        _: control_plane::ports::PasswordAvailability<'_>,
        _: &AuditLogRecord,
    ) -> Result<()> {
        anyhow::bail!("unused fixture operation")
    }
}
