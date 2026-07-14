use access_control::{
    ConsoleAuthorization, ConsoleOperationCompiledInventory, ConsoleOperationInventoryEntry,
    ConsoleOperationOwner, ConsolePolicyGroup, ResourceAccessAction, ResourceAccessRegistration,
    ResourceAccessScopeKind, SettingsFeatureLifecycle, SettingsFeatureOwnerKind,
};

use crate::_tests::support::MemoryRoleRepository;
use crate::ports::{RoleDataModelPolicyInput, RoleDataPolicyDefaultsInput};
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

    ConsoleOperationCompiledInventory {
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
    }
}

fn policy_group(kind: &str, group_id: &str, mode: &str, operations: Vec<ConsolePolicyOperationInput>) -> ConsolePolicyGroupInput {
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

    assert_eq!(catalog.schema_version, "1flowbase.console-operation-inventory/v1");
    assert_eq!(catalog.groups.len(), 3);
    assert_eq!(catalog.groups[0].group_id, "system.applications");
    assert_eq!(catalog.groups[0].operations[0].label, "创建应用");
    assert_eq!(catalog.groups[0].operations[1].authorization.kind(), "resource_action");
    assert_eq!(catalog.resources[0].resource_code, "applications");
    assert_eq!(catalog.resources[0].actions[1].label, "查看应用");
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
                    policy_group("settings_feature", "system.applications", "custom", vec![
                        ConsolePolicyOperationInput::Simple {
                            operation_id: "applications.create".to_string(),
                            enabled: true,
                        },
                        ConsolePolicyOperationInput::Row {
                            operation_id: "applications.view".to_string(),
                            scope: "own".to_string(),
                        },
                    ]),
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
    assert_eq!(fetched.groups()[0].mode(), domain::ConsolePolicyMode::Custom);
    assert_eq!(fetched.groups()[0].operations()[1].row_scope(), Some(domain::ConsoleOperationRowScope::Own));
    assert_eq!(fetched.groups()[1].mode(), domain::ConsolePolicyMode::Full);
    assert_eq!(fetched.groups()[2].mode(), domain::ConsolePolicyMode::Disabled);
    assert_eq!(repository.audit_events(), vec!["role.created", "role.console_policy_replaced"]);
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
