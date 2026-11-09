use super::*;
#[test]
fn password_never_enters_sign_in_event_schema_or_value() {
    let operation = ApiOperationContext {
        interface_id: "public.auth.sign-in".into(),
        method: "POST".into(),
        path: "/api/public/auth/sign-in".into(),
    };
    let input = PublicSignInInput(LoginCommand {
        login_entry_id: Uuid::nil(),
        identifier: "alice".into(),
        password: "SIGN-IN-PASSWORD-SENTINEL".into(),
    });
    let value = input.project_document_input(&operation).unwrap();
    assert_eq!(value["body"]["identifier"], "alice");
    assert!(value["body"].get("password").is_none());
    assert!(!value.to_string().contains("SENTINEL"));
    assert!(!PublicSignInInput::document_input_schema(&operation)
        .unwrap()
        .to_string()
        .contains("password"));
    let output_schema = PublicSignInOutput::document_output_schema(&operation).unwrap();
    assert!(!output_schema.to_string().contains("csrf_token"));
    assert!(!output_schema.to_string().contains("session"));
}
