use crate::{
    ApiKeyRepository, AuthRepository, BootstrapRepository, LoginEntrySettingsRepository,
    MemberRepository, RoleConsolePolicyMigrationCutoverMarker,
    RoleConsolePolicyMigrationCutoverState, RoleConsolePolicyMigrationRepository,
    RoleConsolePolicyMigrationSource, RoleConsolePolicyReader, RoleRepository,
    SelfRegistrationRepository, WorkspaceConsoleSettingsOrder, WorkspaceRepository,
};

#[test]
fn auth_repository_contract_keeps_migration_and_role_dto_semantics() {
    auth_traits_remain_canonical::<
        dyn BootstrapRepository,
        dyn AuthRepository,
        dyn LoginEntrySettingsRepository,
        dyn ApiKeyRepository,
        dyn WorkspaceRepository,
        dyn RoleConsolePolicyMigrationRepository,
        dyn RoleConsolePolicyReader,
        dyn MemberRepository,
        dyn SelfRegistrationRepository,
        dyn RoleRepository,
    >();

    let source = RoleConsolePolicyMigrationSource {
        permission_resources: vec!["application".to_string()],
        exact_permission_codes: vec!["settings_route.visible.settings.roles".to_string()],
    };
    let encoded = serde_json::to_value(&source).expect("migration source must serialize");
    let decoded: RoleConsolePolicyMigrationSource =
        serde_json::from_value(encoded).expect("migration source must deserialize");
    assert_eq!(decoded, source);

    let cutover = RoleConsolePolicyMigrationCutoverState {
        marker: RoleConsolePolicyMigrationCutoverMarker::Fenced,
        run_id: None,
        catalog_fingerprint: Some("sha256:catalog".to_string()),
        mapping_fingerprint: Some("sha256:mapping".to_string()),
    };
    assert_eq!(cutover.clone(), cutover);

    let settings_order = WorkspaceConsoleSettingsOrder {
        revision: 7,
        group_ids: vec!["system.roles".to_string()],
    };
    assert_eq!(settings_order.clone(), settings_order);
}

fn auth_traits_remain_canonical<
    Bootstrap,
    Auth,
    AuthenticatorSettings,
    ApiKey,
    Workspace,
    Migration,
    ConsolePolicy,
    Member,
    SelfRegistration,
    Role,
>()
where
    Bootstrap: BootstrapRepository + ?Sized,
    Auth: AuthRepository + ?Sized,
    AuthenticatorSettings: LoginEntrySettingsRepository + ?Sized,
    ApiKey: ApiKeyRepository + ?Sized,
    Workspace: WorkspaceRepository + ?Sized,
    Migration: RoleConsolePolicyMigrationRepository + ?Sized,
    ConsolePolicy: RoleConsolePolicyReader + ?Sized,
    Member: MemberRepository + ?Sized,
    SelfRegistration: SelfRegistrationRepository + ?Sized,
    Role: RoleRepository + ?Sized,
{
}
