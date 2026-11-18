use super::*;

fn plugin_inventory(
    with_settings: bool,
) -> (
    ConsoleOperationCompiledInventory,
    Vec<access_control::SettingsFeatureInventoryEntry>,
) {
    let owner = ConsoleOperationOwner {
        kind: SettingsFeatureOwnerKind::ManagedService,
        owner_id: "acme.calendar".into(),
        version: "1.0.0".into(),
    };
    let route = ConsoleRouteBinding {
        method: "GET".into(),
        path: "/api/console/acme/calendar".into(),
    };
    let feature = SettingsFeatureRegistration {
        feature_id: "acme.calendar.settings".into(),
        owner: owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        console_surface: SettingsFeatureConsoleSurface {
            route_id: "acme.calendar.settings".into(),
            surface_key: "acme.calendar.settings".into(),
            path: "/settings/acme/calendar".into(),
            label_key: "Calendar".into(),
            description_key: "Calendar operations".into(),
            order: 1,
        },
        api_routes: vec![SettingsApiRoute {
            method: route.method.clone(),
            path: route.path.clone(),
        }],
    };
    let settings =
        SettingsFeatureRegistry::compile(if with_settings { vec![feature] } else { vec![] })
            .unwrap();
    let texts = [
        "Calendar",
        "Calendar operations",
        "Full access",
        "Grant every operation in this group",
        "Custom access",
        "Choose operations and row scopes individually",
        "Disabled",
        "Do not grant this operation",
        "Own records",
        "Allow records created by the current user",
        "Current workspace",
        "Allow records in the current workspace",
    ]
    .into_iter()
    .map(|reference| ConsoleLocaleText {
        reference: reference.into(),
        en_us: reference.into(),
        zh_hans: "测试".into(),
    })
    .collect();
    let registry = ConsoleOperationRegistry::compile_with_locale_catalog(
        &settings,
        [ConsoleOperationRegistration {
            operation_id: "acme.calendar.list".into(),
            authorization_profile_id: None,
            owner: owner.clone(),
            lifecycle: SettingsFeatureLifecycle::Active,
            policy_group: ConsolePolicyGroup::Plugin("acme.calendar".into()),
            order: 1,
            routes: vec![route.clone()],
            authorization: ConsoleAuthorization::Simple,
        }],
        [],
        [ConsoleLocaleCatalogContribution {
            owner,
            lifecycle: SettingsFeatureLifecycle::Active,
            texts,
            policy_groups: vec![],
            plugin_groups: vec![ConsoleOtherPolicyGroupDisplay {
                group_id: "acme.calendar".into(),
                label_ref: "Calendar".into(),
                description_ref: "Calendar operations".into(),
            }],
        }],
    )
    .unwrap()
    .with_interface_metadata([ConsoleInterfaceRegistration {
        interface_id: "acme.calendar.list".into(),
        route,
        summary: "List calendar events".into(),
        description: "List calendar events for the current workspace.".into(),
    }])
    .unwrap();
    (
        registry.inventory().clone(),
        settings.inventory().features.clone(),
    )
}

#[tokio::test]
async fn plugin_settings_projection_uses_one_canonical_catalog_and_authorization_group() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());
    let (inventory, settings) = plugin_inventory(true);
    let catalog = service
        .get_console_policy_catalog(repository.root_user_id(), &inventory, &settings, "en_US")
        .await
        .unwrap();
    assert_eq!(catalog.groups.len(), 1);
    assert_eq!(
        catalog.groups[0].kind,
        domain::ConsolePolicyGroupKind::Plugin
    );
    assert_eq!(catalog.groups[0].group_id, "acme.calendar");
    assert_eq!(
        catalog.groups[0].settings_feature_id.as_deref(),
        Some("acme.calendar.settings")
    );
    assert_eq!(catalog.groups[0].operations.len(), 1);
    let saved = service
        .replace_console_settings_order(
            crate::role::ReplaceConsoleSettingsOrderCommand {
                actor_user_id: repository.root_user_id(),
                expected_revision: 0,
                group_ids: vec!["acme.calendar.settings".into()],
            },
            &inventory,
            &settings,
            "en_US",
        )
        .await
        .unwrap();
    assert_eq!(saved.groups[0].group_id, "acme.calendar");
    assert_eq!(saved.groups.len(), 1);
    assert!(service
        .replace_console_settings_order(
            crate::role::ReplaceConsoleSettingsOrderCommand {
                actor_user_id: repository.root_user_id(),
                expected_revision: saved.settings_order_revision,
                group_ids: vec!["acme.calendar".into()],
            },
            &inventory,
            &settings,
            "en_US"
        )
        .await
        .is_err());
}

#[tokio::test]
async fn plugin_without_settings_page_keeps_its_catalog_group_without_settings_alias() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());
    let (inventory, settings) = plugin_inventory(false);
    let catalog = service
        .get_console_policy_catalog(repository.root_user_id(), &inventory, &settings, "en_US")
        .await
        .unwrap();
    assert_eq!(catalog.groups.len(), 1);
    assert_eq!(catalog.groups[0].group_id, "acme.calendar");
    assert_eq!(catalog.groups[0].settings_feature_id, None);
}

#[tokio::test]
async fn foreign_settings_feature_cannot_become_plugin_settings_projection() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());
    let (inventory, mut settings) = plugin_inventory(true);
    settings[0].owner.owner_id = "other.calendar".into();
    let catalog = service
        .get_console_policy_catalog(repository.root_user_id(), &inventory, &settings, "en_US")
        .await
        .unwrap();
    assert_eq!(catalog.groups[0].settings_feature_id, None);
}

#[tokio::test]
async fn mismatched_settings_owner_version_cannot_become_plugin_settings_projection() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());
    let (inventory, mut settings) = plugin_inventory(true);
    settings[0].owner.version = "2.0.0".into();
    let catalog = service
        .get_console_policy_catalog(repository.root_user_id(), &inventory, &settings, "en_US")
        .await
        .unwrap();
    assert_eq!(catalog.groups[0].settings_feature_id, None);
}
