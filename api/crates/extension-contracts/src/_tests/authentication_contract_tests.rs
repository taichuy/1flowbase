use crate::{AuthenticationProviderDescriptor, AuthenticationRequest, AuthenticationResponse};
use serde_json::json;

#[test]
fn authentication_wire_preserves_oauth_fields_and_external_identity_without_host_authority() {
    let begin = json!({"operation":"begin","callback_url":"https://host.test/callback","state":"nonce","code_challenge":"challenge"});
    let request: AuthenticationRequest = serde_json::from_value(begin.clone()).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), begin);
    let complete = json!({"operation":"complete","callback_url":"https://host.test/callback","code":"one-time-code","code_verifier":"verifier"});
    let request: AuthenticationRequest = serde_json::from_value(complete.clone()).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), complete);
    let response = json!({"type":"verified_identity","subject_type":"github","subject_value":"42","issuer":"https://github.com","realm":null,"profile":{"email_verified":true}});
    let verified: AuthenticationResponse = serde_json::from_value(response.clone()).unwrap();
    assert_eq!(serde_json::to_value(verified).unwrap(), response);
    for forbidden in ["user_id", "session_id", "connection_id", "config"] {
        let mut forged = complete.clone();
        forged[forbidden] = json!("untrusted");
        assert!(serde_json::from_value::<AuthenticationRequest>(forged).is_err());
        let mut forged = response.clone();
        forged[forbidden] = json!("untrusted");
        assert!(serde_json::from_value::<AuthenticationResponse>(forged).is_err());
    }
    let eligibility = json!({"operation":"registration_eligibility","profile":{"created_at":"2020-01-01"},"now_unix":1792368000});
    assert!(matches!(
        serde_json::from_value::<AuthenticationRequest>(eligibility).unwrap(),
        AuthenticationRequest::RegistrationEligibility {
            now_unix: 1792368000,
            ..
        }
    ));
    let decision: AuthenticationResponse = serde_json::from_value(
        json!({"type":"registration_eligibility","allowed":false,"reason_code":"account_too_new"}),
    )
    .unwrap();
    assert!(matches!(
        decision,
        AuthenticationResponse::RegistrationEligibility { allowed: false, .. }
    ));
}

#[test]
fn authentication_descriptor_rejects_public_secret_controls_and_local_password_claims() {
    let valid = json!({"auth_type":"github","display_name":"GitHub","config_schema":[{"key":"token","label":"Token","type":"string","control":"password"}],"default_public_ui_block":"github-login","public_variable_keys":["client_id"],"contribution_id":"fixture.auth"});
    let descriptor: AuthenticationProviderDescriptor =
        serde_json::from_value(valid.clone()).unwrap();
    descriptor.validate().unwrap();
    for key in ["client_secret", "PASSWORD", "token"] {
        let mut value = valid.clone();
        value["public_variable_keys"] = json!([key]);
        assert!(
            serde_json::from_value::<AuthenticationProviderDescriptor>(value)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let mut value = valid;
    value["auth_type"] = json!("password-local");
    assert!(
        serde_json::from_value::<AuthenticationProviderDescriptor>(value)
            .unwrap()
            .validate()
            .is_err()
    );
}
