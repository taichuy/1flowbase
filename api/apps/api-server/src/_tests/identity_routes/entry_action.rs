use super::*;
use extension_contracts::{AuthenticationEntryAction, AuthenticationProviderDescriptor};
use interface_runtime::InterfaceContract;

fn entry(auth_type: &str, enabled: bool) -> domain::LoginEntryRecord {
    domain::LoginEntryRecord {
        id: Uuid::now_v7(),
        connection_id: Uuid::now_v7(),
        auth_type: auth_type.into(),
        title: "Custom provider label".into(),
        enabled,
        is_builtin: false,
        sort_order: 10,
        public_ui_block: "form canary".into(),
        options: serde_json::json!({}),
    }
}

#[test]
fn external_auth_entry_action_comes_from_active_definition_and_safe_public_projection() {
    // An arbitrary provider identifier and title must behave like GitHub when its
    // declaration selects external_login; persisted UI/options cannot override it.
    let descriptor: AuthenticationProviderDescriptor = serde_json::from_value(serde_json::json!({
        "auth_type":"example/renamed", "display_name":"Custom", "entry_action":"external_login",
        "config_schema":[], "default_public_ui_block":"form canary", "public_variable_keys":[],
        "contribution_id":"fixture.auth"
    }))
    .unwrap();
    let registry = AuthenticatorRegistry::new()
        .with_managed_definitions(&[descriptor])
        .unwrap();
    let mut record = entry("example/renamed", true);
    record.options =
        serde_json::json!({"entry_action":"render_ui", "private_secret":"credential-canary"});
    let public = to_public_login_entry(record, &registry).unwrap();
    assert_eq!(
        public.entry_action,
        AuthenticationEntryAction::ExternalLogin
    );
    let json = serde_json::to_value(&public).unwrap();
    assert_eq!(json["entry_action"], "external_login");
    assert!(!json.to_string().contains("credential-canary"));
    let output = login_entries_interface::PublicLoginEntriesOutput(PublicLoginEntriesResponse {
        default_login_entry_id: Uuid::nil(),
        login_entries: vec![public],
    });
    let schema =
        login_entries_interface::PublicLoginEntriesOutput::managed_projection_schema().unwrap();
    let projection = crate::extension_bus::FrozenManagedProjection::compile(
        extension_contracts::ManagedProjectionContract {
            contract_id: "public-login-entries-output".into(),
            contract_version: "1".into(),
            schema,
        },
    )
    .unwrap();
    let value = output.project_for_managed_hook().unwrap();
    let projected = interface_runtime::ManagedInterfaceProjection::from_contract(&output).unwrap();
    projection.validate_projection(&projected).unwrap();
    assert_eq!(
        value["0"]["login_entries"][0]["entry_action"],
        "external_login"
    );
    assert!(!value.to_string().contains("form canary"));
    let password = to_public_login_entry(entry("password-local", true), &registry).unwrap();
    assert_eq!(password.entry_action, AuthenticationEntryAction::RenderUi);
    assert!(to_public_login_entry(entry("example/renamed", false), &registry).is_none());
    assert!(to_public_login_entry(entry("missing/provider", true), &registry).is_none());
}
