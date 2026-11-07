use super::*;
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "PUT".into(),
        path: "/api/console/settings/auth-center/login-entries/:id/config".into(),
    }
}
#[test]
fn auth_center_event_drops_arbitrary_extension_credentials_and_keeps_business_config() {
    let op = operation("auth_center.login_entries.update.config");
    let input = AuthCenterInput::UpdateConfig {
        locale: crate::routes::console_interface::ConsoleLocaleHints::from_headers(
            &axum::http::HeaderMap::new(),
        ),
        id: Uuid::nil(),
        body: auth_center::UpdateAuthCenterLoginEntryConfigBody {
            title: "Corporate sign-in".into(),
            enabled: true,
            description: Some(Some("display".into())),
            self_registration_enabled: false,
            extension_config: Some(serde_json::Map::from_iter([(
                "client_secret".into(),
                json!("EXTENSION-SECRET-SENTINEL"),
            )])),
        },
    };
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(value["body"]["title"], "Corporate sign-in");
    assert_eq!(value["body"]["description"], "display");
    assert!(value["body"].get("extension_config").is_none());
    assert!(!value.to_string().contains("SENTINEL"));
    let schema = AuthCenterInput::document_input_schema(&op).unwrap();
    assert!(!schema.to_string().contains("extension_config"));
}
#[test]
fn auth_center_response_event_omits_config_values_and_schema_defaults() {
    let entry = auth_center::AuthCenterLoginEntryResponse {
        id: Uuid::nil(),
        auth_type: "oidc".into(),
        title: "SSO".into(),
        enabled: true,
        is_builtin: false,
        sort_order: 1,
        public_ui_block: "<button>Sign in</button>".into(),
        default_public_ui_block: None,
        interface_path_prefixes: vec!["/auth".into()],
        public_variables: None,
        context_variables: vec![],
        config_schema: vec![auth_center::AuthCenterConfigFieldResponse {
            key: "client_secret".into(),
            label: "Secret".into(),
            r#type: "secret".into(),
            control: None,
            read_only: None,
            required: Some(true),
            pattern: None,
            min: None,
            max: None,
            step: None,
            precision: None,
            description: None,
            default_value: Some(json!("DEFAULT-SECRET-SENTINEL")),
        }],
        config_values: serde_json::Map::from_iter([(
            "client_secret".into(),
            json!("OUTPUT-SECRET-SENTINEL"),
        )]),
        configured_secret_keys: vec!["client_secret".into()],
    };
    assert!(serde_json::to_value(&entry)
        .unwrap()
        .to_string()
        .contains("OUTPUT-SECRET-SENTINEL"));
    let op = operation("auth_center.login_entries.update.config");
    let value = AuthCenterOutput::Authenticator(entry)
        .project_document_output(&op)
        .unwrap();
    assert_eq!(value["title"], "SSO");
    assert_eq!(value["config_schema"][0]["key"], "client_secret");
    assert!(!value.to_string().contains("SENTINEL"));
    let schema = AuthCenterOutput::document_output_schema(&op).unwrap();
    assert!(!schema.to_string().contains("config_values"));
    assert!(!schema.to_string().contains("default_value"));
}
