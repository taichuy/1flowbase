use access_control::{builtin_role_templates, permission_catalog};

#[test]
fn permission_catalog_seeds_expected_codes() {
    let codes: Vec<String> = permission_catalog()
        .into_iter()
        .map(|permission| permission.code)
        .collect();

    assert!(codes.contains(&"frontstage.page.design".to_string()));
    assert!(codes.contains(&"ui_block.javascript.native".to_string()));
    assert!(codes.contains(&"user.manage.all".to_string()));
    assert!(codes.contains(&"workspace.configure.all".to_string()));
    assert!(!codes.iter().any(|code| code.starts_with("team.")));
    assert!(!codes.iter().any(|code| code.starts_with("route_page.")));
}

#[test]
fn permission_catalog_seeds_api_reference_view_all() {
    let codes = permission_catalog()
        .into_iter()
        .map(|permission| permission.code)
        .collect::<Vec<_>>();

    assert!(codes.contains(&"api_reference.view.all".to_string()));
}

#[test]
fn permission_catalog_seeds_migrated_settings_feature_codes() {
    let codes = permission_catalog()
        .into_iter()
        .map(|permission| permission.code)
        .collect::<Vec<_>>();

    assert!(codes.contains(&"settings_feature.access.system.roles".to_string()));
    assert!(codes.contains(&"settings_feature.access.system.members".to_string()));
    for feature in [
        "docs",
        "api-key-authentication",
        "auth-center",
        "system-runtime",
        "host-infrastructure",
        "i18n-catalog",
        "memory-observation",
        "applications",
        "files",
        "data-models",
        "mcp-management",
        "model-providers",
    ] {
        assert!(codes.contains(&format!("settings_feature.access.system.{feature}")));
    }
    assert!(!codes
        .iter()
        .any(|code| code.starts_with("settings_route.visible.")));
}

#[test]
fn builtin_roles_keep_root_internal_and_expose_admin_and_member_for_workspaces() {
    let templates = builtin_role_templates();

    let root = templates.iter().find(|role| role.code == "root").unwrap();
    let admin = templates.iter().find(|role| role.code == "admin").unwrap();
    let member = templates.iter().find(|role| role.code == "member").unwrap();

    assert_eq!(root.scope_kind, domain::RoleScopeKind::System);
    assert!(!root.is_editable);
    assert_eq!(admin.scope_kind, domain::RoleScopeKind::Workspace);
    assert!(admin.is_editable);
    assert_eq!(member.scope_kind, domain::RoleScopeKind::Workspace);
    assert!(member.is_editable);
    assert!(member.is_default_member_role);
    assert!(!templates.iter().any(|role| role.code == "manager"));
}

#[test]
fn member_role_includes_frontstage_design_permission_by_default() {
    let templates = builtin_role_templates();
    let member = templates.iter().find(|role| role.code == "member").unwrap();

    assert!(member
        .permissions
        .contains(&"frontstage.page.design".to_string()));
}

#[test]
fn member_role_keeps_api_key_authentication_settings_feature_by_default() {
    let templates = builtin_role_templates();
    let member = templates.iter().find(|role| role.code == "member").unwrap();

    assert!(member
        .permissions
        .contains(&"settings_feature.access.system.api-key-authentication".to_string()));
    assert!(!member
        .permissions
        .contains(&"settings_feature.access.system.roles".to_string()));
}
