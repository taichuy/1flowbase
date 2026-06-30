use control_plane::flow::{
    AgentFlowTemplateResourceSnapshot, FlowService, ImportAgentFlowTemplateCommand,
    PreviewAgentFlowTemplateCommand, SaveFlowDraftCommand,
};
use domain::{ApplicationType, FlowChangeKind};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn get_or_create_editor_state_requires_visible_application() {
    let service = FlowService::for_tests_with_permissions(vec![
        "application.view.own",
        "application.create.all",
    ]);
    let owner_id = Uuid::now_v7();
    let other_actor_id = Uuid::now_v7();
    let application = service
        .seed_application_for_actor(owner_id, "Support Agent")
        .await
        .unwrap();

    let error = service
        .get_or_create_editor_state(other_actor_id, application.id)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn get_or_create_editor_state_bootstraps_start_node_without_outputs() {
    let owner_id = Uuid::now_v7();
    let service = FlowService::for_tests();
    let application = service
        .seed_application_for_actor(owner_id, "Support Agent")
        .await
        .unwrap();

    let state = service
        .get_or_create_editor_state(owner_id, application.id)
        .await
        .unwrap();
    let start_node = state.draft.document["graph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["type"] == "start")
        .expect("default draft should include a start node");

    assert_eq!(start_node["outputs"], json!([]));
    assert_eq!(start_node["config"]["input_fields"], json!([]));
}

#[tokio::test]
async fn get_or_create_editor_state_bootstraps_workflow_start_and_end_nodes() {
    let owner_id = Uuid::now_v7();
    let service = FlowService::for_tests();
    let application = service
        .seed_application_with_type_for_actor(
            owner_id,
            "Ticket Workflow",
            ApplicationType::Workflow,
        )
        .await
        .unwrap();

    let state = service
        .get_or_create_editor_state(owner_id, application.id)
        .await
        .unwrap();

    assert_eq!(
        state.draft.document["graph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["workflow_start", "workflow_end"]
    );
    assert_eq!(
        state.draft.document["graph"]["nodes"][0]["config"],
        json!({
            "input_fields": [],
            "sync_timeout_ms": 30000
        })
    );
}

#[tokio::test]
async fn save_draft_only_appends_history_for_logical_changes() {
    let owner_id = Uuid::now_v7();
    let service = FlowService::for_tests();
    let application = service
        .seed_application_for_actor(owner_id, "Support Agent")
        .await
        .unwrap();
    let initial = service
        .get_or_create_editor_state(owner_id, application.id)
        .await
        .unwrap();
    let mut layout_only = initial.draft.document.clone();
    layout_only["editor"]["viewport"] = json!({ "x": 240, "y": 32, "zoom": 0.8 });

    let layout_state = service
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: owner_id,
            application_id: application.id,
            document: layout_only,
            change_kind: FlowChangeKind::Layout,
            summary: "viewport update".into(),
        })
        .await
        .unwrap();

    assert_eq!(layout_state.versions.len(), 1);

    let mut logical_change = layout_state.draft.document.clone();
    logical_change["graph"]["nodes"][1]["bindings"]["prompt_messages"]["value"][0]["content"]
        ["value"] = json!("You are a support agent.");

    let logical_state = service
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: owner_id,
            application_id: application.id,
            document: logical_change,
            change_kind: FlowChangeKind::Logical,
            summary: "update llm prompt".into(),
        })
        .await
        .unwrap();

    assert_eq!(logical_state.versions.len(), 2);
    assert_eq!(logical_state.versions[1].summary, "update llm prompt");
}

#[tokio::test]
async fn save_draft_rejects_invalid_visible_internal_llm_tool_identifiers() {
    let owner_id = Uuid::now_v7();
    let service = FlowService::for_tests();
    let application = service
        .seed_application_for_actor(owner_id, "Support Agent")
        .await
        .unwrap();
    let initial = service
        .get_or_create_editor_state(owner_id, application.id)
        .await
        .unwrap();
    let mut document = initial.draft.document.clone();

    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "查看图片",
            "connector_id": "image-llm",
            "target_node_id": "node-answer"
        }
    ]);

    let error = service
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: owner_id,
            application_id: application.id,
            document,
            change_kind: FlowChangeKind::Logical,
            summary: "add mounted tool".into(),
        })
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "invalid input: visible_internal_llm_tools.tool_name"
    );

    let mut document = initial.draft.document.clone();

    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools_enabled"] = json!(true);
    document["graph"]["nodes"][1]["config"]["visible_internal_llm_tools"] = json!([
        {
            "type": "visible_internal_llm_tool",
            "tool_name": "image_llm",
            "connector_id": "image-llm",
            "target_node_id": "node-answer"
        }
    ]);

    let error = service
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: owner_id,
            application_id: application.id,
            document,
            change_kind: FlowChangeKind::Logical,
            summary: "add mounted tool".into(),
        })
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "invalid input: visible_internal_llm_tools.connector_id"
    );
}

#[tokio::test]
async fn export_agent_flow_template_omits_secret_fields_and_collects_dependencies() {
    let owner_id = Uuid::now_v7();
    let service = FlowService::for_tests();
    let application = service
        .seed_application_for_actor(owner_id, "Support Agent")
        .await
        .unwrap();
    let initial = service
        .get_or_create_editor_state(owner_id, application.id)
        .await
        .unwrap();
    let mut document = initial.draft.document.clone();
    document["graph"]["nodes"][1]["config"]["model_provider"] = json!({
        "provider_code": "fixture_provider",
        "source_instance_id": "runtime-instance",
        "model_id": "gpt-4o-mini",
        "api_key": "sk-should-not-leak"
    });

    service
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: owner_id,
            application_id: application.id,
            document,
            change_kind: FlowChangeKind::Logical,
            summary: "bind model".into(),
        })
        .await
        .unwrap();

    let template = service
        .export_agent_flow_template(owner_id, application.id)
        .await
        .unwrap();
    let serialized = serde_json::to_string(&template).unwrap();

    assert_eq!(template.schema_version, "1flowbase.application-template/v1");
    assert_eq!(template.application.application_type, "agent_flow");
    assert!(template.dependencies.iter().any(|dependency| {
        dependency.kind == "model_provider"
            && dependency.provider_code.as_deref() == Some("fixture_provider")
            && dependency.model_id.as_deref() == Some("gpt-4o-mini")
    }));
    assert!(!serialized.contains("sk-should-not-leak"));
}

#[tokio::test]
async fn preview_agent_flow_template_keeps_llm_node_when_model_provider_is_missing() {
    let owner_id = Uuid::now_v7();
    let service = FlowService::for_tests();
    let application = service
        .seed_application_for_actor(owner_id, "Support Agent")
        .await
        .unwrap();
    let initial = service
        .get_or_create_editor_state(owner_id, application.id)
        .await
        .unwrap();
    let mut document = initial.draft.document.clone();
    document["graph"]["nodes"][1]["config"]["model_provider"] = json!({
        "provider_code": "missing_provider",
        "model_id": "lost-model"
    });
    service
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: owner_id,
            application_id: application.id,
            document,
            change_kind: FlowChangeKind::Logical,
            summary: "bind missing model".into(),
        })
        .await
        .unwrap();
    let template = service
        .export_agent_flow_template(owner_id, application.id)
        .await
        .unwrap();
    let expected_bindings = template.flow_document["graph"]["nodes"][1]["bindings"].clone();
    let expected_outputs = template.flow_document["graph"]["nodes"][1]["outputs"].clone();

    let preview = service
        .preview_agent_flow_template(PreviewAgentFlowTemplateCommand {
            actor_user_id: owner_id,
            template: template.clone(),
            resources: AgentFlowTemplateResourceSnapshot::default(),
        })
        .await
        .unwrap();
    let llm_node = &preview.document["graph"]["nodes"][1];
    let dependency = preview
        .dependencies
        .iter()
        .find(|dependency| dependency.dependency.kind == "model_provider")
        .expect("missing model provider dependency should be reported");

    assert!(preview.unresolved_nodes.is_empty());
    assert_eq!(dependency.status, "missing_dependency");
    assert_eq!(dependency.reason.as_deref(), Some("missing_model_provider"));
    assert_eq!(llm_node["type"], "llm");
    assert_eq!(
        llm_node["config"]["model_provider"]["provider_code"],
        "missing_provider"
    );
    assert_eq!(
        llm_node["config"]["model_provider"]["model_id"],
        "lost-model"
    );
    assert_eq!(llm_node["bindings"], expected_bindings);
    assert_eq!(llm_node["outputs"], expected_outputs);

    let imported = service
        .import_agent_flow_template(ImportAgentFlowTemplateCommand {
            actor_user_id: owner_id,
            template,
            name: Some("Imported missing provider".to_string()),
            description: None,
            resources: AgentFlowTemplateResourceSnapshot::default(),
        })
        .await
        .unwrap();
    let imported_llm = &imported.orchestration.draft.document["graph"]["nodes"][1];

    assert!(imported.preview.unresolved_nodes.is_empty());
    assert_eq!(imported_llm["type"], "llm");
    assert_eq!(
        imported_llm["config"]["model_provider"]["provider_code"],
        "missing_provider"
    );
    assert_eq!(
        imported_llm["config"]["model_provider"]["model_id"],
        "lost-model"
    );
}

#[tokio::test]
async fn preview_agent_flow_template_marks_unsupported_node_type_as_unresolved_node() {
    let owner_id = Uuid::now_v7();
    let service = FlowService::for_tests();
    let application = service
        .seed_application_for_actor(owner_id, "Support Agent")
        .await
        .unwrap();
    let mut template = service
        .export_agent_flow_template(owner_id, application.id)
        .await
        .unwrap();
    template.flow_document["graph"]["nodes"][1]["type"] = json!("future_llm");

    let preview = service
        .preview_agent_flow_template(PreviewAgentFlowTemplateCommand {
            actor_user_id: owner_id,
            template,
            resources: AgentFlowTemplateResourceSnapshot::default(),
        })
        .await
        .unwrap();
    let unresolved_node = &preview.document["graph"]["nodes"][1];

    assert_eq!(preview.unresolved_nodes.len(), 1);
    assert_eq!(unresolved_node["type"], "unresolved_node");
    assert_eq!(
        unresolved_node["config"]["unresolved"]["reason"],
        "unsupported_builtin_node"
    );
}

#[tokio::test]
async fn preview_agent_flow_template_rejects_dangling_edge_target() {
    let owner_id = Uuid::now_v7();
    let service = FlowService::for_tests();
    let application = service
        .seed_application_for_actor(owner_id, "Support Agent")
        .await
        .unwrap();
    let mut template = service
        .export_agent_flow_template(owner_id, application.id)
        .await
        .unwrap();
    template.flow_document["graph"]["edges"][0]["target"] = json!("missing-node");

    let error = service
        .preview_agent_flow_template(PreviewAgentFlowTemplateCommand {
            actor_user_id: owner_id,
            template,
            resources: AgentFlowTemplateResourceSnapshot::default(),
        })
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "invalid input: flow_document.graph.edges.target"
    );
}

#[tokio::test]
async fn import_agent_flow_template_creates_application_and_rewrites_only_flow_id() {
    let owner_id = Uuid::now_v7();
    let service = FlowService::for_tests();
    let source_application = service
        .seed_application_for_actor(owner_id, "Support Agent")
        .await
        .unwrap();
    let source_state = service
        .get_or_create_editor_state(owner_id, source_application.id)
        .await
        .unwrap();
    let template = service
        .export_agent_flow_template(owner_id, source_application.id)
        .await
        .unwrap();

    let imported = service
        .import_agent_flow_template(ImportAgentFlowTemplateCommand {
            actor_user_id: owner_id,
            template,
            name: Some("Imported Support Agent".to_string()),
            description: None,
            resources: AgentFlowTemplateResourceSnapshot::default(),
        })
        .await
        .unwrap();

    assert_ne!(imported.application.id, source_application.id);
    assert_ne!(imported.orchestration.flow.id, source_state.flow.id);
    assert_eq!(imported.application.name, "Imported Support Agent");
    assert_eq!(
        imported.orchestration.draft.document["meta"]["flowId"],
        imported.orchestration.flow.id.to_string()
    );
    assert_eq!(
        imported.orchestration.draft.document["graph"]["nodes"][0]["id"],
        source_state.draft.document["graph"]["nodes"][0]["id"]
    );
    assert_eq!(
        imported.orchestration.draft.document["graph"]["edges"][0]["id"],
        source_state.draft.document["graph"]["edges"][0]["id"]
    );
}

#[tokio::test]
async fn preview_agent_flow_template_recovers_unresolved_node_when_dependency_is_available() {
    let owner_id = Uuid::now_v7();
    let service = FlowService::for_tests();
    let application = service
        .seed_application_for_actor(owner_id, "Support Agent")
        .await
        .unwrap();
    let state = service
        .get_or_create_editor_state(owner_id, application.id)
        .await
        .unwrap();
    let mut template = service
        .export_agent_flow_template(owner_id, application.id)
        .await
        .unwrap();
    let mut original_node = state.draft.document["graph"]["nodes"][1].clone();
    original_node["config"]["model_provider"] = json!({
        "provider_code": "fixture_provider",
        "model_id": "gpt-4o-mini"
    });
    template.flow_document["graph"]["nodes"][1] = json!({
        "id": "node-llm",
        "type": "unresolved_node",
        "alias": "LLM",
        "description": "",
        "containerId": null,
        "position": { "x": 360, "y": 220 },
        "configVersion": 1,
        "config": {
            "unresolved": {
                "dependency_status": "missing_dependency",
                "reason": "missing_model_provider",
                "original_type": "llm",
                "original_node": original_node
            }
        },
        "bindings": {},
        "outputs": []
    });

    let preview = service
        .preview_agent_flow_template(PreviewAgentFlowTemplateCommand {
            actor_user_id: owner_id,
            template,
            resources: AgentFlowTemplateResourceSnapshot::from_ready_model(
                "fixture_provider",
                "gpt-4o-mini",
            ),
        })
        .await
        .unwrap();

    assert!(preview.unresolved_nodes.is_empty());
    assert_eq!(preview.document["graph"]["nodes"][1]["type"], "llm");
    assert_eq!(
        preview.document["graph"]["nodes"][1]["config"]["model_provider"]["model_id"],
        "gpt-4o-mini"
    );
}
