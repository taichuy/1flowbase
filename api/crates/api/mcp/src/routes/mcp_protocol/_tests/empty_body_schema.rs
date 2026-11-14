use super::*;
use domain::McpToolExecutionTarget;

fn wrapper() -> McpToolExecutionTarget {
    McpToolExecutionTarget::InterfaceWrapper {
        interface_id: "terminal.info".into(),
    }
}
fn empty_body() -> Value {
    json!({"type":"object","properties":{"body":{"type":"object","properties":{},"additionalProperties":false}},"required":["body"],"additionalProperties":false})
}

#[test]
fn explicitly_unmapped_required_empty_body_has_no_business_parameters() {
    let mapping = json!({"interface_parameters":[],"mappings":[]});
    let mut schema = empty_body();
    for omit_properties in [false, true] {
        if omit_properties {
            schema["properties"]["body"]
                .as_object_mut()
                .unwrap()
                .remove("properties");
        }
        schema["properties"]["body"]["description"] = json!("empty transport body");
        let projected = tool_mapped_schema(&wrapper(), &schema, &mapping);
        assert_eq!(
            projected,
            json!({"type":"object","properties":{},"additionalProperties":false})
        );
        assert!(projected.get("required").is_none());
        assert_eq!(
            mapped_schema(&schema, &mapping),
            schema,
            "generic mapping contract stays unchanged"
        );
    }
}

#[test]
fn raw_and_other_tool_targets_keep_even_empty_body_schema() {
    let schema = empty_body();
    for mapping in [json!({}), json!({"mappings":null}), json!({"mappings":{}})] {
        assert_eq!(tool_mapped_schema(&wrapper(), &schema, &mapping), schema);
    }
    for target in [
        McpToolExecutionTarget::McpProxy {
            upstream_connection_id: uuid::Uuid::nil(),
            remote_tool_name: "empty".into(),
            source_schema_hash: "revision".into(),
        },
        McpToolExecutionTarget::AssistantClient {
            capability_code: "file.read".into(),
        },
    ] {
        assert_eq!(
            tool_mapped_schema(&target, &schema, &json!({"mappings":[]})),
            schema
        );
    }
}

#[test]
fn scalar_business_fields_open_objects_and_other_locations_keep_raw_schema() {
    let mapping = json!({"mappings":[]});
    for body in [
        json!({"type":"string"}),
        json!({"type":"object","properties":{"query":{"type":"string"}},"additionalProperties":false}),
        json!({"type":"object","properties":{}}),
        json!({"type":"object","properties":{},"additionalProperties":true}),
        json!({"type":"object","properties":{},"additionalProperties":false,"minProperties":1}),
        json!({"type":"object","properties":{},"additionalProperties":false,"required":["missing"]}),
        json!({"type":"object","properties":{},"additionalProperties":false,"allOf":[false]}),
    ] {
        let mut schema = empty_body();
        schema["properties"]["body"] = body;
        assert_eq!(tool_mapped_schema(&wrapper(), &schema, &mapping), schema);
    }
    for location in ["path", "query"] {
        let mut schema = empty_body();
        schema["properties"][location] =
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]});
        assert_eq!(tool_mapped_schema(&wrapper(), &schema, &mapping), schema);
    }
    let mut optional = empty_body();
    optional.as_object_mut().unwrap().remove("required");
    assert_eq!(
        tool_mapped_schema(&wrapper(), &optional, &mapping),
        optional
    );
    let mut constrained = empty_body();
    constrained["not"] = json!({});
    assert_eq!(
        tool_mapped_schema(&wrapper(), &constrained, &mapping),
        constrained
    );
}

#[test]
fn configured_scalar_body_and_path_parameters_continue_through_normal_mapping() {
    let schema = json!({"type":"object","properties":{"body":{"type":"string"},"path":{"type":"object","properties":{"id":{"type":"string"}}}}});
    let mapping = json!({"mappings":[
        {"interface_param":"body","mcp_param":"body","required":true},
        {"interface_param":"id","mcp_param":"path.id","required":true}
    ]});
    let projected = tool_mapped_schema(&wrapper(), &schema, &mapping);
    assert_eq!(projected["properties"]["body"]["type"], "string");
    assert_eq!(
        projected["properties"]["path"]["properties"]["id"]["type"],
        "string"
    );
    assert_eq!(projected, mapped_schema(&schema, &mapping));
}
