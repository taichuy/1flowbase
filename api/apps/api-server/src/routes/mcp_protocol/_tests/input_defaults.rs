use super::*;

#[test]
fn declared_body_fields_do_not_inherit_transport_container_schemas() {
    let source = json!({"type":"object","properties":{
        "path":{"type":"object","properties":{"host_id":{"type":"string"}}},
        "query":{"type":"object","properties":{"page":{"type":"integer"}}},
        "body":{"type":"object","properties":{
            "path":{"type":"string","minLength":1},
            "query":{"type":"string","maxLength":20},
            "body":{"type":"string","enum":["payload"]}
        }}
    }});
    for parameter in ["path", "query", "body"] {
        let mapping = json!({
            "interface_parameters":[{"name":parameter,"parameter_type":"json_body"}],
            "mappings":[{"interface_param":parameter,"mcp_param":parameter,"required":true}]
        });
        let actual = mapped_schema(&source, &mapping);
        assert_eq!(
            actual["properties"][parameter],
            source["properties"]["body"]["properties"][parameter]
        );
    }
}

#[test]
fn declared_url_location_and_business_alias_keep_the_source_contract() {
    let source = json!({"type":"object","properties":{
        "path":{"type":"object","properties":{"host_id":{"type":"string"}}},
        "query":{"type":"object","properties":{"path":{"type":"integer","minimum":1}}},
        "title":{"type":"boolean"},
        "body":{"type":"object","properties":{"amount":{"type":"integer","minimum":0}}}
    }});
    let mapping = json!({
        "interface_parameters":[{"name":"path","parameter_type":"url"},{"name":"amount","parameter_type":"form"}],
        "mappings":[
            {"interface_param":"path","mcp_param":"path","required":true},
            {"interface_param":"amount","mcp_param":"title","required":true}
        ]
    });
    let actual = mapped_schema(&source, &mapping);
    assert_eq!(
        actual["properties"]["path"],
        source["properties"]["query"]["properties"]["path"]
    );
    assert_eq!(
        actual["properties"]["title"],
        source["properties"]["body"]["properties"]["amount"]
    );
}

#[test]
fn mappings_without_location_hints_preserve_the_legacy_flat_schema() {
    let source = json!({"type":"object","properties":{
        "path":{"type":"string","enum":["flat"]},
        "body":{"type":"object","properties":{"path":{"type":"integer"}}}
    }});
    let actual = mapped_schema(
        &source,
        &json!({"mappings":[{"interface_param":"path","mcp_param":"path","required":true}]}),
    );
    assert_eq!(actual["properties"]["path"], source["properties"]["path"]);
}
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
