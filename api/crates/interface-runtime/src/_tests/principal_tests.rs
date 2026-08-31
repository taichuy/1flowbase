use domain::ActorContext;
use uuid::Uuid;

use crate::{
    ApplicationPrincipal, ApplicationPrincipalError, AuthenticatedSessionIdentity,
    InvocationPrincipal, PrincipalProfile, PublicPrincipal, UserCredentialKind, UserPrincipal,
};

fn actor(workspace_id: Uuid) -> ActorContext {
    ActorContext::scoped(Uuid::now_v7(), workspace_id, "member", [])
}

#[test]
fn authenticated_session_is_a_sealed_user_fact_not_a_receipt_summary_field() {
    let actor = actor(Uuid::now_v7());
    let principal = UserPrincipal::with_authenticated_session(
        actor.clone(),
        AuthenticatedSessionIdentity::new("session-verified").unwrap(),
    );
    assert_eq!(
        principal
            .authenticated_session()
            .unwrap()
            .expose_to_trusted_handler(),
        "session-verified"
    );
    assert_eq!(
        principal.credential_kind(),
        UserCredentialKind::CookieSession
    );
    assert_eq!(principal.summary().user_id(), Some(actor.user_id));
    assert_eq!(principal.summary().api_key_id(), None);
}

#[test]
fn public_user_and_application_profiles_expose_only_stable_summaries() {
    let public = PublicPrincipal::new().summary();
    assert_eq!(public.profile(), PrincipalProfile::Public);
    assert_eq!(public.user_id(), None);
    assert_eq!(public.application_id(), None);
    assert_eq!(public.api_key_id(), None);
    assert_eq!(public.workspace_id(), None);

    let workspace_id = Uuid::now_v7();
    let api_key_id = Uuid::now_v7();
    let user = UserPrincipal::new(
        actor(workspace_id),
        UserCredentialKind::UserApiKey { api_key_id },
    );
    let user_summary = user.summary();
    assert_eq!(user_summary.profile(), PrincipalProfile::User);
    assert_eq!(user_summary.api_key_id(), Some(api_key_id));
    assert_eq!(user_summary.workspace_id(), Some(workspace_id));
    assert_eq!(
        user.credential_kind(),
        UserCredentialKind::UserApiKey { api_key_id }
    );

    let application_id = Uuid::now_v7();
    let application_api_key_id = Uuid::now_v7();
    let application = ApplicationPrincipal::new(
        application_id,
        application_api_key_id,
        workspace_id,
        actor(workspace_id),
    )
    .unwrap();
    let application_summary = application.summary();
    assert_eq!(application_summary.profile(), PrincipalProfile::Application);
    assert_eq!(application_summary.application_id(), Some(application_id));
    assert_eq!(
        application_summary.api_key_id(),
        Some(application_api_key_id)
    );
    assert_eq!(application_summary.workspace_id(), Some(workspace_id));
}

#[test]
fn application_profile_rejects_missing_or_mismatched_identity() {
    let workspace_id = Uuid::now_v7();
    assert_eq!(
        ApplicationPrincipal::new(
            Uuid::nil(),
            Uuid::now_v7(),
            workspace_id,
            actor(workspace_id),
        )
        .unwrap_err(),
        ApplicationPrincipalError::MissingIdentity {
            identity: "application"
        }
    );
    assert_eq!(
        ApplicationPrincipal::new(
            Uuid::now_v7(),
            Uuid::nil(),
            workspace_id,
            actor(workspace_id),
        )
        .unwrap_err(),
        ApplicationPrincipalError::MissingIdentity {
            identity: "api_key"
        }
    );
    assert_eq!(
        ApplicationPrincipal::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            workspace_id,
            actor(Uuid::now_v7()),
        )
        .unwrap_err(),
        ApplicationPrincipalError::WorkspaceMismatch
    );
}
