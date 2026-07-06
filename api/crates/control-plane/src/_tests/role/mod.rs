use crate::_tests::support::MemoryRoleRepository;
use crate::ports::{RoleDataModelPolicyInput, RoleDataPolicyDefaultsInput};
use crate::role::{
    CreateRoleCommand, DeleteRoleCommand, ReplaceRoleDataPolicyCommand,
    ReplaceRolePermissionsCommand, RoleService, UpdateRoleCommand,
};
use uuid::Uuid;

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
            code: "manager".into(),
            name: "Manager".into(),
            introduction: "manager role".into(),
            auto_grant_new_permissions: false,
            is_default_member_role: false,
        })
        .await
        .unwrap();

    let default_scope_error = service
        .replace_data_policy(ReplaceRoleDataPolicyCommand {
            actor_user_id: repository.root_user_id(),
            role_code: "manager".into(),
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
            role_code: "manager".into(),
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
