use crate::*;
use serde_json::json;
#[test]
fn credential_requests_reject_worker_asserted_identity() {
    for identity in [
        "plugin_code",
        "contribution_id",
        "publisher_namespace",
        "installation_id",
        "scope_id",
        "workspace_id",
        "plugin_version",
    ] {
        for operation in ["put", "get", "delete"] {
            let mut request = json!({"operation":operation, "credential_id":"server/password"});
            if operation == "put" {
                request["value"] = json!({"password":"fixture-only"});
            }
            request[identity] = json!("forged");
            assert!(serde_json::from_value::<PluginCredentialRequest>(request).is_err());
        }
    }
}
#[test]
fn credential_wire_supports_only_named_operations() {
    for operation in ["put", "get", "delete"] {
        let mut value = json!({"operation": operation, "credential_id":"server/password"});
        if operation == "put" {
            value["value"] = json!({"password":"fixture-only"});
        }
        let request: PluginCredentialRequest = serde_json::from_value(value.clone()).unwrap();
        request.validate().unwrap();
        assert_eq!(serde_json::to_value(request).unwrap(), value);
    }
    assert!(
        serde_json::from_value::<PluginCredentialRequest>(json!({"operation":"list"})).is_err()
    );
    assert!(PluginCredentialRequest::Get {
        credential_id: "".into()
    }
    .validate()
    .is_err());
}
