use super::*;
use serde_json::json;

#[test]
fn dispatch_uses_hidden_defaults_and_visible_fallbacks_in_real_target_locations() {
    let interface = domain::McpInterfaceCatalogEntry {
        interface_id: "defaults".into(),
        source: domain::mcp_management::McpInterfaceCatalogSource::StaticApi,
        method: "POST".into(),
        path: "/records/{id}".into(),
        name: "Defaults".into(),
        short_description: String::new(),
        parameter_descriptors: vec![],
        parameter_schema: json!({"type":"object","properties":{
            "path":{"type":"object","properties":{"id":{"type":"string"}}},
            "query":{"type":"object","properties":{"count":{"type":"integer"}}},
            "body":{"type":"object","properties":{"enabled":{"type":"boolean"},"title":{"type":"string"},"optional":{"type":"string"},"config":{"type":"object","properties":{"secret":{"type":"string"}}}}}
        }}),
        result_schema: json!({}),
        permission_code: None,
        security: json!({}),
        risk_level: domain::mcp_management::McpRiskLevel::Low,
        bindable: true,
        disabled_reason: None,
    };
    let mapping = json!({"mappings":[
        {"interface_param":"config.secret","mcp_param":"config.secret","hidden":true,"default_value":"fixed"},
        {"interface_param":"config","mcp_param":"config"},
        {"interface_param":"id","mcp_param":"id","required":true,"hidden":true,"default_value":"fixed"},
        {"interface_param":"count","mcp_param":"count","required":true,"default_value":0},
        {"interface_param":"enabled","source":{"kind":"mcp_argument","path":"body.enabled"},"hidden":true,"default_value":false},
        {"interface_param":"title","mcp_param":"title","default_value":"fallback"},
        {"interface_param":"optional","mcp_param":"optional","hidden":true}
    ]});
    let result = build_interface_arguments(
        &interface,
        &mapping,
        &json!({"id":"override","body":{"enabled":true},"title":"supplied","optional":"override","config":{"secret":"override"}}),
        McpServerBoundInputs {
            workspace_id: Uuid::nil(),
        },
    )
    .unwrap();
    assert_eq!(
        result.path,
        serde_json::from_value::<Map<String, Value>>(json!({"id":"fixed"})).unwrap()
    );
    assert_eq!(result.query["count"], 0);
    assert_eq!(
        result.body,
        serde_json::from_value::<Map<String, Value>>(
            json!({"enabled":false,"title":"supplied","config":{"secret":"fixed"}})
        )
        .unwrap()
    );
}
