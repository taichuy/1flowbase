use super::*;
use crate::event_center::api_projection::{ApiEventInput, ApiEventOutput};
use crate::routes::application_api::{
    interface_keys::ApplicationApiKeyOutput, CreatedApplicationApiKeyResponse,
};
use crate::routes::application_runtime::interface_debug_commands::{
    ApplicationRuntimeDebugCommandsInput, ApplicationRuntimeDebugStreamOutput,
};
use crate::routes::applications::interface::{ApplicationsInput, ApplicationsOutput};
use crate::routes::applications::{
    ApplicationEnvironmentVariableResponse, ApplicationSummaryResponse, ApplicationTypeDto,
};
use uuid::Uuid;

fn operation(interface_id: &str, method: &str, path: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: interface_id.into(),
        method: method.into(),
        path: path.into(),
    }
}

#[test]
fn application_document_debug_input_uses_actual_body_and_omits_headers() {
    let application_id = Uuid::now_v7();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "authorization",
        "Bearer DOCUMENT_SENTINEL_SECRET".parse().unwrap(),
    );
    let expected = json!({"input_payload":{"question":"actual question","items":[1,2,3]},"document":{"nodes":[]},"debug_session_id":"session","mcp_instance_ids":["one","two"]});
    let mut source = expected.clone();
    source["input_payload"]["env"] = json!({"token":"ENV_DEBUG_SENTINEL_SECRET"});
    let input = ApplicationRuntimeDebugCommandsInput::Start {
        application_id,
        body: serde_json::from_value(source.clone()).unwrap(),
        headers,
    };
    let context = operation(
        "applications.runtime.debug-runs.create",
        "POST",
        "/api/console/applications/:id/orchestration/debug-runs",
    );
    let value = input.project_document_input(&context).unwrap();
    assert_eq!(value, json!({"path":{"id":application_id},"body":expected}));
    assert!(!value.to_string().contains("DOCUMENT_SENTINEL_SECRET"));
    let schema = ApplicationRuntimeDebugCommandsInput::document_input_schema(&context).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&value));
    assert!(!validator.is_valid(&json!({"path":{"id":application_id},"body":source})));
}

#[test]
fn application_document_list_output_keeps_all_actual_items_without_enum_tags() {
    let items = (0..40)
        .map(|index| ApplicationSummaryResponse {
            id: Uuid::now_v7().to_string(),
            application_type: ApplicationTypeDto::AgentFlow,
            name: format!("actual application {index}"),
            description: "actual description".repeat(100),
            icon: None,
            icon_type: None,
            icon_background: None,
            created_by: Uuid::now_v7().to_string(),
            updated_at: "2026-11-07T00:00:00Z".into(),
            tags: vec![],
        })
        .collect::<Vec<_>>();
    let expected = serde_json::to_value(&items).unwrap();
    let output = ApplicationsOutput::Applications(items);
    let value = output
        .project_document_output(&operation(
            "applications.list",
            "GET",
            "/api/console/applications",
        ))
        .unwrap();
    assert_eq!(value, expected);
    assert_eq!(value.as_array().unwrap().len(), 40);
    assert_eq!(value[39]["name"], "actual application 39");
    assert!(value[39]["description"].as_str().unwrap().len() > 1000);
    assert!(value.get("variant").is_none());
}

#[test]
fn application_document_environment_values_are_absent_from_values_and_schemas() {
    let context = operation(
        "applications.environment-variables.replace",
        "PUT",
        "/api/console/applications/:id/environment-variables",
    );
    let input = ApplicationsInput::ReplaceEnvironmentVariables {
        application_id: Uuid::nil(),
        body: serde_json::from_value(json!({"variables":[{"name":"API_TOKEN","value_type":"string","value":"ENV_DOCUMENT_SENTINEL_SECRET","description":"owner metadata"}]})).unwrap(),
    };
    let projected = input.project_document_input(&context).unwrap();
    assert_eq!(
        projected["body"]["variables"][0],
        json!({"name":"API_TOKEN","value_type":"string","description":"owner metadata"})
    );
    assert!(!projected
        .to_string()
        .contains("ENV_DOCUMENT_SENTINEL_SECRET"));
    let schema = ApplicationsInput::document_input_schema(&context).unwrap();
    assert!(!schema.to_string().contains("\"value\""));
    assert!(jsonschema::validator_for(&schema)
        .unwrap()
        .is_valid(&projected));
    let output =
        ApplicationsOutput::EnvironmentVariables(vec![ApplicationEnvironmentVariableResponse {
            name: "API_TOKEN".into(),
            value_type: "string".into(),
            value: json!("ENV_DOCUMENT_SENTINEL_SECRET"),
            description: "owner metadata".into(),
            updated_at: "2026-11-07T00:00:00Z".into(),
        }]);
    let projected = output.project_document_output(&context).unwrap();
    assert!(!projected
        .to_string()
        .contains("ENV_DOCUMENT_SENTINEL_SECRET"));
    let schema = ApplicationsOutput::document_output_schema(&context).unwrap();
    assert!(!schema.to_string().contains("\"value\""));
    assert!(jsonschema::validator_for(&schema)
        .unwrap()
        .is_valid(&projected));
}

#[test]
fn application_document_created_key_omits_token_without_changing_http_dto() {
    let value = CreatedApplicationApiKeyResponse {
        id: Uuid::now_v7(),
        name: "actual key".into(),
        token: "KEY_DOCUMENT_SENTINEL_SECRET".into(),
        token_prefix: "prefix".into(),
        creator_user_id: Uuid::now_v7(),
        enabled: true,
        expires_at: None,
        last_used_at: None,
        created_at: "2026-11-07T00:00:00Z".into(),
        updated_at: "2026-11-07T00:00:00Z".into(),
    };
    assert_eq!(
        serde_json::to_value(&value).unwrap()["token"],
        "KEY_DOCUMENT_SENTINEL_SECRET"
    );
    let output = ApplicationApiKeyOutput::Created(value);
    let context = operation(
        "applications.api-keys.create",
        "POST",
        "/api/console/applications/:application_id/api-keys",
    );
    let projected = output.project_document_output(&context).unwrap();
    assert_eq!(projected["name"], "actual key");
    assert!(projected.get("token").is_none());
    assert!(projected.get("token_prefix").is_none());
    let schema = ApplicationApiKeyOutput::document_output_schema(&context).unwrap();
    assert!(!schema.to_string().contains("\"token\""));
    assert!(jsonschema::validator_for(&schema)
        .unwrap()
        .is_valid(&projected));
}

#[test]
fn application_document_stream_terminal_is_the_typed_locator() {
    let output = ApplicationRuntimeDebugStreamOutput {
        application_id: Uuid::now_v7(),
        run_id: Uuid::now_v7(),
    };
    let context = operation(
        "applications.runtime.debug-runs.stream.create",
        "POST",
        "/api/console/applications/:id/orchestration/debug-runs/stream",
    );
    let value = output.project_document_output(&context).unwrap();
    assert_eq!(
        value,
        json!({"application_id":output.application_id,"run_id":output.run_id})
    );
    assert!(jsonschema::validator_for(
        &ApplicationRuntimeDebugStreamOutput::document_output_schema(&context).unwrap()
    )
    .unwrap()
    .is_valid(&value));
    assert!(value.get("stream_completed").is_none());
}

#[test]
fn application_document_raw_query_owner_omits_unknown_credential_carriers() {
    assert_eq!(preview_query(Some("artifact_preview=auto&artifact_preview_field=body.text&artifact_preview_field=items&access_token=QUERY_SENTINEL_SECRET")),json!({"artifact_preview":"auto","artifact_preview_field":["body.text","items"]}));
    let context = operation("applications.get", "GET", "/api/console/applications/:id");
    assert_eq!(
        request(
            &context,
            json!({"application_id":Uuid::nil()}),
            json!({"optional":null}),
            Value::Null
        ),
        json!({"path":{"id":Uuid::nil()}})
    );
}

#[test]
fn application_document_run_payload_omits_frozen_env_and_preserves_nested_business_json() {
    use crate::routes::application_runtime::interface_runtime_reads::ApplicationRuntimeReadsOutput;
    let context = operation(
        "applications.runtime.run.payload.get",
        "GET",
        "/api/console/applications/:id/logs/runs/:run_id/payloads/:section",
    );
    let source = json!({"env":{"API_TOKEN":"FROZEN_ENV_DOCUMENT_SECRET"},"question":"actual question","user_data":{"env":{"meaning":"business-owned nested field"}}});
    let output = ApplicationRuntimeReadsOutput::RunPayload(source.clone());
    let value = output.project_document_output(&context).unwrap();
    assert_eq!(value["question"], "actual question");
    assert_eq!(value["user_data"], source["user_data"]);
    assert!(value.get("env").is_none());
    let schema = ApplicationRuntimeReadsOutput::document_output_schema(&context).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&value));
    assert!(!validator.is_valid(&source));
}

#[test]
fn application_document_json_downloads_have_schemas_and_only_zip_completion_is_unavailable() {
    use crate::routes::application_runtime::archive::interface::ApplicationRuntimeArchiveOutput;
    use crate::routes::application_runtime::interface_trace_exports::ApplicationRuntimeTraceExportsOutput;
    let archive = operation(
        "applications.runtime.archive.run.export",
        "GET",
        "/api/console/applications/:id/logs/runs/:run_id/archive",
    );
    let schema = ApplicationRuntimeArchiveOutput::document_output_schema(&archive).unwrap();
    assert!(ApplicationRuntimeArchiveOutput::document_output_unavailable(&archive).is_none());
    for field in [
        "compiled_plan",
        "flow_run_fact",
        "runtime_spans",
        "canonical_contents",
        "invocation_context_bindings",
        "resume_claims",
    ] {
        assert!(
            !schema.to_string().contains(&format!("\"{field}\"")),
            "opaque restore carrier must not enter safe schema: {field}"
        );
    }
    let trace = operation(
        "applications.runtime.trace-export.get",
        "GET",
        "/api/console/applications/:id/logs/runs/:run_id/export",
    );
    assert!(ApplicationRuntimeTraceExportsOutput::document_output_schema(&trace).is_some());
    assert!(ApplicationRuntimeTraceExportsOutput::document_output_unavailable(&trace).is_none());
    let zip = operation(
        "applications.runtime.trace-export.selected-runs",
        "POST",
        "/api/console/applications/:id/logs/runs/export",
    );
    assert_eq!(
        ApplicationRuntimeTraceExportsOutput::document_output_unavailable(&zip),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    );
}
