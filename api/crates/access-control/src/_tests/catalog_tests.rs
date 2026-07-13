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

    assert!(codes.contains(&"settings_route.visible.settings.api-key-authentication".to_string()));
    assert!(codes.contains(&"settings_feature.access.system.roles".to_string()));
    assert!(codes.contains(&"settings_feature.access.system.members".to_string()));
    assert!(!codes.contains(&"settings_route.visible.settings.roles".to_string()));
    assert!(!codes.contains(&"settings_route.visible.settings.members".to_string()));
}

#[test]
fn builtin_roles_lock_root_but_keep_admin_and_manager_editable() {
    let templates = builtin_role_templates();

    let root = templates.iter().find(|role| role.code == "root").unwrap();
    let admin = templates.iter().find(|role| role.code == "admin").unwrap();
    let manager = templates
        .iter()
        .find(|role| role.code == "manager")
        .unwrap();

    assert!(!root.is_editable);
    assert!(admin.is_editable);
    assert!(manager.is_editable);
}

#[test]
fn manager_role_includes_frontstage_design_permission_by_default() {
    let templates = builtin_role_templates();
    let manager = templates
        .iter()
        .find(|role| role.code == "manager")
        .unwrap();

    assert!(manager
        .permissions
        .contains(&"frontstage.page.design".to_string()));
}

#[test]
fn manager_role_keeps_api_key_authentication_settings_route_permission_by_default() {
    let templates = builtin_role_templates();
    let manager = templates
        .iter()
        .find(|role| role.code == "manager")
        .unwrap();

    assert!(manager
        .permissions
        .contains(&"settings_route.visible.settings.api-key-authentication".to_string()));
    assert!(!manager
        .permissions
        .contains(&"settings_feature.access.system.roles".to_string()));
}
