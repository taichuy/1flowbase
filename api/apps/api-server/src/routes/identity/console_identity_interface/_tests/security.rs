use super::*;
fn operation(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/user-api-keys".into(),
    }
}
#[test]
fn api_key_event_omits_token_without_changing_http_response() {
    let response = user_api_keys::UserApiKeyResponse {
        id: Uuid::nil(),
        name: "integration-key".into(),
        token: Some("API-KEY-SENTINEL".into()),
        token_prefix: "fb_".into(),
        key_kind: "user".into(),
        role_code: Some("reader".into()),
        creator_user_id: Uuid::nil(),
        tenant_id: Uuid::nil(),
        scope_kind: "workspace".into(),
        scope_id: Uuid::nil(),
        enabled: true,
        revoked: false,
        expires_at: None,
        last_used_at: None,
        created_at: "2026-11-07".into(),
        updated_at: "2026-11-07".into(),
    };
    assert_eq!(
        serde_json::to_value(&response).unwrap()["token"],
        "API-KEY-SENTINEL"
    );
    let op = operation("console.identity.user-api-keys.create");
    let value = ConsoleIdentityOutput::UserApiKeyCreated(response)
        .project_document_output(&op)
        .unwrap();
    assert!(value.get("token").is_none());
    assert_eq!(value["name"], "integration-key");
    assert_eq!(value["role_code"], "reader");
    let schema = ConsoleIdentityOutput::document_output_schema(&op).unwrap();
    assert!(!schema.to_string().contains("\"token\""));
    assert!(schema.to_string().contains("token_prefix"));
}
#[test]
fn session_event_omits_handle_csrf_and_future_unknown_credentials() {
    let id = Uuid::nil().to_string();
    let response = session::SessionResponse {
        actor: json!({"id":id,"account":"alice","effective_display_role":"reader","current_workspace_id":id,"credential":"ACTOR-SENTINEL"}),
        session: json!({"id":"SESSION-HANDLE-SENTINEL","user_id":id,"tenant_id":id,"current_workspace_id":id,"active_role_code":"reader","future_secret":"SESSION-EXTRA-SENTINEL"}),
        available_roles: vec![],
        active_role_permissions: vec!["read".into()],
        csrf_token: "CSRF-SENTINEL".into(),
        cookie_name: "session".into(),
    };
    let op = operation("console.identity.session.get");
    let value = ConsoleIdentityOutput::Session(response)
        .project_document_output(&op)
        .unwrap();
    assert!(!value.to_string().contains("SENTINEL"));
    assert_eq!(value["session"]["current_workspace_id"], id);
    assert!(value["session"].get("id").is_none());
    assert!(value.get("csrf_token").is_none());
    let schema = ConsoleIdentityOutput::document_output_schema(&op).unwrap();
    assert!(!schema.to_string().contains("csrf_token"));
    assert!(!schema.to_string().contains("session_id"));
}
#[test]
fn change_password_event_is_safe_in_schema_and_value() {
    let op = operation("console.identity.me.change-password");
    let input = ConsoleIdentityInput::ChangePassword(me::ChangePasswordBody {
        old_password: "OLD-SENTINEL".into(),
        new_password: "NEW-SENTINEL".into(),
    });
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"body":{}})
    );
    assert!(!ConsoleIdentityInput::document_input_schema(&op)
        .unwrap()
        .to_string()
        .contains("password"));
}
