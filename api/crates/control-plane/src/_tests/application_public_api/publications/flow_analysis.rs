use super::*;
use control_plane::flow::{FlowService, SaveFlowDraftCommand};
use serde_json::{json, Value};

fn invalid_output_reference_document() -> Value {
    let mut document = domain::default_flow_document_for_application(
        domain::ApplicationType::Workflow,
        Uuid::nil(),
    );
    let nodes = document["graph"]["nodes"].as_array_mut().unwrap();
    let start_id = nodes[0]["id"].as_str().unwrap().to_owned();
    let end_id = nodes[1]["id"].as_str().unwrap().to_owned();
    nodes[1]["bindings"]["result"] = json!({
        "kind":"selector", "value":["code", "result", "missing"]
    });
    let mut code = nodes[1].clone();
    code["id"] = json!("code");
    code["type"] = json!("code");
    code["alias"] = json!("Code");
    code["bindings"] = json!({});
    code["config"] = json!({
        "source":"function main(){return {allowed:1}}",
        "language":"javascript",
        "entrypoint":"main"
    });
    code["outputs"] = json!([{
        "key":"payload", "title":"Payload", "valueType":"json", "selector":["result"],
        "jsonSchema":{"type":"object", "properties":{"allowed":{"type":"number"}}, "additionalProperties":false}
    }]);
    nodes.push(code);
    document["graph"]["edges"] = json!([
        {"id":"start-code", "source":start_id, "target":"code"},
        {"id":"code-end", "source":"code", "target":end_id}
    ]);
    document
}

fn assert_output_reference_error(error: &anyhow::Error) {
    let validation = error
        .downcast_ref::<orchestration_runtime::compiler::FlowValidationError>()
        .unwrap_or_else(|| panic!("expected output-reference diagnostics, got {error:#}"));
    assert!(!validation.diagnostics.is_empty());
    for diagnostic in &validation.diagnostics {
        assert_eq!(diagnostic.node_id.as_deref(), Some("node-workflow-end"));
        assert_eq!(diagnostic.code, "selector_output_not_found");
        assert_eq!(diagnostic.field_path, "/bindings/result");
        assert!(
            diagnostic.message.contains("code.result.missing"),
            "{diagnostic:?}"
        );
    }
}

#[tokio::test]
async fn analysis_is_read_only_and_enforces_application_visibility() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![application_row_operation(
            access_control::APPLICATIONS_VIEW_OPERATION_ID,
            domain::ConsoleOperationRowScope::Own,
        )]),
    ]);
    let application = harness.seed_workflow_application(actor_user_id(), "Analysis");
    let repository = harness.repository();
    let flows = FlowService::new(repository.clone());
    let document = invalid_output_reference_document();
    let analysis = flows
        .analyze_document(actor_user_id(), application.id, &document)
        .await
        .unwrap();
    let errors = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == "error")
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1, "{:?}", analysis.diagnostics);
    assert_eq!(errors[0].node_id.as_deref(), Some("node-workflow-end"));
    assert_eq!(errors[0].code, "selector_output_not_found");
    assert_eq!(errors[0].field_path, "/bindings/result");
    assert!(
        repository
            .load_flow_draft_document(application.workspace_id, application.id)
            .await
            .unwrap()
            .is_none(),
        "analysis must not bootstrap a draft"
    );
    assert_eq!(repository.editor_state_read_count(), 0);
    assert!(
        flows
            .analyze_document(other_user_id(), application.id, &document)
            .await
            .is_err(),
        "row visibility also applies to analysis"
    );
    assert!(repository
        .load_flow_draft_document(application.workspace_id, application.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn analysis_expands_catalogs_only_for_the_selected_node_and_keeps_whole_graph_diagnostics() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(root_user_id(), "Selected node catalogs");
    let repository = harness.repository();
    let flows = FlowService::new(repository.clone());
    let document = invalid_output_reference_document();
    let unselected = flows
        .analyze_document(root_user_id(), application.id, &document)
        .await
        .unwrap();
    assert!(unselected.variable_catalogs.is_empty());
    let selected = flows
        .analyze_document_for_node(
            root_user_id(),
            application.id,
            &document,
            Some("node-workflow-end"),
        )
        .await
        .unwrap();
    assert!(!selected.variable_catalogs.is_empty());
    assert!(selected
        .variable_catalogs
        .iter()
        .all(|catalog| catalog.node_id == "node-workflow-end"));
    assert!(selected
        .variable_catalogs
        .iter()
        .any(|catalog| catalog.field_path == "/bindings/result"));
    assert_eq!(selected.diagnostics, unselected.diagnostics);
    assert_eq!(selected.node_outputs, unselected.node_outputs);
    assert!(repository
        .load_flow_draft_document(application.workspace_id, application.id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(repository.editor_state_read_count(), 0);
}

#[tokio::test]
async fn analysis_uses_the_owned_application_family_instead_of_the_submitted_document_family() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(root_user_id(), "Owned AgentFlow");
    let repository = harness.repository();
    let flows = FlowService::new(repository.clone());
    let before = flows
        .get_or_create_editor_state(root_user_id(), application.id)
        .await
        .unwrap();
    repository.reset_editor_state_read_count();
    let workflow = domain::default_flow_document_for_application(
        domain::ApplicationType::Workflow,
        Uuid::nil(),
    );
    let analysis = flows
        .analyze_document(root_user_id(), application.id, &workflow)
        .await
        .unwrap();
    assert!(analysis.has_errors());
    assert!(
        analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.level == "error"
                && diagnostic
                    .message
                    .contains("agent_flow document cannot contain workflow_start")
        }),
        "{:?}",
        analysis.diagnostics
    );
    assert_eq!(repository.editor_state_read_count(), 0);
    let after = flows
        .get_or_create_editor_state(root_user_id(), application.id)
        .await
        .unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn authorized_editor_projection_does_not_add_a_view_grant_to_other_commands() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![]);
    let application = harness.seed_workflow_application(actor_user_id(), "Independent command");
    let repository = harness.repository();
    let state = repository
        .get_or_create_editor_state(application.workspace_id, application.id, actor_user_id())
        .await
        .unwrap();
    let flows = FlowService::new(repository);
    assert!(
        flows
            .analyze_document(actor_user_id(), application.id, &state.draft.document)
            .await
            .is_err(),
        "the standalone analysis endpoint requires view access"
    );
    flows
        .analyze_editor_state(actor_user_id(), &state)
        .await
        .expect("an already authorized state projection must not add another operation grant");
}

#[tokio::test]
async fn schedule_enable_checks_the_published_snapshot_as_well_as_the_current_draft() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(root_user_id(), "Published schedule");
    let repository = harness.repository();
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: false,
        })
        .await
        .unwrap();
    let flows = FlowService::new(repository.clone());
    let before = flows
        .get_or_create_editor_state(root_user_id(), application.id)
        .await
        .unwrap();
    assert!(!flows
        .analyze_document(root_user_id(), application.id, &before.draft.document)
        .await
        .unwrap()
        .has_errors());
    repository.set_active_publication_document_snapshot(
        application.id,
        invalid_output_reference_document(),
    );
    repository.reset_editor_state_read_count();
    let schedules = WorkflowScheduleTriggerService::new(repository.clone());
    let error = schedules
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "UTC".into(),
            input_payload: json!({}),
        })
        .await
        .expect_err("the schedule must validate the version it actually executes");
    assert_output_reference_error(&error);
    assert_eq!(repository.editor_state_read_count(), 0);
    assert!(schedules
        .get_trigger(GetWorkflowScheduleTriggerCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        flows
            .get_or_create_editor_state(root_user_id(), application.id)
            .await
            .unwrap(),
        before
    );
}

#[tokio::test]
async fn semantic_errors_save_but_block_publication_and_schedule_without_history_writes() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(root_user_id(), "Invalid output reference");
    let repository = harness.repository();
    let flows = FlowService::new(repository.clone());
    let document = invalid_output_reference_document();
    let saved = flows
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
            document: document.clone(),
            change_kind: domain::FlowChangeKind::Logical,
            summary: "Editable invalid draft".into(),
        })
        .await
        .unwrap();
    assert_eq!(saved.draft.document, document);
    repository.reset_editor_state_read_count();
    let publication_error = ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .expect_err("invalid output references must not publish");
    assert_output_reference_error(&publication_error);
    let schedules = WorkflowScheduleTriggerService::new(repository.clone());
    let enable_error = schedules
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "UTC".into(),
            input_payload: json!({}),
        })
        .await
        .expect_err("invalid output references must not enable schedules");
    assert_output_reference_error(&enable_error);
    assert_eq!(
        repository.editor_state_read_count(),
        0,
        "rejection must not bootstrap or freeze"
    );
    assert!(schedules
        .get_trigger(GetWorkflowScheduleTriggerCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap()
        .is_none());
    let after = flows
        .get_or_create_editor_state(root_user_id(), application.id)
        .await
        .unwrap();
    assert_eq!(
        after, saved,
        "rejection preserves draft, timestamps and complete history"
    );
    schedules
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
            enabled: false,
            cron: "0 9 * * *".into(),
            timezone: "UTC".into(),
            input_payload: json!({}),
        })
        .await
        .expect("an invalid draft must still permit disabling schedules");
}

#[tokio::test]
async fn dynamic_json_warnings_allow_publication_and_schedule_enable_without_executing_code() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(root_user_id(), "Dynamic JSON output");
    let repository = harness.repository();
    let flows = FlowService::new(repository.clone());
    let mut document = invalid_output_reference_document();
    document["graph"]["nodes"][1]["bindings"]["result"]["value"] =
        json!(["code", "result", "dynamic", "name"]);
    document["graph"]["nodes"][2]["config"]["source"] = json!("function main(){return {}}");
    document["graph"]["nodes"][2]["outputs"][0]["jsonSchema"] = json!({"type":"object"});
    flows
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
            document: document.clone(),
            change_kind: domain::FlowChangeKind::Logical,
            summary: "Keep unknown JSON paths as warnings".into(),
        })
        .await
        .unwrap();
    let analysis = flows
        .analyze_document(root_user_id(), application.id, &document)
        .await
        .unwrap();
    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    assert!(
        analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.level == "warning"
                && diagnostic.code == "dynamic_selector_path"
                && diagnostic.node_id.as_deref() == Some("node-workflow-end")
                && diagnostic.field_path == "/bindings/result"
                && diagnostic.selector.as_ref().is_some_and(|selector| {
                    selector
                        .iter()
                        .map(String::as_str)
                        .eq(["code", "result", "dynamic", "name"])
                })
        }),
        "{:?}",
        analysis.diagnostics
    );
    let publication = ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: false,
        })
        .await
        .expect("dynamic JSON warnings must not block publication");
    assert!(publication.active);
    let schedule = WorkflowScheduleTriggerService::new(repository)
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "UTC".into(),
            input_payload: json!({}),
        })
        .await
        .expect("dynamic JSON warnings must not block scheduling the active publication");
    assert!(schedule.enabled);
}
