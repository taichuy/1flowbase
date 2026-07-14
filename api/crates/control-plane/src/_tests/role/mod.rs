use std::collections::BTreeSet;

use access_control::{
    ConsoleAuthorization, ConsoleLocaleCatalogContribution, ConsoleLocaleText,
    ConsoleOperationCompiledInventory, ConsoleOperationInventoryEntry, ConsoleOperationOwner,
    ConsoleOperationRegistration, ConsoleOperationRegistry, ConsoleOtherPolicyGroupDisplay,
    ConsolePolicyGroup, ConsoleRouteBinding, ResourceAccessAction, ResourceAccessRegistration,
    ResourceAccessScopeKind, SettingsApiRoute, SettingsFeatureConsoleSurface,
    SettingsFeatureLifecycle, SettingsFeatureOwnerKind, SettingsFeatureRegistration,
    SettingsFeatureRegistry,
};

use crate::_tests::support::MemoryRoleRepository;
use crate::ports::{
    ReplaceRoleConsolePolicyInput, RoleDataModelPolicyInput, RoleDataPolicyDefaultsInput,
    RoleRepository,
};
use crate::role::{
    ConsolePolicyGroupInput, ConsolePolicyOperationInput, CreateRoleCommand, DeleteRoleCommand,
    ReplaceRoleConsolePolicyCommand, ReplaceRoleDataPolicyCommand, ReplaceRolePermissionsCommand,
    RoleService, UpdateRoleCommand,
};
use uuid::Uuid;

fn console_policy_inventory() -> ConsoleOperationCompiledInventory {
    let owner = ConsoleOperationOwner {
        kind: SettingsFeatureOwnerKind::Core,
        owner_id: "boot-core".to_string(),
        version: "test".to_string(),
    };
    let applications = ConsolePolicyGroup::SettingsFeature("system.applications".to_string());
    let files = ConsolePolicyGroup::SettingsFeature("system.files".to_string());
    let other_files = ConsolePolicyGroup::Other("other.files".to_string());

    let legacy_inventory = ConsoleOperationCompiledInventory {
        schema_version: "1flowbase.console-operation-inventory/v1",
        operations: vec![
            ConsoleOperationInventoryEntry {
                operation_id: "applications.create".to_string(),
                owner: owner.clone(),
                lifecycle: SettingsFeatureLifecycle::Active,
                policy_group: applications.clone(),
                label_ref: "console.operations.applications.create.label".to_string(),
                description_ref: Some(
                    "console.operations.applications.create.description".to_string(),
                ),
                order: 100,
                routes: vec![],
                authorization: ConsoleAuthorization::Simple,
            },
            ConsoleOperationInventoryEntry {
                operation_id: "applications.view".to_string(),
                owner: owner.clone(),
                lifecycle: SettingsFeatureLifecycle::Active,
                policy_group: applications,
                label_ref: "console.operations.applications.view.label".to_string(),
                description_ref: Some(
                    "console.operations.applications.view.description".to_string(),
                ),
                order: 110,
                routes: vec![],
                authorization: ConsoleAuthorization::ResourceAction {
                    resource_code: "applications".to_string(),
                    action_code: "view".to_string(),
                },
            },
            ConsoleOperationInventoryEntry {
                operation_id: "files.upload".to_string(),
                owner: owner.clone(),
                lifecycle: SettingsFeatureLifecycle::Active,
                policy_group: files,
                label_ref: "console.operations.files.upload.label".to_string(),
                description_ref: Some("console.operations.files.upload.description".to_string()),
                order: 200,
                routes: vec![],
                authorization: ConsoleAuthorization::Simple,
            },
            ConsoleOperationInventoryEntry {
                operation_id: "files.content.download".to_string(),
                owner,
                lifecycle: SettingsFeatureLifecycle::Active,
                policy_group: other_files,
                label_ref: "console.operations.files.content.download.label".to_string(),
                description_ref: Some(
                    "console.operations.files.content.download.description".to_string(),
                ),
                order: 210,
                routes: vec![],
                authorization: ConsoleAuthorization::Simple,
            },
        ],
        resources: vec![ResourceAccessRegistration {
            resource_code: "applications".to_string(),
            owner: ConsoleOperationOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: "test".to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            scope_kind: ResourceAccessScopeKind::Workspace,
            identity_field: "id".to_string(),
            scope_field: Some("scope_id".to_string()),
            owner_field: Some("created_by".to_string()),
            label_ref: "console.resources.applications.label".to_string(),
            description_ref: Some("console.resources.applications.description".to_string()),
            actions: vec![
                ResourceAccessAction {
                    action_code: "create".to_string(),
                    label_ref: "console.resources.applications.actions.create.label".to_string(),
                    description_ref: Some(
                        "console.resources.applications.actions.create.description".to_string(),
                    ),
                },
                ResourceAccessAction {
                    action_code: "view".to_string(),
                    label_ref: "console.resources.applications.actions.view.label".to_string(),
                    description_ref: Some(
                        "console.resources.applications.actions.view.description".to_string(),
                    ),
                },
            ],
        }],
        locale_catalog: None,
    };

    let settings = SettingsFeatureRegistry::compile([
        SettingsFeatureRegistration {
            feature_id: "system.applications".to_string(),
            owner: ConsoleOperationOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: "test".to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.applications".to_string(),
                surface_key: "applications".to_string(),
                path: "/settings/applications".to_string(),
                label_key: "settings.system.applications.label".to_string(),
                description_key: "settings.system.applications.description".to_string(),
                order: 100,
            },
            api_routes: vec![SettingsApiRoute {
                method: "POST".to_string(),
                path: "/api/console/test/applications".to_string(),
            }],
        },
        SettingsFeatureRegistration {
            feature_id: "system.files".to_string(),
            owner: ConsoleOperationOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: "test".to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.files".to_string(),
                surface_key: "files".to_string(),
                path: "/settings/files".to_string(),
                label_key: "settings.system.files.label".to_string(),
                description_key: "settings.system.files.description".to_string(),
                order: 200,
            },
            api_routes: vec![SettingsApiRoute {
                method: "POST".to_string(),
                path: "/api/console/test/files/upload".to_string(),
            }],
        },
    ])
    .expect("test settings features must compile");
    let operation_entries = legacy_inventory.operations;
    let resources = legacy_inventory.resources;
    let operation_routes = [
        ("POST", "/api/console/test/applications"),
        ("GET", "/api/console/test/applications/:id"),
        ("POST", "/api/console/test/files/upload"),
        ("GET", "/api/console/test/files/:id/content"),
    ];
    let registrations = operation_entries
        .iter()
        .cloned()
        .zip(operation_routes)
        .map(|(entry, (method, path))| ConsoleOperationRegistration {
            operation_id: entry.operation_id,
            owner: entry.owner,
            lifecycle: entry.lifecycle,
            policy_group: entry.policy_group,
            label_ref: entry.label_ref,
            description_ref: entry.description_ref,
            order: entry.order,
            routes: vec![ConsoleRouteBinding {
                method: method.to_string(),
                path: path.to_string(),
            }],
            authorization: entry.authorization,
        })
        .collect::<Vec<_>>();
    let mut references = BTreeSet::new();
    for feature in &settings.inventory().features {
        references.insert(feature.console_surface.label_key.clone());
        references.insert(feature.console_surface.description_key.clone());
    }
    for operation in &operation_entries {
        references.insert(operation.label_ref.clone());
        references.insert(
            operation
                .description_ref
                .clone()
                .expect("test operation description ref"),
        );
    }
    for resource in &resources {
        references.insert(resource.label_ref.clone());
        references.insert(
            resource
                .description_ref
                .clone()
                .expect("test resource description ref"),
        );
        for action in &resource.actions {
            references.insert(action.label_ref.clone());
            references.insert(
                action
                    .description_ref
                    .clone()
                    .expect("test resource action description ref"),
            );
        }
    }
    for reference in [
        "console.policy.group_modes.disabled.label",
        "console.policy.group_modes.disabled.description",
        "console.policy.group_modes.full.label",
        "console.policy.group_modes.full.description",
        "console.policy.group_modes.custom.label",
        "console.policy.group_modes.custom.description",
        "console.policy.row_scopes.disabled.label",
        "console.policy.row_scopes.disabled.description",
        "console.policy.row_scopes.own.label",
        "console.policy.row_scopes.own.description",
        "console.policy.row_scopes.scope_all.label",
        "console.policy.row_scopes.scope_all.description",
        "console.policy_groups.other.other.files.label",
        "console.policy_groups.other.other.files.description",
    ] {
        references.insert(reference.to_string());
    }
    let texts = references
        .into_iter()
        .map(|reference| {
            let (en_us, zh_hans) = match reference.as_str() {
                "console.operations.applications.create.label" => {
                    ("Create application", "创建应用")
                }
                "console.operations.applications.view.label" => ("View applications", "查看应用"),
                "console.resources.applications.actions.view.label" => ("View", "查看"),
                "console.policy.row_scopes.own.label" => ("Own records", "仅自己"),
                "console.policy.row_scopes.scope_all.label" => ("Current workspace", "当前空间"),
                "console.policy.row_scopes.disabled.label" => ("Disabled", "关闭"),
                "console.policy.group_modes.disabled.label" => ("Disabled", "关闭"),
                "console.policy.group_modes.full.label" => ("Full access", "完全开放"),
                "console.policy.group_modes.custom.label" => ("Custom access", "自定义"),
                _ if reference.ends_with(".description") => ("Test description", "测试说明"),
                _ => ("Test label", "测试标签"),
            };
            ConsoleLocaleText {
                reference,
                en_us: en_us.to_string(),
                zh_hans: zh_hans.to_string(),
            }
        })
        .collect::<Vec<_>>();

    ConsoleOperationRegistry::compile_with_locale_catalog(
        &settings,
        registrations,
        resources,
        [ConsoleLocaleCatalogContribution {
            owner: ConsoleOperationOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: "test".to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            texts,
            policy_groups: vec![ConsoleOtherPolicyGroupDisplay {
                group_id: "other.files".to_string(),
                label_ref: "console.policy_groups.other.other.files.label".to_string(),
                description_ref: "console.policy_groups.other.other.files.description".to_string(),
            }],
        }],
    )
    .expect("test locale catalog must compile")
    .inventory()
    .clone()
}

fn policy_group(
    kind: &str,
    group_id: &str,
    mode: &str,
    operations: Vec<ConsolePolicyOperationInput>,
) -> ConsolePolicyGroupInput {
    ConsolePolicyGroupInput {
        kind: kind.to_string(),
        group_id: group_id.to_string(),
        mode: mode.to_string(),
        operations,
    }
}

async fn editable_role(
    service: &RoleService<MemoryRoleRepository>,
    repository: &MemoryRoleRepository,
    role_code: &str,
) {
    service
        .create_role(CreateRoleCommand {
            actor_user_id: repository.root_user_id(),
            code: role_code.to_string(),
            name: "Policy editor".to_string(),
            introduction: "Console policy test role".to_string(),
            auto_grant_new_permissions: false,
            is_default_member_role: false,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn role_service_console_policy_catalog_localizes_compiled_inventory() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());

    let catalog = service
        .get_console_policy_catalog(
            repository.root_user_id(),
            &console_policy_inventory(),
            "zh_Hans",
        )
        .await
        .unwrap();

    assert_eq!(
        catalog.schema_version,
        "1flowbase.console-operation-inventory/v1"
    );
    assert_eq!(catalog.locale, "zh_Hans");
    assert_eq!(
        catalog
            .group_mode_options
            .iter()
            .map(|option| option.value.as_str())
            .collect::<Vec<_>>(),
        vec!["disabled", "full", "custom"]
    );
    assert_eq!(catalog.group_mode_options[2].label, "自定义");
    assert!(!catalog.group_mode_options[2].description.is_empty());
    assert_eq!(catalog.groups.len(), 3);
    assert_eq!(catalog.groups[0].group_id, "system.applications");
    assert_eq!(catalog.groups[0].operations[0].label, "创建应用");
    assert_eq!(catalog.groups[0].operations[0].order, 100);
    assert!(catalog.groups[0].operations[0]
        .allowed_row_scopes
        .is_empty());
    assert_eq!(
        catalog.groups[0].operations[1].authorization.kind(),
        "resource_action"
    );
    assert_eq!(
        catalog.groups[0].operations[1]
            .allowed_row_scopes
            .iter()
            .map(|option| option.value.as_str())
            .collect::<Vec<_>>(),
        vec!["disabled", "own", "scope_all"]
    );
    assert_eq!(
        catalog.groups[0].operations[1].allowed_row_scopes[1].label,
        "仅自己"
    );
    assert_eq!(catalog.resources[0].resource_code, "applications");
    assert_eq!(catalog.resources[0].actions[1].label, "查看");
    assert!(!catalog.groups[0].operations[0]
        .label
        .contains("applications.create"));
}

#[tokio::test]
async fn role_service_console_policy_round_trips_disabled_full_custom_and_row_scopes() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());
    editable_role(&service, &repository, "policy-editor").await;

    let policy = service
        .replace_console_policy(
            ReplaceRoleConsolePolicyCommand {
                actor_user_id: repository.root_user_id(),
                role_code: "policy-editor".to_string(),
                groups: vec![
                    policy_group(
                        "settings_feature",
                        "system.applications",
                        "custom",
                        vec![
                            ConsolePolicyOperationInput::Simple {
                                operation_id: "applications.create".to_string(),
                                enabled: true,
                            },
                            ConsolePolicyOperationInput::Row {
                                operation_id: "applications.view".to_string(),
                                scope: "own".to_string(),
                            },
                        ],
                    ),
                    policy_group("settings_feature", "system.files", "full", vec![]),
                    policy_group("other", "other.files", "disabled", vec![]),
                ],
            },
            &console_policy_inventory(),
        )
        .await
        .unwrap();

    let fetched = service
        .get_console_policy(
            repository.root_user_id(),
            "policy-editor",
            &console_policy_inventory(),
        )
        .await
        .unwrap();

    assert_eq!(policy.groups(), fetched.groups());
    assert_eq!(
        fetched.groups()[0].mode(),
        domain::ConsolePolicyMode::Custom
    );
    assert_eq!(
        fetched.groups()[0].operations()[1].row_scope(),
        Some(domain::ConsoleOperationRowScope::Own)
    );
    assert_eq!(fetched.groups()[1].mode(), domain::ConsolePolicyMode::Full);
    assert_eq!(
        fetched.groups()[2].mode(),
        domain::ConsolePolicyMode::Disabled
    );
    assert_eq!(
        repository.audit_events(),
        vec!["role.created", "role.console_policy_replaced"]
    );
}

#[tokio::test]
async fn role_service_console_policy_rejects_system_all_scope() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());
    editable_role(&service, &repository, "policy-editor").await;

    let error = service
        .replace_console_policy(
            ReplaceRoleConsolePolicyCommand {
                actor_user_id: repository.root_user_id(),
                role_code: "policy-editor".to_string(),
                groups: vec![
                    policy_group(
                        "settings_feature",
                        "system.applications",
                        "custom",
                        vec![ConsolePolicyOperationInput::Row {
                            operation_id: "applications.view".to_string(),
                            scope: "system_all".to_string(),
                        }],
                    ),
                    policy_group("settings_feature", "system.files", "disabled", vec![]),
                    policy_group("other", "other.files", "disabled", vec![]),
                ],
            },
            &console_policy_inventory(),
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("console_policy_scope"));
    assert_eq!(repository.audit_events(), vec!["role.created"]);
}

#[tokio::test]
async fn role_service_console_policy_rejects_unknown_operation() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());
    editable_role(&service, &repository, "policy-editor").await;

    let error = service
        .replace_console_policy(
            ReplaceRoleConsolePolicyCommand {
                actor_user_id: repository.root_user_id(),
                role_code: "policy-editor".to_string(),
                groups: vec![
                    policy_group(
                        "settings_feature",
                        "system.applications",
                        "custom",
                        vec![ConsolePolicyOperationInput::Simple {
                            operation_id: "applications.unknown".to_string(),
                            enabled: true,
                        }],
                    ),
                    policy_group("settings_feature", "system.files", "disabled", vec![]),
                    policy_group("other", "other.files", "disabled", vec![]),
                ],
            },
            &console_policy_inventory(),
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("console_policy_operation"));
    assert_eq!(repository.audit_events(), vec!["role.created"]);
}

// AC-004/AC-006: the role-policy write boundary must reject malformed policy
// shapes rather than normalizing an ambiguous or wider grant.
#[tokio::test]
async fn role_service_console_policy_rejects_duplicate_groups_operations_type_mismatches_and_group_shapes(
) {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());
    editable_role(&service, &repository, "policy-editor").await;

    let duplicate_group = service
        .replace_console_policy(
            ReplaceRoleConsolePolicyCommand {
                actor_user_id: repository.root_user_id(),
                role_code: "policy-editor".to_string(),
                groups: vec![
                    policy_group(
                        "settings_feature",
                        "system.applications",
                        "disabled",
                        vec![],
                    ),
                    policy_group(
                        "settings_feature",
                        "system.applications",
                        "disabled",
                        vec![],
                    ),
                ],
            },
            &console_policy_inventory(),
        )
        .await
        .unwrap_err();
    assert!(duplicate_group
        .to_string()
        .contains("console_policy_group_duplicate"));

    let duplicate_operation = service
        .replace_console_policy(
            ReplaceRoleConsolePolicyCommand {
                actor_user_id: repository.root_user_id(),
                role_code: "policy-editor".to_string(),
                groups: vec![policy_group(
                    "settings_feature",
                    "system.applications",
                    "custom",
                    vec![
                        ConsolePolicyOperationInput::Simple {
                            operation_id: "applications.create".to_string(),
                            enabled: true,
                        },
                        ConsolePolicyOperationInput::Simple {
                            operation_id: "applications.create".to_string(),
                            enabled: false,
                        },
                    ],
                )],
            },
            &console_policy_inventory(),
        )
        .await
        .unwrap_err();
    assert!(duplicate_operation
        .to_string()
        .contains("console_policy_operation_duplicate"));

    let type_mismatch = service
        .replace_console_policy(
            ReplaceRoleConsolePolicyCommand {
                actor_user_id: repository.root_user_id(),
                role_code: "policy-editor".to_string(),
                groups: vec![policy_group(
                    "settings_feature",
                    "system.applications",
                    "custom",
                    vec![ConsolePolicyOperationInput::Row {
                        operation_id: "applications.create".to_string(),
                        scope: "own".to_string(),
                    }],
                )],
            },
            &console_policy_inventory(),
        )
        .await
        .unwrap_err();
    assert!(type_mismatch
        .to_string()
        .contains("console_policy_operation_type"));

    for mode in ["disabled", "full"] {
        let error = service
            .replace_console_policy(
                ReplaceRoleConsolePolicyCommand {
                    actor_user_id: repository.root_user_id(),
                    role_code: "policy-editor".to_string(),
                    groups: vec![policy_group(
                        "settings_feature",
                        "system.applications",
                        mode,
                        vec![ConsolePolicyOperationInput::Simple {
                            operation_id: "applications.create".to_string(),
                            enabled: true,
                        }],
                    )],
                },
                &console_policy_inventory(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("console_policy_group_shape"));
    }

    assert_eq!(repository.audit_events(), vec!["role.created"]);
}

// AC-004: an active catalog group without a stored row is a concrete denied
// group, so callers can render and edit every active group deterministically.
#[tokio::test]
async fn role_service_console_policy_defaults_missing_active_groups_to_disabled() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());
    editable_role(&service, &repository, "policy-editor").await;
    repository
        .replace_role_console_policy(&ReplaceRoleConsolePolicyInput {
            actor_user_id: repository.root_user_id(),
            workspace_id: Uuid::nil(),
            role_code: "policy-editor".to_string(),
            groups: vec![],
        })
        .await
        .unwrap();

    let policy = service
        .get_console_policy(
            repository.root_user_id(),
            "policy-editor",
            &console_policy_inventory(),
        )
        .await
        .unwrap();

    assert_eq!(policy.groups().len(), 3);
    assert!(policy
        .groups()
        .iter()
        .all(|group| group.mode() == domain::ConsolePolicyMode::Disabled));
}

#[tokio::test]
async fn role_service_rejects_root_mutation_and_replaces_permissions_for_team_roles() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());

    service
        .create_role(CreateRoleCommand {
            actor_user_id: repository.root_user_id(),
            code: "qa".into(),
            name: "QA".into(),
            introduction: "qa role".into(),
            auto_grant_new_permissions: false,
            is_default_member_role: false,
        })
        .await
        .unwrap();

    service
        .update_role(UpdateRoleCommand {
            actor_user_id: repository.root_user_id(),
            role_code: "qa".into(),
            name: "QA Updated".into(),
            introduction: "updated qa role".into(),
            auto_grant_new_permissions: None,
            is_default_member_role: None,
        })
        .await
        .unwrap();

    service
        .replace_permissions(ReplaceRolePermissionsCommand {
            actor_user_id: repository.root_user_id(),
            role_code: "qa".into(),
            permission_codes: vec!["application.view.own".into(), "application.edit.own".into()],
        })
        .await
        .unwrap();

    service
        .delete_role(DeleteRoleCommand {
            actor_user_id: repository.root_user_id(),
            role_code: "qa".into(),
        })
        .await
        .unwrap();

    assert!(service
        .replace_permissions(ReplaceRolePermissionsCommand {
            actor_user_id: repository.root_user_id(),
            role_code: "root".into(),
            permission_codes: vec!["workspace.configure.all".into()],
        })
        .await
        .is_err());
    assert_eq!(
        repository.audit_events(),
        vec![
            "role.created",
            "role.updated",
            "role.permissions_replaced",
            "role.deleted",
        ]
    );
}

#[tokio::test]
async fn role_service_tracks_policy_flags_on_create_and_update() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());

    service
        .create_role(CreateRoleCommand {
            actor_user_id: repository.root_user_id(),
            code: "qa".into(),
            name: "QA".into(),
            introduction: "qa role".into(),
            auto_grant_new_permissions: true,
            is_default_member_role: false,
        })
        .await
        .unwrap();

    service
        .update_role(UpdateRoleCommand {
            actor_user_id: repository.root_user_id(),
            role_code: "qa".into(),
            name: "QA Updated".into(),
            introduction: "updated qa role".into(),
            auto_grant_new_permissions: Some(false),
            is_default_member_role: Some(true),
        })
        .await
        .unwrap();

    let roles = service.list_roles(repository.root_user_id()).await.unwrap();
    let qa = roles.iter().find(|role| role.code == "qa").unwrap();

    assert_eq!(qa.name, "QA Updated");
    assert!(!qa.auto_grant_new_permissions);
    assert!(qa.is_default_member_role);
    assert_eq!(
        repository.audit_events(),
        vec!["role.created", "role.updated"]
    );
}

#[tokio::test]
async fn role_service_rejects_system_all_data_policy_for_workspace_roles() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());

    service
        .create_role(CreateRoleCommand {
            actor_user_id: repository.root_user_id(),
            code: "editor".into(),
            name: "Editor".into(),
            introduction: "editor role".into(),
            auto_grant_new_permissions: false,
            is_default_member_role: false,
        })
        .await
        .unwrap();

    let default_scope_error = service
        .replace_data_policy(ReplaceRoleDataPolicyCommand {
            actor_user_id: repository.root_user_id(),
            role_code: "editor".into(),
            default_policy: RoleDataPolicyDefaultsInput {
                can_view: true,
                can_create: true,
                can_update: true,
                can_delete: true,
                default_view_scope: domain::RoleDataPolicyScope::SystemAll,
                default_update_scope: domain::RoleDataPolicyScope::ScopeAll,
                default_delete_scope: domain::RoleDataPolicyScope::Own,
            },
            model_policies: Vec::new(),
        })
        .await
        .unwrap_err();
    assert!(default_scope_error
        .to_string()
        .contains("system_all_requires_system_role"));

    let model_scope_error = service
        .replace_data_policy(ReplaceRoleDataPolicyCommand {
            actor_user_id: repository.root_user_id(),
            role_code: "editor".into(),
            default_policy: RoleDataPolicyDefaultsInput {
                can_view: true,
                can_create: true,
                can_update: true,
                can_delete: true,
                default_view_scope: domain::RoleDataPolicyScope::ScopeAll,
                default_update_scope: domain::RoleDataPolicyScope::ScopeAll,
                default_delete_scope: domain::RoleDataPolicyScope::Own,
            },
            model_policies: vec![RoleDataModelPolicyInput {
                data_model_id: Uuid::now_v7(),
                can_create_override: None,
                view_scope_override: Some(domain::RoleDataPolicyScope::SystemAll),
                update_scope_override: None,
                delete_scope_override: None,
            }],
        })
        .await
        .unwrap_err();
    assert!(model_scope_error
        .to_string()
        .contains("system_all_requires_system_role"));
}
