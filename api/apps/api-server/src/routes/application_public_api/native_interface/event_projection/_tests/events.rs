use super::*;
#[test]
fn native_business_input_keeps_query_and_model_without_protocol_envelope_or_handles() {
    let mut request: NativeRunRequest = serde_json::from_value(
        json!({"query":"actual query","model":"actual-model","inputs":{"question":"actual input"}}),
    )
    .unwrap();
    request.attachments = serde_json::from_value(
        json!([{"source":"url","value":"https://private.invalid/?token=secret"}]),
    )
    .unwrap();
    let input = ApplicationNativeRunInput {
        request,
        protocol: TranslationProtocol::Native,
    };
    let projected = input.project_for_event().unwrap();
    let validator = jsonschema::validator_for(&input_schema()).unwrap();
    assert!(validator.is_valid(&projected));
    assert!(!validator.is_valid(&input.project_for_managed_hook().unwrap()));
    assert_eq!(projected["request"]["query"], "actual query");
    assert_eq!(projected["request"]["model"], "actual-model");
    assert_eq!(projected["request"]["inputs"]["question"], "actual input");
    assert!(projected["request"]
        .get("client_protocol_envelope")
        .is_none());
    assert!(projected["request"].get("attachments").is_none());
    assert!(projected.to_string().find("token=secret").is_none());
    assert!(
        input.project_for_managed_hook().unwrap()["request"]["query"]
            .get("byte_count")
            .is_some()
    );
}
#[test]
fn native_result_keeps_actual_answer_and_uses_the_declared_output_schema() {
    let output = ApplicationNativeRunOutput(NativeRunResult {
        id: uuid::Uuid::now_v7(),
        application_id: uuid::Uuid::now_v7(),
        api_key_id: uuid::Uuid::now_v7(),
        publication_version_id: uuid::Uuid::now_v7(),
        status: control_plane::application_public_api::native::NativeRunStatus::Succeeded,
        node_input_payload: json!({"private_internal":"not projected"}),
        metadata: json!({"internal_handle":"not projected"}),
        answer: Some("actual answer".into()),
        answer_segments: None,
        required_action: None,
        tool_calls: None,
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: time::OffsetDateTime::now_utc(),
    });
    let projected = output.project_for_event().unwrap();
    let validator = jsonschema::validator_for(&output_schema()).unwrap();
    assert!(validator.is_valid(&projected));
    assert!(!validator.is_valid(&output.project_for_managed_hook().unwrap()));
    assert_eq!(projected["0"]["answer"], "actual answer");
    assert_eq!(projected["0"]["status"], "succeeded");
    assert!(projected["0"].get("api_key_id").is_none());
    assert!(projected["0"].get("metadata").is_none());
    assert_eq!(
        output_schema()["properties"]["0"]["properties"]["answer"]["anyOf"][0]["type"],
        "string"
    );
}

struct Noop;
impl ApplicationNativeRunPort for Noop {
    fn create<'a>(
        &'a self,
        _: &'a ApplicationPrincipal,
        _: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunFuture<'a> {
        Box::pin(std::future::pending())
    }
    fn execute_blocking<'a>(
        &'a self,
        _: &'a ApplicationPrincipal,
        _: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunFuture<'a> {
        Box::pin(std::future::pending())
    }
    fn execute_stream<'a>(
        &'a self,
        _: &'a ApplicationPrincipal,
        _: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunStreamFuture<'a> {
        Box::pin(std::future::pending())
    }
}
#[test]
fn native_create_blocking_and_stream_have_declared_event_capabilities_at_boot() {
    let registry = compile_registry(Arc::new(Noop)).unwrap();
    let ids = registry
        .event_capabilities()
        .map(|capability| capability.definition().interface_id().as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            ASYNC_INTERFACE_ID,
            BLOCKING_INTERFACE_ID,
            STREAM_INTERFACE_ID
        ]
    );
    assert_eq!(
        registry
            .event_capability(&InterfaceId::new(STREAM_INTERFACE_ID).unwrap())
            .unwrap()
            .output_schema()["properties"]["0"]["properties"]["answer"]["anyOf"][0]["type"],
        "string"
    );
}
