use super::tool_result;
use serde_json::{json, Value};

#[test]
fn structured_tool_results_wrap_non_object_values_without_data_loss() {
    for value in [
        json!([{"id": "tool", "risk_level": "high"}]),
        json!([]),
        json!("hello"),
        json!(42),
        json!(false),
        Value::Null,
    ] {
        let delivered = tool_result(value.clone());
        assert!(delivered["structuredContent"].is_object());
        assert_eq!(delivered["structuredContent"], json!({"result": value}));
        assert_eq!(
            serde_json::from_str::<Value>(delivered["content"][0]["text"].as_str().unwrap())
                .unwrap(),
            delivered["structuredContent"]
        );
        assert_eq!(delivered["isError"], false);
    }
}

#[test]
fn structured_tool_results_preserve_object_fields_and_continuation_paths() {
    for value in [
        json!({"input_schema": {"type": "object"}, "result": [1, 2]}),
        json!({"entries": [{"path": "/0/id", "value": "tool"}], "next_cursor": "cursor"}),
        json!({"detail": {"result_ref": "ref", "next_cursor": "cursor"}}),
        json!({}),
    ] {
        let delivered = tool_result(value.clone());
        assert_eq!(delivered["structuredContent"], value);
        assert_eq!(
            serde_json::from_str::<Value>(delivered["content"][0]["text"].as_str().unwrap())
                .unwrap(),
            value
        );
    }
}
