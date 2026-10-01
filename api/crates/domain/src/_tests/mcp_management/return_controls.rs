use crate::mcp_management::{
    is_mcp_result_pointer, mcp_call_parameter_is_open, mcp_return_control_properties,
    validate_mcp_call_parameters, validate_mcp_return_defaults,
};
use serde_json::json;

#[test]
fn configured_call_parameters_are_an_explicit_allowlist() {
    let legacy = json!({});
    assert!(mcp_call_parameter_is_open(&legacy, "max_inline_chars"));
    let legacy_selected = json!({"call_parameters":["response_fields"]});
    assert!(mcp_call_parameter_is_open(
        &legacy_selected,
        "response_fields"
    ));
    assert!(!mcp_call_parameter_is_open(&legacy_selected, "des_id"));
    let interface_parameters = json!([
        {"name":"des_id","required":false,"source":{"kind":"mcp_call"}},
        {"name":"max_inline_chars","required":false,"source":{"kind":"mcp_call"}},
        {"name":"response_fields","required":false,"source":{"kind":"mcp_call"}}
    ]);
    let closed = json!({"interface_parameters":interface_parameters,"mappings":[]});
    assert!(!mcp_call_parameter_is_open(&closed, "des_id"));
    assert!(!mcp_call_parameter_is_open(&closed, "response_fields"));
    let selected = json!({
        "interface_parameters":interface_parameters,
        "mappings":[{"interface_param":"response_fields","mcp_param":"response_fields","required":false,"source":{"kind":"mcp_call","path":"response_fields"}}]
    });
    assert!(mcp_call_parameter_is_open(&selected, "response_fields"));
    assert!(!mcp_call_parameter_is_open(&selected, "max_inline_chars"));
    assert!(validate_mcp_call_parameters(&selected).is_ok());
    for invalid in [
        json!({"interface_parameters":interface_parameters,"mappings":[{"interface_param":"max_inline_chars","mcp_param":"budget","required":false,"source":{"kind":"mcp_call","path":"max_inline_chars"}}]}),
        json!({"interface_parameters":interface_parameters,"mappings":[{"interface_param":"des_id","mcp_param":"des_id","required":true,"source":{"kind":"mcp_call","path":"des_id"}}]}),
        json!({"mappings":[{"interface_param":"unknown","mcp_param":"unknown","required":false,"source":{"kind":"mcp_call","path":"unknown"}}]}),
    ] {
        assert!(validate_mcp_call_parameters(&invalid).is_err());
    }
}

#[test]
fn return_defaults_accept_positive_budgets_without_a_fixed_ceiling() {
    assert!(validate_mcp_return_defaults(None, None).is_ok());
    assert!(validate_mcp_return_defaults(Some(80_000), Some(&[])).is_ok());
    assert!(validate_mcp_return_defaults(Some(i64::MAX), None).is_ok());
    assert!(validate_mcp_return_defaults(Some(0), None).is_err());
    assert!(validate_mcp_return_defaults(Some(-1), None).is_err());
}

#[test]
fn field_paths_use_json_pointer_and_validate_escaping() {
    for pointer in ["", "/title", "/items/0/body", "/a~1b/~0"] {
        assert!(is_mcp_result_pointer(pointer));
    }
    for pointer in ["title", "body.text", "/bad~", "/bad~2"] {
        assert!(!is_mcp_result_pointer(pointer));
    }
}

#[test]
fn discovery_controls_have_no_upper_budget_or_unlimited_mode() {
    let properties = mcp_return_control_properties();
    assert_eq!(properties["max_inline_chars"]["minimum"], 1);
    assert!(properties["max_inline_chars"].get("maximum").is_none());
    assert!(properties["max_inline_chars"].get("default").is_none());
    assert!(properties.contains_key("response_fields"));
    assert_eq!(
        properties["response_fields"]["items"]["pattern"],
        "^(?:|/(?:[^~]|~[01])*)$"
    );
    assert!(properties.contains_key("string_ranges"));
}
