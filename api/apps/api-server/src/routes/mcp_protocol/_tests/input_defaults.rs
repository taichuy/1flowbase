use super::*;
use serde_json::json;

#[test]
fn hidden_parameters_never_reappear_when_all_mappings_are_hidden() {
    let schema =
        json!({"type":"object","properties":{"secret":{"type":"string"}},"required":["secret"]});
    let mapping = json!({"mappings":[{"interface_param":"secret","mcp_param":"secret","required":true,"hidden":true,"default_value":"fixed"}]});
    let result = mapped_schema(&schema, &mapping);
    assert_eq!(result["properties"], json!({}));
    assert!(result.get("required").is_none());
}

#[test]
fn visible_fallback_is_optional_and_historical_required_parameter_stays_required() {
    let schema = json!({"type":"object","properties":{"count":{"type":"integer"},"title":{"type":"string"}}});
    let mapping = json!({"mappings":[
        {"interface_param":"count","mcp_param":"body.count","required":true,"default_value":0},
        {"interface_param":"title","mcp_param":"body.title","required":true}
    ]});
    let result = mapped_schema(&schema, &mapping);
    assert_eq!(result["properties"]["body"]["required"], json!(["title"]));
    assert!(result["properties"]["body"]["properties"]
        .get("count")
        .is_some());
}

#[test]
fn visible_object_schema_does_not_reexpose_hidden_descendant() {
    let schema = json!({"type":"object","properties":{"config":{"type":"object","properties":{"secret":{"type":"string"},"name":{"type":"string"}},"required":["secret","name"]}}});
    let mapping = json!({"mappings":[
        {"interface_param":"config.secret","mcp_param":"config.secret","hidden":true,"default_value":"fixed"},
        {"interface_param":"config","mcp_param":"config"}
    ]});
    let result = mapped_schema(&schema, &mapping);
    assert!(result["properties"]["config"]["properties"]
        .get("secret")
        .is_none());
    assert_eq!(result["properties"]["config"]["required"], json!(["name"]));
}
