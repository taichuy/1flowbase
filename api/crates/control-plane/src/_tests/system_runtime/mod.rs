use crate::_tests::support::MemoryAuthRepository;
use crate::system_runtime::SystemRuntimeService;

fn system_runtime_policy() -> domain::RoleConsolePolicy {
    let group = domain::ConsolePolicyGroup::settings_feature("system.system-runtime")
        .expect("system-runtime feature id must be valid");
    domain::RoleConsolePolicy::new(
        uuid::Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::custom(
            group,
            vec![domain::ConsoleOperationPolicy::simple(
                domain::ConsoleOperationId::try_from("system.runtime_profile.view")
                    .expect("runtime profile operation id must be valid"),
                true,
            )],
        )],
    )
}

#[tokio::test]
async fn ac_011_system_runtime_policy_only_allows_runtime_profile_without_legacy_grant() {
    let store = MemoryAuthRepository::scoped_user(&[]);
    store
        .set_console_policies(vec![system_runtime_policy()])
        .await;
    let service = SystemRuntimeService::new(store.clone());

    assert!(service.authorize_view(store.user().id).await.is_ok());
}

#[tokio::test]
async fn ac_011_system_runtime_legacy_grants_do_not_authorize_runtime_profile() {
    let store = MemoryAuthRepository::scoped_user(&[
        "system_runtime.view.all",
        access_control::SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_PERMISSION,
    ]);
    let service = SystemRuntimeService::new(store.clone());

    assert!(service.authorize_view(store.user().id).await.is_err());
}

#[tokio::test]
async fn authorize_view_requires_system_runtime_permission_for_non_root() {
    let store = MemoryAuthRepository::scoped_user(&["plugin_config.view.all"]);
    let service = SystemRuntimeService::new(store.clone());

    let error = service.authorize_view(store.user().id).await.unwrap_err();
    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn authorize_view_returns_user_locale_for_root() {
    let store = MemoryAuthRepository::root_user(Some("en_US"));
    let service = SystemRuntimeService::new(store.clone());

    let access = service.authorize_view(store.user().id).await.unwrap();
    assert_eq!(access.preferred_locale.as_deref(), Some("en_US"));
}
