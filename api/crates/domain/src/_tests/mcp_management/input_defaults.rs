use super::*;
use serde_json::json;

#[test]
fn defaults_preserve_falsy_values_and_hidden_values_cannot_be_overridden() {
    for value in [json!(0), json!(false), json!([]), json!({}), json!("")] {
        let hidden = McpInputDefaults {
            default_value: Some(value.clone()),
            hidden: true,
        };
        assert_eq!(hidden.resolve(Some(&json!("attacker"))), Some(&value));
        let visible = McpInputDefaults {
            hidden: false,
            ..hidden
        };
        assert_eq!(visible.resolve(None), Some(&value));
        assert_eq!(visible.resolve(Some(&Value::Null)), Some(&Value::Null));
    }
    let historical: McpInputDefaults = serde_json::from_value(json!({})).unwrap();
    assert!(!historical.hidden);
    assert_eq!(historical.resolve(None), None);
}

#[test]
fn hidden_required_defaults_are_validated_and_legacy_projection_is_explicit() {
    let mapping = json!({"mappings":[{"interface_param":"name","required":true,"hidden":true}]});
    assert_eq!(validate_input_defaults(&mapping, &[]), Err("input_mapping"));
    let legacy = json!({"mappings":[{"interface_param":"name","required":true}]});
    assert!(validate_input_defaults(&legacy, &[]).is_ok());
    let projected = input_mapping_with_defaults(legacy.clone());
    assert_eq!(projected["mappings"][0]["hidden"], false);
    assert!(projected["mappings"][0]["default_value"].is_null());
    assert!(legacy["mappings"][0].get("hidden").is_none());
}

#[test]
fn call_controls_use_configured_defaults_without_exposing_hidden_controls() {
    let mapping = json!({
        "interface_parameters":[{"source":{"kind":"mcp_call"}}],
        "mappings":[
            {"interface_param":"max_inline_chars","mcp_param":"max_inline_chars","source":{"kind":"mcp_call","path":"max_inline_chars"},"hidden":true,"default_value":500},
            {"interface_param":"response_fields","mcp_param":"response_fields","source":{"kind":"mcp_call","path":"response_fields"},"default_value":[]}
        ]
    });
    assert!(!crate::mcp_management::mcp_call_parameter_is_open(
        &mapping,
        "max_inline_chars"
    ));
    assert!(crate::mcp_management::mcp_call_parameter_is_open(
        &mapping,
        "response_fields"
    ));
    assert_eq!(
        resolve_call_defaults(&json!({"max_inline_chars":10000}), &mapping),
        json!({"max_inline_chars":500,"response_fields":[]})
    );
    assert!(validate_input_defaults(&mapping, &[]).is_ok());
    let mut invalid = mapping;
    invalid["mappings"][0]["default_value"] = json!(0);
    assert_eq!(validate_input_defaults(&invalid, &[]), Err("input_mapping"));
}

#[test]
fn defaults_are_checked_against_canonical_parameter_types() {
    let parameter = crate::mcp_management::McpParameterDescriptor {
        name: "enabled".into(),
        field_type: "boolean".into(),
        parameter_type: crate::mcp_management::McpParameterType::JsonBody,
        description: None,
        required: true,
        schema: json!({"type":"boolean"}),
    };
    let mut mapping =
        json!({"mappings":[{"interface_param":"enabled","hidden":true,"default_value":false}]});
    assert!(validate_input_defaults(&mapping, std::slice::from_ref(&parameter)).is_ok());
    mapping["mappings"][0]["default_value"] = json!("false");
    assert_eq!(
        validate_input_defaults(&mapping, std::slice::from_ref(&parameter)),
        Err("input_mapping")
    );
    mapping["mappings"][0]["default_value"] = Value::Null;
    assert_eq!(
        validate_input_defaults(&mapping, &[parameter]),
        Err("input_mapping")
    );
}
