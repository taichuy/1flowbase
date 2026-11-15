use crate::*;
use serde_json::json;
#[test]
fn ephemeral_requests_reject_worker_asserted_identity() {
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
            let mut request = json!({"operation":operation, "key":"server/password"});
            if operation == "put" {
                request["ttl_ms"] = json!(60000);
                request["value"] = json!({"password":"fixture-only"});
            }
            request[identity] = json!("forged");
            assert!(serde_json::from_value::<PluginEphemeralRequest>(request).is_err());
        }
    }
}
#[test]
fn ephemeral_wire_supports_only_named_operations() {
    for operation in ["put", "get", "delete"] {
        let mut value = json!({"operation": operation, "key":"server/password"});
        if operation == "put" {
            value["ttl_ms"] = json!(60000);
            value["value"] = json!({"password":"fixture-only"});
        }
        let request: PluginEphemeralRequest = serde_json::from_value(value.clone()).unwrap();
        request.validate().unwrap();
        assert_eq!(serde_json::to_value(request).unwrap(), value);
    }
    assert!(serde_json::from_value::<PluginEphemeralRequest>(json!({"operation":"list"})).is_err());
    assert!(PluginEphemeralRequest::Get { key: "".into() }
        .validate()
        .is_err());
}

#[test]
fn ephemeral_put_requires_positive_integer_ttl() {
    for ttl in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!(null),
        json!("infinity"),
    ] {
        let parsed = serde_json::from_value::<PluginEphemeralRequest>(json!({
            "operation":"put", "key":"token", "value":"fixture", "ttl_ms":ttl
        }));
        assert!(parsed.is_err() || parsed.unwrap().validate().is_err());
    }
    assert!(serde_json::from_value::<PluginEphemeralRequest>(json!({
        "operation":"put", "key":"token", "value":"fixture"
    }))
    .is_err());
    let response = PluginEphemeralResponse {
        value: Some(json!({"token":"fixture"})),
    };
    assert_eq!(
        serde_json::from_value::<PluginEphemeralResponse>(serde_json::to_value(&response).unwrap())
            .unwrap()
            .value,
        response.value
    );
}

#[test]
fn ephemeral_response_preserves_stored_json_null_separately_from_missing() {
    for value in [
        None,
        Some(serde_json::Value::Null),
        Some(json!({"token":"fixture"})),
    ] {
        let encoded = serde_json::to_value(PluginEphemeralResponse {
            value: value.clone(),
        })
        .unwrap();
        assert_eq!(encoded.get("value").is_some(), value.is_some());
        let decoded: PluginEphemeralResponse = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.value, value);
    }
}
