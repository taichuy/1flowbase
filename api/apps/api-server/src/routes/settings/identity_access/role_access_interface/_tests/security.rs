use super::*;
#[test]
fn role_permissions_keep_all_entries_and_real_path_body_shape() {
    let operation = ApiEventOperationContext {
        interface_id: "roles.permissions.replace".into(),
        method: "PUT".into(),
        path: "/api/console/settings/roles/:id/permissions".into(),
    };
    let permissions = (0..80)
        .map(|i| format!("permission-{i}"))
        .collect::<Vec<_>>();
    let input = RoleAccessInput::ReplaceRolePermissions {
        role_code: "reader".into(),
        body: roles::ReplaceRolePermissionsBody {
            permission_codes: permissions.clone(),
        },
    };
    assert_eq!(
        input.project_document_input(&operation).unwrap(),
        json!({"path":{"id":"reader"},"body":{"permission_codes":permissions}})
    );
    assert_eq!(RoleAccessInput::document_input_schema(&operation), None);
}
#[test]
fn role_output_matches_http_business_response_without_enum_wrapper() {
    let operation = ApiEventOperationContext {
        interface_id: "roles.list".into(),
        method: "GET".into(),
        path: "/api/console/settings/roles".into(),
    };
    let roles = vec![roles::RoleResponse {
        code: "reader".into(),
        name: "Read only".repeat(100),
        introduction: "description".into(),
        scope_kind: "workspace".into(),
        is_builtin: false,
        is_editable: true,
        auto_grant_new_permissions: false,
        is_default_member_role: false,
        permission_codes: vec!["read".into()],
    }];
    let expected = serde_json::to_value(&roles).unwrap();
    let output = RoleAccessOutput::Roles(roles)
        .project_document_output(&operation)
        .unwrap();
    assert_eq!(output, expected);
    assert!(output.is_array());
}
