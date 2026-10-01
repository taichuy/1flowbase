use crate::mcp_management::{
    is_mcp_result_pointer, mcp_return_control_properties, validate_mcp_return_defaults,
};

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
    assert!(properties.contains_key("string_ranges"));
}
