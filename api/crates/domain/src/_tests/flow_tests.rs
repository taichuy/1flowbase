use crate::{
    default_flow_document, default_flow_document_for_application, ApplicationType,
    DefaultUpgradePolicy, DEFAULT_AUTO_INCLUDE_NEW_PROVIDER_INSTANCES,
    DEFAULT_CODE_ISOLATION_TIMEOUT_MS, FLOW_AUTOSAVE_INTERVAL_SECONDS, FLOW_SCHEMA_VERSION,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn start_output_structure_rejects_duplicates_and_malformed_arrays_for_both_flow_families() {
    for node_type in ["start", "workflow_start"] {
        for outputs in [json!([{"key":"name"}]), json!(null), json!({}), json!("[]")] {
            let document = json!({"schemaVersion":FLOW_SCHEMA_VERSION,"graph":{"edges":[],"nodes":[{"id":"entry","type":node_type,"config":{},"bindings":{},"outputs":outputs}]}});
            let error = crate::flow::validate_flow_draft_structure(&document).unwrap_err();
            assert_eq!(error.node_id.as_deref(), Some("entry"));
            assert_eq!(error.expected.as_deref(), Some(&json!([])));
            assert!(error.to_string().contains("outputs must be empty"));
        }
        assert!(crate::flow::validate_flow_draft_structure(
            &json!({"graph":{"nodes":[{"type":node_type}]}})
        )
        .is_err());
        let schema = crate::flow::flow_node_document_schema(node_type).unwrap();
        assert_eq!(schema["properties"]["outputs"]["maxItems"], 0);
        assert_eq!(schema["required"], json!(["outputs"]));
    }
}

#[test]
fn start_output_structure_preserves_incomplete_drafts_and_regular_node_outputs() {
    let document = json!({"schemaVersion":FLOW_SCHEMA_VERSION,"graph":{"nodes":[
        {"id":"entry","type":"workflow_start","outputs":[],"bindings":{},"config":{"input_fields":[{"key":"name"}]}},
        {"id":"unfinished","type":"http_request","config":{},"bindings":{},"outputs":[{"key":"body","title":"Body","valueType":"json"}]}
    ],"edges":[]}});
    crate::flow::validate_flow_draft_structure(&document).unwrap();
    assert!(crate::flow::flow_node_document_schema("http_request").is_none());
}

#[test]
fn document_structure_rejects_invalid_selectors_and_duplicate_keys() {
    let mut document =
        default_flow_document_for_application(ApplicationType::Workflow, Uuid::now_v7());
    document["graph"]["nodes"][1]["outputs"] =
        json!([{"key":"report","title":"Report","valueType":"object","selector":["result",7]}]);
    let error = crate::flow::validate_flow_draft_structure(&document).unwrap_err();
    assert_eq!(error.field_path, "/outputs/0/selector");
    document["graph"]["nodes"][1]["outputs"] = json!([{"key":"report","title":"Report","valueType":"object"},{"key":"report","title":"Duplicate","valueType":"object"}]);
    assert_eq!(
        crate::flow::validate_flow_draft_structure(&document)
            .unwrap_err()
            .field_path,
        "/outputs/1/key"
    );
    assert!(crate::flow::validate_flow_draft_structure(&json!({})).is_err());
}

#[test]
fn default_flow_document_uses_v2_prompt_messages_contract() {
    let document = default_flow_document(Uuid::now_v7());

    assert_eq!(document["schemaVersion"], json!(FLOW_SCHEMA_VERSION));
    assert_eq!(FLOW_SCHEMA_VERSION, "1flowbase.flow/v2");
    assert!(document["graph"]["nodes"][1]["bindings"]["prompt_messages"].is_object());
    assert!(document["graph"]["nodes"][1]["bindings"]["user_prompt"].is_null());
}

#[test]
fn default_flow_document_start_config_includes_input_fields_and_model_list() {
    let document = default_flow_document(Uuid::now_v7());
    let start_config = &document["graph"]["nodes"][0]["config"];

    assert_eq!(start_config["input_fields"], json!([]));
    assert_eq!(start_config["model_list"], json!([]));
}

#[test]
fn default_flow_document_llm_outputs_match_frontend_contract() {
    let document = default_flow_document(Uuid::now_v7());

    assert_eq!(
        document["graph"]["nodes"][1]["outputs"],
        json!([
            { "key": "text", "title": "模型输出", "valueType": "string" },
            { "key": "usage", "title": "用量", "valueType": "json" }
        ])
    );
}

#[test]
fn default_workflow_document_uses_workflow_start_and_end_only() {
    let document = default_flow_document_for_application(ApplicationType::Workflow, Uuid::now_v7());

    assert_eq!(document["schemaVersion"], json!(FLOW_SCHEMA_VERSION));
    assert_eq!(
        document["graph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["workflow_start", "workflow_end"]
    );
    assert_eq!(document["graph"]["edges"].as_array().unwrap().len(), 1);
    assert_eq!(
        document["graph"]["nodes"][0]["config"],
        json!({
            "input_fields": [],
            "sync_timeout_ms": 30000
        })
    );
    assert_eq!(document["graph"]["nodes"][1]["outputs"], json!([]));
}

#[test]
fn default_agent_flow_document_stays_on_agent_flow_start_and_answer() {
    let document =
        default_flow_document_for_application(ApplicationType::AgentFlow, Uuid::now_v7());

    assert_eq!(
        document["graph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["start", "llm", "answer"]
    );
}

#[test]
fn default_upgrade_policy_strings_are_stable() {
    assert_eq!(DefaultUpgradePolicy::CreateOnly.as_str(), "create_only");
    assert_eq!(
        DefaultUpgradePolicy::SystemMaintainedAutoUpdate.as_str(),
        "system_maintained_auto_update"
    );
    assert_eq!(
        DefaultUpgradePolicy::UserConfigurablePreserveHistory.as_str(),
        "user_configurable_preserve_history"
    );
    assert_eq!(
        DefaultUpgradePolicy::ExplicitMigrationOnly.as_str(),
        "explicit_migration_only"
    );
}

#[test]
fn simple_system_default_constants_are_stable() {
    let defaults = (
        FLOW_AUTOSAVE_INTERVAL_SECONDS,
        DEFAULT_AUTO_INCLUDE_NEW_PROVIDER_INSTANCES,
        DEFAULT_CODE_ISOLATION_TIMEOUT_MS,
    );

    assert_eq!(defaults, (30, true, 100));
}

#[test]
fn validated_draft_view_preserves_borrowed_values_and_document_order() {
    let document = json!({"schemaVersion":FLOW_SCHEMA_VERSION,"graph":{
        "nodes":[
            {"id":"second","type":"code","config":{},"bindings":{"input":{"value":7}},"outputs":[{"key":"result","title":"Result","valueType":"json"}]},
            {"id":"first","type":"http_request","config":{},"bindings":{},"outputs":[]}
        ],
        "edges":[{"id":"edge","source":"second","target":"missing"}]
    }});
    let draft = crate::flow::validated_flow_draft_structure(&document).unwrap();
    assert_eq!(
        draft
            .nodes()
            .iter()
            .map(|node| node.id())
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    let node = &draft.nodes()[0];
    assert_eq!(node.node_type(), "code");
    assert!(std::ptr::eq(node.value(), &document["graph"]["nodes"][0]));
    assert!(std::ptr::eq(
        node.outputs(),
        document["graph"]["nodes"][0]["outputs"]
            .as_array()
            .unwrap()
            .as_slice()
    ));
    assert!(std::ptr::eq(
        node.bindings(),
        document["graph"]["nodes"][0]["bindings"]
            .as_object()
            .unwrap()
    ));
    assert_eq!(draft.edges()[0].source(), "second");
    // Structural validation deliberately leaves graph endpoint resolution to the compiler.
    assert_eq!(draft.edges()[0].target(), "missing");
}

#[test]
fn validated_draft_preserves_first_structural_error() {
    let base = json!({"schemaVersion":FLOW_SCHEMA_VERSION,"graph":{
        "nodes":[
            {"id":"one","type":"code","config":{},"bindings":{},"outputs":[]},
            {"id":"two","type":"code","config":{},"bindings":{},"outputs":[]}
        ],
        "edges":[{"id":"edge","source":"one","target":"two"}]
    }});
    let cases = [
        (
            vec![("/graph/nodes", json!(null)), ("/graph/edges", json!(null))],
            "/graph/nodes",
            "graph.nodes must be an array",
        ),
        (
            vec![
                ("/graph/edges", json!(null)),
                ("/schemaVersion", json!(null)),
            ],
            "/graph/edges",
            "graph.edges must be an array",
        ),
        (
            vec![
                ("/schemaVersion", json!(null)),
                ("/graph/nodes/0/id", json!(null)),
            ],
            "/schemaVersion",
            "unsupported or missing flow schemaVersion",
        ),
        (
            vec![
                ("/graph/nodes/0/type", json!("start")),
                ("/graph/nodes/0/id", json!(null)),
                ("/graph/nodes/0/outputs", json!(null)),
            ],
            "/outputs",
            "start node (unnamed) outputs must be empty",
        ),
        (
            vec![
                ("/graph/nodes/0/id", json!(" ")),
                ("/graph/nodes/0/type", json!(null)),
            ],
            "/id",
            "node id and type must be nonempty strings",
        ),
        (
            vec![
                ("/graph/nodes/1/id", json!("one")),
                ("/graph/nodes/1/config", json!(null)),
            ],
            "/id",
            "node ids must be unique",
        ),
        (
            vec![
                ("/graph/nodes/0/config", json!(null)),
                ("/graph/nodes/0/bindings", json!(null)),
            ],
            "/config",
            "node config and bindings must be objects",
        ),
        (
            vec![
                ("/graph/nodes/0/bindings", json!(null)),
                ("/graph/nodes/0/outputs", json!(null)),
            ],
            "/bindings",
            "node config and bindings must be objects",
        ),
        (
            vec![(
                "/graph/nodes/0/outputs",
                json!([{"key":"__internal","title":" ","valueType":null}]),
            )],
            "/outputs/0/title",
            "output key, title and valueType must be nonempty strings",
        ),
        (
            vec![
                ("/graph/edges/0/source", json!(null)),
                ("/graph/edges/0/target", json!(null)),
            ],
            "/graph/edges",
            "edge id, source and target must be nonempty strings",
        ),
    ];
    for (changes, path, message) in cases {
        let mut document = base.clone();
        for (pointer, value) in changes {
            *document.pointer_mut(pointer).unwrap() = value;
        }
        let original_entry = crate::flow::validate_flow_draft_structure(&document).unwrap_err();
        let typed_entry = crate::flow::validated_flow_draft_structure(&document).unwrap_err();
        assert_eq!(original_entry, typed_entry);
        assert_eq!(typed_entry.field_path, path);
        assert_eq!(typed_entry.message, message);
    }
}
