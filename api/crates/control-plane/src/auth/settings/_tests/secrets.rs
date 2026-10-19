use super::*;
use uuid::Uuid;
struct FixtureCodec;
impl AuthenticationSecretCodec for FixtureCodec {
    fn seal(&self, connection_id: Uuid, value: &Value) -> Result<Value> {
        Ok(serde_json::json!({"connection":connection_id,"value":value}))
    }
    fn open(&self, connection_id: Uuid, envelope: &Value) -> Result<Value> {
        anyhow::ensure!(
            envelope["connection"] == connection_id.to_string(),
            "fixture connection mismatch"
        );
        Ok(envelope["value"].clone())
    }
}
fn entry() -> domain::LoginEntryRecord {
    domain::LoginEntryRecord {
        id: Uuid::now_v7(),
        connection_id: Uuid::now_v7(),
        auth_type: "1flowbase/github-login".into(),
        title: "GitHub".into(),
        enabled: true,
        is_builtin: false,
        sort_order: 0,
        public_ui_block: "block".into(),
        options: serde_json::json!({}),
    }
}
fn schema() -> Value {
    serde_json::json!([
        {"key":"client_id","type":"string","required":true},
        {"key":"client_secret","type":"secret","required":true},
        {"key":"minimum_account_age_years","type":"number","min":0,"precision":0},
        {"key":"callback_url","type":"string","required":true}
    ])
}
#[test]
fn save_and_resave_keep_secrets_out_of_public_configuration() {
    let mut entry = entry();
    let mut config = serde_json::json!({"client_id":"fixture-id","client_secret":"fixture-secret","minimum_account_age_years":1,"callback_url":"https://untrusted.example/callback"}).as_object().unwrap().clone();
    protect_configuration(&mut entry, &mut config, &schema(), Some(&FixtureCodec)).unwrap();
    assert!(!config.contains_key("client_secret"));
    assert!(!config.contains_key("callback_url"));
    assert_eq!(
        entry.options["authentication_secret_keys"],
        serde_json::json!(["client_secret"])
    );
    let original = entry.options["authentication_secret_envelope"].clone();
    config.insert("client_secret".into(), Value::String(String::new()));
    protect_configuration(&mut entry, &mut config, &schema(), Some(&FixtureCodec)).unwrap();
    assert_eq!(entry.options["authentication_secret_envelope"], original);
    assert!(!config.contains_key("client_secret"));
}
#[test]
fn missing_cipher_unknown_fields_and_invalid_age_cannot_be_saved() {
    let config = serde_json::json!({"client_id":"fixture-id","client_secret":"fixture-secret","minimum_account_age_years":0}).as_object().unwrap().clone();
    assert!(protect_configuration(&mut entry(), &mut config.clone(), &schema(), None).is_err());
    for age in [
        serde_json::json!(-1),
        serde_json::json!(0.5),
        serde_json::json!("1"),
    ] {
        let mut config = config.clone();
        config.insert("minimum_account_age_years".into(), age);
        assert!(
            protect_configuration(&mut entry(), &mut config, &schema(), Some(&FixtureCodec))
                .is_err()
        );
    }
    let mut config = config;
    config.insert("undeclared_secret".into(), serde_json::json!("forbidden"));
    assert!(
        protect_configuration(&mut entry(), &mut config, &schema(), Some(&FixtureCodec)).is_err()
    );
}
