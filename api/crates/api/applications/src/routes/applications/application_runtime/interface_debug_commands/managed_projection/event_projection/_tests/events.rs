use super::*;
#[test]
fn debug_business_values_exclude_headers_documents_and_debug_session_handles() {
    let input = ApplicationRuntimeDebugCommandsInput::Start {
        application_id: Uuid::now_v7(),
        body: StartFlowDebugRunBody {
            input_payload: json!({"query":"actual debug input"}),
            document: Some(json!({"credential":"secret"})),
            debug_session_id: Some("private-handle".into()),
            mcp_instance_ids: Vec::new(),
        },
        headers: HeaderMap::from_iter([(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer credential"),
        )]),
    };
    let projected = input.project_for_event().unwrap();
    assert!(jsonschema::validator_for(&commands_input_schema())
        .unwrap()
        .is_valid(&projected));
    assert!(!jsonschema::validator_for(&commands_input_schema())
        .unwrap()
        .is_valid(&input.project_for_managed_hook().unwrap()));
    assert_eq!(
        projected["body"]["input_payload"]["query"],
        "actual debug input"
    );
    assert!(projected["body"].get("document").is_none());
    assert!(projected.get("headers").is_none());
    assert!(!projected.to_string().contains("private-handle"));
    assert_eq!(
        commands_input_schema()["oneOf"][0]["properties"]["body"]["properties"]["input_payload"],
        true
    );
}
#[test]
fn stream_result_is_actual_identity_without_claiming_application_terminal_status() {
    let application_id = Uuid::now_v7();
    let run_id = Uuid::now_v7();
    let output = ApplicationRuntimeDebugStreamOutput {
        application_id,
        run_id,
    };
    let projected = output.project_for_event().unwrap();
    assert!(jsonschema::validator_for(&stream_output_schema())
        .unwrap()
        .is_valid(&projected));
    assert_eq!(projected["application_id"], application_id.to_string());
    assert_eq!(projected["run_id"], run_id.to_string());
    assert!(projected.get("status").is_none());
    assert!(output
        .project_for_managed_hook()
        .unwrap()
        .get("run_id")
        .is_none());
}

#[test]
fn debug_run_output_projection_keeps_real_query_and_output_without_internal_references() {
    let value: FlowRunResponse = serde_json::from_value(json!({
        "id":"run-1","application_id":"application-1","flow_id":"flow-1","draft_id":"draft-1","compiled_plan_id":null,
        "run_mode":"flow_debug","status":"succeeded","target_node_id":null,"title":"actual title","expand_id":null,
        "authorized_account":"private-account","external_conversation_id":null,"query":"actual query","model":"actual model",
        "input_payload":{"value":"actual input"},"output_payload":{"answer":"actual answer"},"error_payload":null,
        "created_by":"user-1","started_at":"2026-11-07T00:00:00Z","finished_at":"2026-11-07T00:00:01Z","created_at":"2026-11-07T00:00:00Z","updated_at":"2026-11-07T00:00:01Z",
    })).unwrap();
    let projected = flow(&value);
    assert_eq!(projected["query"], "actual query");
    assert_eq!(projected["output_payload"]["answer"], "actual answer");
    assert!(projected.get("authorized_account").is_none());
    assert!(jsonschema::validator_for(&flow_schema())
        .unwrap()
        .is_valid(&projected));
}
