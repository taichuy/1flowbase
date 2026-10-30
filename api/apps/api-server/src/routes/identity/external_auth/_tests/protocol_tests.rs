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

fn projection<T: interface_runtime::InterfaceContract>(value: &T) -> serde_json::Value {
    let projected = interface_runtime::ManagedInterfaceProjection::from_contract(value)
        .expect("every external-auth contract must publish an explicit safe summary");
    let contract = extension_contracts::ManagedProjectionContract {
        contract_id: projected.contract().contract_id().into(),
        contract_version: projected.contract().version().into(),
        schema: projected.schema().clone(),
    };
    crate::extension_bus::FrozenManagedProjection::compile(contract)
        .unwrap()
        .validate_projection(&projected)
        .unwrap();
    projected.value().clone()
}

#[test]
fn external_auth_safe_projections_compile_without_exporting_credential_canaries() {
    const SECRET: &str = "external_auth_private_canary_never_project";
    let entry_id = Uuid::new_v4();
    let output = || interface::AuthorizeOutput {
        cookies: CookieJar::new().add(Cookie::new("private_session", SECRET)),
        authorization_url: format!("https://provider.example/authorize?state={SECRET}"),
    };
    let public_input = projection(&interface::PublicInput::Start {
        login_entry_id: entry_id,
    });
    let public_output = projection(&interface::PublicOutput::Authorization(output()));
    let authorize_input = projection(&interface::AuthorizeInput(entry_id));
    let authorize_output = projection(&output());
    let target_error = projection(&interface::TargetError(anyhow::anyhow!(SECRET).into()));
    assert_eq!(public_input, serde_json::json!({}));
    assert_eq!(public_output, serde_json::json!({}));
    assert_eq!(
        authorize_input,
        serde_json::json!({"login_entry_id":entry_id.to_string()})
    );
    assert_eq!(
        authorize_output,
        serde_json::json!({"authorization_url":{"byte_count":output().authorization_url.len()}})
    );
    assert_eq!(
        target_error,
        serde_json::json!({"kind":"ExternalAuthTargetError"})
    );
    for value in [
        public_input,
        public_output,
        authorize_input,
        authorize_output,
        target_error,
    ] {
        let wire = value.to_string();
        for private in [
            SECRET,
            "private_session",
            "provider.example",
            "state=",
            "cookies",
            "profile",
            "identity",
            "authority",
        ] {
            assert!(
                !wire.contains(private),
                "private field escaped safe projection: {private}"
            );
        }
    }
}

#[tokio::test]
async fn external_auth_complete_boot_catalog_recognizes_all_safe_contracts() {
    use interface_runtime::InterfaceContract;
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let boot = state.extension_boot_snapshot.as_ref().unwrap();
    let _owner = boot
        .publish_complete_catalog(&state)
        .expect("complete managed catalog must accept every external-auth binding at startup");
    let registry = boot.interface_registry().unwrap().snapshot();
    for binding in [
        interface::START_BINDING,
        interface::CALLBACK_BINDING,
        interface::AUTHORIZE_BINDING,
    ] {
        let plan = registry
            .plan(&interface_runtime::BindingId::new(binding).unwrap())
            .unwrap();
        assert!(plan.has_managed_invocation_bridge());
    }
    for contract_id in [
        interface::PublicInput::CONTRACT_ID,
        interface::PublicOutput::CONTRACT_ID,
        interface::AuthorizeInput::CONTRACT_ID,
        interface::AuthorizeOutput::CONTRACT_ID,
        interface::TargetError::CONTRACT_ID,
    ] {
        let descriptor = registry
            .managed_contracts()
            .find(|item| item.contract.contract_id() == contract_id)
            .expect("startup catalog must retain the actual typed contract");
        crate::extension_bus::FrozenManagedProjection::compile(
            extension_contracts::ManagedProjectionContract {
                contract_id: descriptor.contract.contract_id().into(),
                contract_version: descriptor.contract.version().into(),
                schema: descriptor
                    .schema
                    .clone()
                    .expect("canonical startup requires every safe schema"),
            },
        )
        .unwrap();
    }
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
