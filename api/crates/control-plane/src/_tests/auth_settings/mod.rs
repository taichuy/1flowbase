use crate::{
    _tests::support::MemoryAuthRepository,
    auth::settings::{AuthCenterSettingsService, CreateAuthCenterAuthenticatorCommand},
    ports::AuthRepository,
};

fn auth_center_policy(
    operations: Vec<domain::ConsoleOperationPolicy>,
) -> domain::RoleConsolePolicy {
    let group = domain::ConsolePolicyGroup::settings_feature("system.auth-center")
        .expect("auth-center feature id must be valid");
    domain::RoleConsolePolicy::new(
        uuid::Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::custom(group, operations)],
    )
}

fn simple_operation(operation_id: &str) -> domain::ConsoleOperationPolicy {
    domain::ConsoleOperationPolicy::simple(
        domain::ConsoleOperationId::try_from(operation_id)
            .expect("auth-center operation id must be valid"),
        true,
    )
}

#[tokio::test]
async fn ac_011_auth_center_policy_only_allows_overview_and_create_without_legacy_grant() {
    let repository = MemoryAuthRepository::scoped_user(&[]);
    repository
        .set_console_policies(vec![auth_center_policy(vec![
            simple_operation("auth_center.overview.view"),
            simple_operation("auth_center.authenticators.create"),
        ])])
        .await;
    let actor = AuthRepository::load_actor_context_for_user(&repository, repository.user().id)
        .await
        .unwrap();
    let service = AuthCenterSettingsService::new(repository.clone());

    service.overview(&actor).await.unwrap();
    service
        .create_authenticator(
            &actor,
            CreateAuthCenterAuthenticatorCommand {
                auth_type: "password-local".to_string(),
                title: "Policy-only password".to_string(),
                description: None,
                enabled: true,
                sort_order: None,
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn ac_011_auth_center_legacy_feature_grant_does_not_authorize_overview_or_create() {
    let repository = MemoryAuthRepository::scoped_user(&[
        access_control::SYSTEM_AUTH_CENTER_SETTINGS_FEATURE_PERMISSION,
    ]);
    let actor = AuthRepository::load_actor_context_for_user(&repository, repository.user().id)
        .await
        .unwrap();
    let service = AuthCenterSettingsService::new(repository);

    assert!(service.overview(&actor).await.is_err());
    assert!(service
        .create_authenticator(
            &actor,
            CreateAuthCenterAuthenticatorCommand {
                auth_type: "password-local".to_string(),
                title: "Legacy-only password".to_string(),
                description: None,
                enabled: true,
                sort_order: None,
            },
        )
        .await
        .is_err());
}
