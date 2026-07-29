use crate::{
    _tests::support::MemoryAuthRepository,
    auth::settings::{AuthCenterSettingsService, CreateAuthCenterAuthenticatorCommand},
    ports::AuthRepository,
};
use plugin_framework::{
    AuthProviderContributionManifest, HostExtensionBootstrapPhase, HostExtensionRegistry,
    RegisteredHostExtension,
};
use std::sync::Arc;

fn fixture_authenticator_registry() -> crate::auth::AuthenticatorRegistry {
    let mut host_extensions = HostExtensionRegistry::default();
    host_extensions
        .register(RegisteredHostExtension {
            extension_id: "fixture-auth".to_string(),
            bootstrap_phase: HostExtensionBootstrapPhase::Boot,
            provides_contracts: vec![],
            overrides_contracts: vec![],
            registers_slots: vec![],
            registers_storage: vec![],
            infrastructure_providers: vec![],
            auth_providers: vec![AuthProviderContributionManifest {
                auth_type: "fixture-auth.qr".to_string(),
                display_name: "Fixture QR".to_string(),
                config_schema: vec![serde_json::from_value(serde_json::json!({
                    "key": "issuer",
                    "label": "Issuer",
                    "type": "string"
                }))
                .unwrap()],
                default_public_ui_block: "export default { main } satisfies BlockModule;"
                    .to_string(),
                public_variable_keys: vec!["issuer".to_string()],
                public_route_ids: vec!["fixture-auth.qr.start".to_string()],
            }],
            owned_resources: vec![],
            extends_resources: vec![],
            routes: vec!["fixture-auth.qr.start".to_string()],
            workers: vec![],
            migrations: vec![],
        })
        .unwrap();
    crate::auth::AuthenticatorRegistry::from_host_extensions(&host_extensions).unwrap()
}

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

#[test]
fn ac_004_password_local_default_block_owns_the_authenticator_selector_action() {
    let source = crate::auth::public_ui::PASSWORD_LOCAL_PUBLIC_UI_BLOCK;
    assert!(source.contains("authenticator_selection_available"));
    assert!(source.contains("authenticator_selector_requested"));
    assert!(source.contains("ArrowLeftOutlined"));
}

#[tokio::test]
async fn backend_only_provider_seeds_new_authenticator_with_its_schema_and_default_block() {
    let repository = MemoryAuthRepository::root_user(None);
    let actor = AuthRepository::load_actor_context_for_user(&repository, repository.user().id)
        .await
        .unwrap();
    let service = AuthCenterSettingsService::with_registry(
        repository,
        Arc::new(fixture_authenticator_registry()),
    );

    let authenticator = service
        .create_authenticator(
            &actor,
            CreateAuthCenterAuthenticatorCommand {
                auth_type: "fixture-auth.qr".to_string(),
                title: "Fixture QR".to_string(),
                description: Some("Scan to sign in".to_string()),
                enabled: true,
                sort_order: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(
        authenticator.public_ui_block,
        "export default { main } satisfies BlockModule;"
    );
    assert_eq!(
        authenticator.options["config_form_schema"][0]["key"],
        "issuer"
    );
    assert_eq!(
        authenticator.options["extension_config"],
        serde_json::json!({})
    );
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
