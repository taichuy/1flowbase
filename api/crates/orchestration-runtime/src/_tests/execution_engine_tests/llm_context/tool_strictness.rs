use super::*;

#[test]
fn provider_tool_strictness_projects_explicit_bools_without_changing_schema() {
    for strict in [Some(false), Some(true), None] {
        let schema = json!({
            "type": "object",
            "properties": {
                "value": {"type": "string"},
                "sandbox_permissions": {"type": "string"},
                "justification": {"type": "string"}
            },
            "required": ["value"],
            "additionalProperties": false
        });
        let mut native = json!({
            "name": "probe", "description": "Echo value",
            "input_schema": schema, "source": "client"
        });
        let mut expected = json!({
            "type": "function",
            "function": {"name": "probe", "description": "Echo value", "parameters": schema}
        });
        if let Some(strict) = strict {
            native["strict"] = json!(strict);
            expected["function"]["strict"] = json!(strict);
        }

        assert_eq!(provider_tool_payload(&native), expected);
    }
}

#[test]
fn provider_tool_strictness_keeps_existing_function_wrappers_unchanged() {
    for strict in [Some(false), Some(true), None] {
        let mut wrapped = json!({
            "type": "function",
            "function": {
                "name": "probe", "description": " Echo value ",
                "parameters": {"type": "object", "required": ["value"]},
                "extension": {"preserve": true}
            }
        });
        if let Some(strict) = strict {
            wrapped["function"]["strict"] = json!(strict);
        }

        assert_eq!(provider_tool_payload(&wrapped), wrapped);
    }
}
