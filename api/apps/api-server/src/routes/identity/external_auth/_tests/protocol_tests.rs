use super::*;

fn entry() -> LoginEntryRecord {
    LoginEntryRecord {
        id: Uuid::new_v4(),
        connection_id: Uuid::new_v4(),
        auth_type: "github".into(),
        title: "GitHub".into(),
        enabled: true,
        is_builtin: false,
        sort_order: 0,
        public_ui_block: "block".into(),
        options: serde_json::json!({}),
    }
}

#[test]
fn disabled_builtin_or_local_entries_cannot_start_oauth() {
    let mut entry = entry();
    assert!(validate_entry(&entry).is_ok());
    entry.enabled = false;
    assert!(validate_entry(&entry).is_err());
    entry.enabled = true;
    entry.is_builtin = true;
    assert!(validate_entry(&entry).is_err());
    entry.is_builtin = false;
    entry.connection_id = domain::PASSWORD_LOCAL_CONNECTION_ID;
    assert!(validate_entry(&entry).is_err());
}

#[test]
fn duplicate_state_code_and_entry_parameters_are_not_silently_selected() {
    for query in [
        "state=a&state=b&code=c",
        "state=a&code=b&code=c",
        "state=a&%73tate=b&code=c",
    ] {
        assert!(unique_query(Some(query)).is_err());
    }
    let id = Uuid::new_v4();
    assert_eq!(
        start_entry_id(Some(&format!("login_entry_id={id}"))).unwrap(),
        id
    );
    for query in [
        format!("login_entry_id={id}&login_entry_id={id}"),
        format!("login_entry_id={id}&return_to=https://evil.example/"),
        format!("login_entry_id={id}&user_id={id}"),
    ] {
        assert!(start_entry_id(Some(&query)).is_err());
    }
    // The authenticated JSON operation does not accept an anonymous actor ID.
    assert!(serde_json::from_value::<AuthorizeBody>(
        serde_json::json!({"login_entry_id":id,"user_id":id})
    )
    .is_err());
}

#[test]
fn authorization_redirect_requires_https_and_host_state_pkce_callback() {
    let callback = "https://app.example/api/public/auth/external/callback";
    let mut url = url::Url::parse("https://github.com/login/oauth/authorize").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("state", "state"),
        ("code_challenge", "challenge"),
        ("code_challenge_method", "S256"),
        ("redirect_uri", callback),
        ("response_type", "code"),
    ]);
    assert!(authorization_url(url.to_string(), "state", "challenge", callback).is_ok());
    assert!(authorization_url(url.to_string(), "wrong-state", "challenge", callback).is_err());
    assert!(authorization_url(url.to_string(), "state", "wrong-challenge", callback).is_err());
    assert!(authorization_url(
        url.to_string(),
        "state",
        "challenge",
        "https://evil.example/"
    )
    .is_err());
    for candidate in [
        url.as_str().replacen("https:", "http:", 1),
        format!("{}#token=secret", url.as_str()),
        url.as_str()
            .replacen("github.com", "attacker@github.com", 1),
        format!("{}&state=another", url.as_str()),
    ] {
        assert!(authorization_url(candidate, "state", "challenge", callback).is_err());
    }
}

#[test]
fn interface_contracts_have_no_credential_or_profile_projection() {
    use interface_runtime::InterfaceContract;
    assert!(interface::PublicInput::managed_projection_schema().is_none());
    assert!(interface::PublicOutput::managed_projection_schema().is_none());
    assert!(interface::AuthorizeInput::managed_projection_schema().is_none());
}

#[test]
fn callback_cancellation_and_failures_use_only_finite_safe_redirects() {
    let query = unique_query(Some(
        "state=unused&error=access_denied&error_description=raw-secret",
    ))
    .unwrap();
    assert_eq!(
        failure_redirect(&callback_code(&query).unwrap_err()),
        "/sign-in?auth_error=cancelled"
    );
    let query = unique_query(Some("state=unused&code=secret-code&error=access_denied")).unwrap();
    assert_eq!(
        failure_redirect(&callback_code(&query).unwrap_err()),
        "/sign-in?auth_error=external_auth_failed"
    );
    let query = unique_query(Some(
        "state=unused&error=raw-secret&error_uri=https://evil.example/",
    ))
    .unwrap();
    assert_eq!(
        failure_redirect(&callback_code(&query).unwrap_err()),
        "/sign-in?auth_error=external_auth_failed"
    );
    let query = unique_query(Some("state=unused&code=accepted")).unwrap();
    assert_eq!(callback_code(&query).unwrap(), "accepted");
    assert_eq!(
        failure_redirect(&eligibility_failure(Some("github_account_too_new"))),
        "/sign-in?auth_error=account_too_new"
    );
    assert_eq!(
        failure_redirect(&eligibility_failure(Some("raw-secret"))),
        "/sign-in?auth_error=registration_ineligible"
    );
    assert_eq!(
        failure_redirect(&transaction::denied()),
        "/sign-in?auth_error=invalid_state"
    );
    assert_eq!(
        failure_redirect(&ApiError::from(
            control_plane::errors::ControlPlaneError::PermissionDenied(
                "self_registration_disabled"
            )
        )),
        "/sign-in?auth_error=registration_disabled"
    );
    assert_eq!(
        failure_redirect(&ApiError::from(
            control_plane::errors::ControlPlaneError::InvalidInput(
                "external_registration_email_required"
            )
        )),
        "/sign-in?auth_error=email_required"
    );
    assert_eq!(
        failure_redirect(&ApiError::from(
            control_plane::errors::ControlPlaneError::Conflict("external_identity_already_bound")
        )),
        "/sign-in?auth_error=identity_exists"
    );
}
