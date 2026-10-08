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
        json!({"interface_parameters":interface_parameters,"mappings":[{"interface_param":"max_inline_chars","mcp_param":"max_inline_chars","required":true,"source":{"kind":"mcp_call","path":"max_inline_chars"}}]}),
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

fn description_mapping(tool_id: Option<&str>) -> serde_json::Value {
    let mut mapping = json!({
        "interface_parameters": [
            {"name":"des_id","required":false,"source":{"kind":"mcp_call"}},
            {"name":"max_inline_chars","required":false,"source":{"kind":"mcp_call"}},
            {"name":"response_fields","required":false,"source":{"kind":"mcp_call"}}
        ],
        "mappings": [{"interface_param":"des_id","mcp_param":"des_id","required":true,"source":{"kind":"mcp_call","path":"des_id"}}]
    });
    if let Some(id) = tool_id {
        mapping["mappings"][0]["source"]["tool_id"] = json!(id);
    }
    mapping
}

#[test]
fn description_acknowledgement_is_derived_from_mapping() {
    for target in [None, Some("prerequisite")] {
        let mapping = description_mapping(target);
        assert!(validate_mcp_call_parameters(&mapping).is_ok());
        assert!(crate::mcp_management::mcp_des_id_required(&mapping));
        assert_eq!(
            crate::mcp_management::mcp_description_tool_id(&mapping),
            target
        );
    }
    assert!(!crate::mcp_management::mcp_des_id_required(&json!({})));
}

#[test]
fn description_guard_rejects_bypass_and_invalid_reference_configuration() {
    let valid = description_mapping(Some("prerequisite"));
    for (pointer, value) in [
        ("/mappings/0/source/tool_id", json!("")),
        ("/mappings/0/source/tool_id", json!("   ")),
        ("/mappings/0/source/tool_id", json!(null)),
        ("/mappings/0/default_value", json!("token")),
        ("/mappings/0/hidden", json!(true)),
        ("/mappings/0/required", json!(false)),
        ("/mappings/0/interface_param", json!("response_fields")),
        ("/mappings/0/source/kind", json!("mcp_argument")),
    ] {
        let mut invalid = valid.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        invalid.pointer_mut(parent).unwrap()[key] = value;
        assert!(validate_mcp_call_parameters(&invalid).is_err(), "{pointer}");
    }
    let mut duplicate = valid.clone();
    duplicate["mappings"]
        .as_array_mut()
        .unwrap()
        .push(valid["mappings"][0].clone());
    assert!(validate_mcp_call_parameters(&duplicate).is_err());
    let mut other = description_mapping(None);
    other["mappings"][0]["interface_param"] = json!("max_inline_chars");
    other["mappings"][0]["mcp_param"] = json!("max_inline_chars");
    other["mappings"][0]["source"]["path"] = json!("max_inline_chars");
    assert!(validate_mcp_call_parameters(&other).is_err());
}
