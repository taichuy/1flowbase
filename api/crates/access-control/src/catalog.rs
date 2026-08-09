use domain::{PermissionDefinition, RoleScopeKind, RoleTemplate};

use crate::{
    settings_feature_permission_definitions,
    SYSTEM_API_KEY_AUTHENTICATION_SETTINGS_FEATURE_PERMISSION,
};

fn push_permissions(
    permissions: &mut Vec<PermissionDefinition>,
    resource: &str,
    actions: &[(&str, &[&str])],
) {
    for (action, scopes) in actions {
        for scope in *scopes {
            permissions.push(PermissionDefinition {
                code: format!("{resource}.{action}.{scope}"),
                resource: resource.to_string(),
                action: (*action).to_string(),
                scope: (*scope).to_string(),
                name: format!("{resource}:{action}:{scope}"),
            });
        }
    }
}

pub fn permission_catalog() -> Vec<PermissionDefinition> {
    let mut permissions = Vec::new();

    push_permissions(
        &mut permissions,
        "application",
        &[
            ("view", &["own", "all"]),
            ("create", &["all"]),
            ("edit", &["own", "all"]),
            ("delete", &["own", "all"]),
            ("manage", &["own", "all"]),
            ("use", &["own", "all"]),
        ],
    );
    push_permissions(
        &mut permissions,
        "flow",
        &[
            ("view", &["own", "all"]),
            ("create", &["all"]),
            ("edit", &["own", "all"]),
            ("delete", &["own", "all"]),
            ("manage", &["own", "all"]),
            ("use", &["own", "all"]),
        ],
    );
    push_permissions(
        &mut permissions,
        "publish_endpoint",
        &[
            ("view", &["own", "all"]),
            ("create", &["all"]),
            ("edit", &["own", "all"]),
            ("delete", &["own", "all"]),
            ("publish", &["own", "all"]),
            ("use", &["own", "all"]),
        ],
    );
    push_permissions(&mut permissions, "frontstage", &[("page", &["design"])]);
    push_permissions(&mut permissions, "ui_block", &[("javascript", &["native"])]);
    push_permissions(
        &mut permissions,
        "state_model",
        &[
            ("view", &["own", "all"]),
            ("create", &["all"]),
            ("edit", &["own", "all"]),
            ("delete", &["own", "all"]),
            ("manage", &["own", "all"]),
        ],
    );
    push_permissions(
        &mut permissions,
        "state_data",
        &[
            ("view", &["own", "all"]),
            ("create", &["all"]),
            ("edit", &["own", "all"]),
            ("delete", &["own", "all"]),
            ("manage", &["own", "all"]),
            ("use", &["own", "all"]),
        ],
    );
    push_permissions(
        &mut permissions,
        "external_data_source",
        &[
            ("view", &["own", "all"]),
            ("create", &["all"]),
            ("edit", &["own", "all"]),
            ("delete", &["own", "all"]),
            ("configure", &["own", "all"]),
            ("use", &["own", "all"]),
        ],
    );
    push_permissions(
        &mut permissions,
        "plugin_config",
        &[
            ("view", &["all"]),
            ("edit", &["all"]),
            ("configure", &["all"]),
        ],
    );
    push_permissions(
        &mut permissions,
        "embedded_app",
        &[
            ("view", &["own", "all"]),
            ("create", &["all"]),
            ("edit", &["own", "all"]),
            ("delete", &["own", "all"]),
            ("use", &["own", "all"]),
        ],
    );
    push_permissions(
        &mut permissions,
        "file_storage",
        &[("view", &["all"]), ("manage", &["all"])],
    );
    push_permissions(
        &mut permissions,
        "file_table",
        &[
            ("view", &["own", "all"]),
            ("create", &["all"]),
            ("delete", &["own", "all"]),
            ("bind", &["all"]),
        ],
    );
    push_permissions(
        &mut permissions,
        "user",
        &[("view", &["all"]), ("manage", &["all"])],
    );
    push_permissions(
        &mut permissions,
        "role_permission",
        &[("view", &["all"]), ("manage", &["all"])],
    );
    push_permissions(
        &mut permissions,
        "workspace",
        &[("view", &["all"]), ("configure", &["all"])],
    );
    push_permissions(&mut permissions, "system_runtime", &[("view", &["all"])]);
    push_permissions(&mut permissions, "api_reference", &[("view", &["all"])]);
    push_permissions(
        &mut permissions,
        "mcp_management",
        &[("view", &["all"]), ("manage", &["all"])],
    );
    permissions.extend(settings_feature_permission_definitions());

    permissions
}

pub fn bootstrap_role_templates() -> Vec<RoleTemplate> {
    let all_codes = permission_catalog()
        .into_iter()
        .map(|permission| permission.code)
        .collect::<Vec<_>>();

    let member_resources = [
        "application",
        "flow",
        "publish_endpoint",
        "frontstage",
        "state_model",
        "state_data",
        "external_data_source",
        "embedded_app",
    ];
    let member_permissions = all_codes
        .iter()
        .filter(|code| {
            let matches_resource = member_resources
                .iter()
                .any(|resource| code.starts_with(resource));

            matches_resource && (!code.ends_with(".all") || code.ends_with(".create.all"))
        })
        .cloned()
        .chain([SYSTEM_API_KEY_AUTHENTICATION_SETTINGS_FEATURE_PERMISSION.to_string()])
        .collect();

    vec![
        RoleTemplate {
            code: "root".to_string(),
            name: "Root".to_string(),
            introduction: "系统最高权限角色".to_string(),
            scope_kind: RoleScopeKind::System,
            is_builtin: true,
            is_editable: false,
            auto_grant_new_permissions: false,
            is_default_member_role: false,
            permissions: Vec::new(),
        },
        RoleTemplate {
            code: "admin".to_string(),
            name: "Admin".to_string(),
            introduction: "工作区管理员角色".to_string(),
            scope_kind: RoleScopeKind::Workspace,
            is_builtin: false,
            is_editable: true,
            auto_grant_new_permissions: true,
            is_default_member_role: false,
            permissions: all_codes.clone(),
        },
        RoleTemplate {
            code: "member".to_string(),
            name: "Member".to_string(),
            introduction: "工作区普通成员默认角色".to_string(),
            scope_kind: RoleScopeKind::Workspace,
            is_builtin: false,
            is_editable: true,
            auto_grant_new_permissions: false,
            is_default_member_role: true,
            permissions: member_permissions,
        },
    ]
}
