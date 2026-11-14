use super::*;
#[test]
fn credential_only_redirect_is_unavailable_and_never_projected_as_empty_business_result() {
    let operation = ApiOperationContext {
        interface_id: OPERATIONS[2].into(),
        method: "POST".into(),
        path: "/api/console/me/external-identities/authorize".into(),
    };
    assert_eq!(
        AuthorizeOutput::document_output_unavailable(&operation),
        Some(domain::event_center::ApiEventUnavailableReason::CredentialBoundary)
    );
    let value = AuthorizeOutput {
        cookies: axum_extra::extract::cookie::CookieJar::new(),
        authorization_url:
            "https://provider.test/authorize?state=STATE-SENTINEL&code_challenge=CHALLENGE-SENTINEL"
                .into(),
    };
    assert!(value.project_document_output(&operation).is_err());
    let input = AuthorizeInput(Uuid::nil())
        .project_document_input(&operation)
        .unwrap();
    assert_eq!(input, json!({"body":{"login_entry_id":Uuid::nil()}}));
}
#[test]
fn callback_credential_boundary_is_explicit_and_start_keeps_safe_query() {
    let mut operation = ApiOperationContext {
        interface_id: OPERATIONS[1].into(),
        method: "GET".into(),
        path: "/api/public/auth/external/callback".into(),
    };
    assert_eq!(
        PublicInput::document_input_unavailable(&operation),
        Some(domain::event_center::ApiEventUnavailableReason::CredentialBoundary)
    );
    operation.interface_id = OPERATIONS[0].into();
    assert_eq!(PublicInput::document_input_unavailable(&operation), None);
    assert_eq!(
        PublicInput::Start {
            login_entry_id: Uuid::nil()
        }
        .project_document_input(&operation)
        .unwrap(),
        json!({"query":{"login_entry_id":Uuid::nil()}})
    );
}
