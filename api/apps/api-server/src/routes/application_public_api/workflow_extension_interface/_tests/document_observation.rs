use super::*;
use control_plane::application_public_api::{
    mapping::{
        ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
        WorkflowExtensionApiConfig,
    },
    publications::ApplicationPublicationVersionRecord,
    published_workflow_operation::PublishedWorkflowOperation,
};
use time::OffsetDateTime;
use uuid::Uuid;

fn sample_operation(mode: WorkflowExtensionResponseMode) -> PublishedWorkflowOperation {
    PublishedWorkflowOperation::from_publication(ApplicationPublicationVersionRecord {
        id:Uuid::now_v7(),application_id:Uuid::now_v7(),workspace_id:Uuid::now_v7(),flow_id:Uuid::now_v7(),flow_version_id:Uuid::now_v7(),
        mapping_snapshot:ApplicationApiMappingConfig {input:ApplicationApiMappingInput {query_target:"start".into(),model_target:None,inputs_target:None,history_target:None,attachments_target:None},output:ApplicationApiMappingOutput::default(),extension:Some(WorkflowExtensionApiConfig {slug:"orders/{order_id}".into(),method:WorkflowExtensionHttpMethod::Post,response_mode:mode})},
        extension_slug:Some("orders/{order_id}".into()),compiled_plan_id:Uuid::now_v7(),version_sequence:1,active:true,api_enabled:true,flow_schema_version:"1flowbase.flow/v2".into(),document_hash:"fixture-hash".into(),
        document_snapshot:json!({"graph":{"nodes":[{"id":"start","type":"workflow_start","config":{"input_fields":[
            {"key":"order_id","source":"path","valueType":"string","required":true},
            {"key":"count","source":"query","valueType":"number","required":true},
            {"key":"enabled","source":"query","valueType":"boolean","defaultValue":true},
            {"key":"message","source":"body","valueType":"string"},
            {"key":"note","source":"form","valueType":"string"}
        ]}},{"id":"end","type":"workflow_end","outputs":[{"key":"answer","valueType":"string"}]}]}}),
        runtime_profile_snapshot:json!({"private_runtime_profile":"credential-sentinel"}),output_selector:json!({}),dependency_snapshot:Vec::new(),created_by:Uuid::now_v7(),created_at:OffsetDateTime::UNIX_EPOCH,
    }).unwrap()
}
fn detail(status: domain::FlowRunStatus) -> domain::ApplicationRunDetail {
    domain::ApplicationRunDetail {
        flow_run: domain::FlowRunRecord {
            id: Uuid::now_v7(),
            application_id: Uuid::now_v7(),
            flow_id: Uuid::now_v7(),
            draft_id: Uuid::now_v7(),
            compiled_plan_id: Some(Uuid::now_v7()),
            debug_session_id: String::new(),
            flow_schema_version: "v2".into(),
            document_hash: "hash".into(),
            run_mode: domain::FlowRunMode::WorkflowHttpRun,
            target_node_id: None,
            title: "runtime-detail-sentinel".into(),
            status,
            input_payload: json!({"env":"credential-sentinel"}),
            output_payload: json!({"answer":"business-answer"}),
            error_payload: Some(json!({"message":"failed"})),
            created_by: Uuid::now_v7(),
            authorized_account: None,
            api_key_id: None,
            publication_version_id: Some(Uuid::now_v7()),
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        },
        native_messages: vec![json!({"private_trace":"credential-sentinel"})],
        node_runs: Vec::new(),
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
        task_rounds: Vec::new(),
        child_task_traces: Vec::new(),
    }
}
#[test]
fn workflow_codec_projects_actual_resolved_and_coerced_request_without_undeclared_values() {
    let operation = sample_operation(WorkflowExtensionResponseMode::Sync);
    let descriptor =
        descriptor_for_operation(&operation, &operation.snapshot_generation()).unwrap();
    let codec = Codec(Arc::new(operation));
    let input = WorkflowExtensionInput {
        request_path: "orders/actual-order".into(),
        method: WorkflowExtensionHttpMethod::Post,
        parameters: WorkflowExtensionRequestParameters {
            path: std::collections::BTreeMap::from([(
                "order_id".into(),
                json!("request-injected-target"),
            )]),
            query: serde_json::from_value(
                json!({"count":"12","enabled":"false","unknown_credential":"credential-sentinel"}),
            )
            .unwrap(),
            form: serde_json::from_value(json!({"note":"actual-form"})).unwrap(),
            body: json!({"message":"actual-message","unknown":"credential-sentinel"}),
        },
    };
    let projected = codec.project_input(&input).unwrap();
    assert_eq!(
        projected,
        json!({"path":{"order_id":"actual-order"},"query":{"count":12.0,"enabled":false},"body":{"message":"actual-message","note":"actual-form"}})
    );
    assert!(!projected.to_string().contains("credential-sentinel"));
    assert!(
        jsonschema::validator_for(descriptor.schemas.request.as_ref().unwrap())
            .unwrap()
            .is_valid(&projected)
    );
}
#[test]
fn output_is_http_business_payload_or_acceptance_receipt_and_never_run_detail() {
    for mode in [
        WorkflowExtensionResponseMode::Sync,
        WorkflowExtensionResponseMode::Async,
    ] {
        let operation = sample_operation(mode);
        let descriptor =
            descriptor_for_operation(&operation, &operation.snapshot_generation()).unwrap();
        let codec = Codec(Arc::new(operation));
        let validator =
            jsonschema::validator_for(descriptor.schemas.result.as_ref().unwrap()).unwrap();
        let run_id = Uuid::now_v7();
        let accepted = codec
            .project_output(&WorkflowExtensionOutput::Accepted {
                run_id,
                status: domain::FlowRunStatus::Running,
            })
            .unwrap();
        assert_eq!(accepted, json!({"run_id":run_id,"status":"running"}));
        assert!(validator.is_valid(&accepted));
        if mode == WorkflowExtensionResponseMode::Sync {
            let completed = codec
                .project_output(&WorkflowExtensionOutput::Completed(detail(
                    domain::FlowRunStatus::Succeeded,
                )))
                .unwrap();
            assert_eq!(completed, json!({"answer":"business-answer"}));
            assert!(validator.is_valid(&completed));
            assert!(!completed.to_string().contains("credential-sentinel"));
        }
    }
}
#[test]
fn failed_cancelled_or_incomplete_http_result_is_normalized_before_kernel_success() {
    let operation = sample_operation(WorkflowExtensionResponseMode::Sync);
    let codec = Codec(Arc::new(operation));
    for status in [
        domain::FlowRunStatus::Failed,
        domain::FlowRunStatus::Cancelled,
        domain::FlowRunStatus::Incomplete,
    ] {
        assert!(codec
            .project_output(&WorkflowExtensionOutput::Completed(detail(status)))
            .is_err());
        assert!(
            crate::routes::application_public_api::ex::normalize_workflow_extension_output(
                WorkflowExtensionOutput::Completed(detail(status))
            )
            .is_err()
        );
    }
    let output = crate::routes::application_public_api::ex::normalize_workflow_extension_output(
        WorkflowExtensionOutput::Completed(detail(domain::FlowRunStatus::WaitingCallback)),
    )
    .unwrap();
    assert!(matches!(
        output,
        WorkflowExtensionOutput::Accepted {
            status: domain::FlowRunStatus::WaitingCallback,
            ..
        }
    ));
}
#[test]
fn selector_rejects_request_identity_and_unsealed_publication_snapshot() {
    let operation = sample_operation(WorkflowExtensionResponseMode::Sync);
    assert!(Selector.select(&operation).is_err());
    assert!(Selector
        .select(&json!({"operation_id":operation.interface_id}))
        .is_err());
}
#[test]
fn document_identity_and_generation_distinguish_applications_and_same_uuid_republish() {
    let operation = sample_operation(WorkflowExtensionResponseMode::Sync);
    let original = descriptor_for_operation(&operation, &operation.snapshot_generation()).unwrap();
    let mut publication = operation.publication.clone();
    publication.document_snapshot["graph"]["nodes"][1]["outputs"] =
        json!([{"key":"new_answer","valueType":"boolean"}]);
    let replacement = PublishedWorkflowOperation::from_publication(publication).unwrap();
    let replaced =
        descriptor_for_operation(&replacement, &replacement.snapshot_generation()).unwrap();
    assert_eq!(original.operation_id, replaced.operation_id);
    assert_ne!(original.target_generation, replaced.target_generation);
    assert_ne!(original.schemas.result, replaced.schemas.result);
    let other = sample_operation(WorkflowExtensionResponseMode::Sync);
    let other = descriptor_for_operation(&other, &other.snapshot_generation()).unwrap();
    assert_ne!(original.operation_id, other.operation_id);
    assert_ne!(original.target_identity, other.target_identity);
    assert_eq!(original.binding_id, other.binding_id);
}
