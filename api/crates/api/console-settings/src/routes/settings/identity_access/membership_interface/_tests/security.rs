use super::*;
fn operation(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/settings/members".into(),
    }
}
#[test]
fn member_event_preserves_business_fields_and_omits_password_in_value_and_schema() {
    let expected_name = "成员".repeat(200);
    let input = MembershipInput::CreateMember(members::CreateMemberBody {
        account: "ordinary-account".into(),
        email: "person@example.test".into(),
        phone: None,
        password: "PASSWORD-SENTINEL-DO-NOT-PUBLISH".into(),
        name: expected_name.clone(),
        nickname: "nick".into(),
        introduction: "intro".into(),
        email_login_enabled: true,
        phone_login_enabled: false,
    });
    let op = operation("members.create");
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(value["body"]["account"], "ordinary-account");
    assert_eq!(value["body"]["name"], expected_name);
    assert!(value["body"].get("password").is_none());
    assert!(!value.to_string().contains("PASSWORD-SENTINEL"));
    let schema = MembershipInput::document_input_schema(&op).unwrap();
    assert!(!schema.to_string().contains("password"));
    assert!(schema.to_string().contains("email_login_enabled"));
}
#[test]
fn reset_member_never_exposes_new_password() {
    let op = operation("members.password.reset");
    let value = MembershipInput::ResetMember {
        member_id: "member-a".into(),
        body: members::ResetMemberPasswordBody {
            new_password: "RESET-SENTINEL".into(),
        },
    }
    .project_document_input(&op)
    .unwrap();
    assert_eq!(value, json!({"path":{"id":"member-a"},"body":{}}));
    assert!(!MembershipInput::document_input_schema(&op)
        .unwrap()
        .to_string()
        .contains("new_password"));
}
#[test]
fn member_role_array_is_complete_and_has_documented_body_shape() {
    let roles = (0..96).map(|n| format!("role-{n}")).collect::<Vec<_>>();
    let value = MembershipInput::ReplaceMemberRoles {
        member_id: "member-a".into(),
        body: members::ReplaceMemberRolesBody {
            role_codes: roles.clone(),
        },
    }
    .project_document_input(&operation("members.roles.replace"))
    .unwrap();
    assert_eq!(
        value,
        json!({"path":{"id":"member-a"},"body":{"role_codes":roles}})
    );
}
