use serde_json::json;

use super::compiled_plan_has_mcp_bindings;

#[test]
fn compiled_plan_without_mcp_bindings_skips_catalog() {
    let plan = json!({"nodes": {"llm": {"config": {"model": "test"}}}});
    assert_eq!(compiled_plan_has_mcp_bindings(&plan), Some(false));
}

#[test]
fn compiled_plan_with_mcp_bindings_keeps_catalog() {
    let plan = json!({"nodes": {"llm": {"config": {"mcp_instance_ids": ["instance"]}}}});
    assert_eq!(compiled_plan_has_mcp_bindings(&plan), Some(true));
}

#[test]
fn unknown_compiled_plan_keeps_catalog() {
    let plan = json!({"nodes": {"llm": {"config": {"mcp_instance_ids": 1}}}});
    assert_eq!(compiled_plan_has_mcp_bindings(&plan), None);
}
