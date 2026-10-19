use super::*;
#[test]
fn authentication_secrets_reject_relocation_and_wrong_key() {
    let cipher = AuthenticationSecretCipher::new("fixture-auth-secret-key");
    let connection = Uuid::now_v7();
    let secret = serde_json::json!({"client_secret":"fixture-value"});
    let envelope = cipher.seal(connection, &secret).unwrap();
    assert!(!envelope.to_string().contains("fixture-value"));
    assert_eq!(cipher.open(connection, &envelope).unwrap(), secret);
    assert!(cipher.open(Uuid::now_v7(), &envelope).is_err());
    assert!(AuthenticationSecretCipher::new("another-key")
        .open(connection, &envelope)
        .is_err());
}
