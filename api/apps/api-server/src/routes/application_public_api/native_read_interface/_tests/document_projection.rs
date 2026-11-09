use super::*;
fn operation() -> ApiOperationContext {
    ApiOperationContext {
        interface_id: "application.native.runs.resume".into(),
        method: "POST".into(),
        path: "/api/agent/v1/runs/:run_id/resume".into(),
    }
}

#[test]
fn native_read_path_body_and_binary_boundaries_follow_the_http_contract() {
    let id = Uuid::now_v7();
    let callback = Uuid::now_v7();
    for value in [
        NativeGetRunInput(id)
            .project_document_input(&operation())
            .unwrap(),
        NativeCancelRunInput(id)
            .project_document_input(&operation())
            .unwrap(),
    ] {
        assert_eq!(value, json!({"path":{"run_id":id}}));
        assert!(value.get("body").is_none());
        assert!(value.get("0").is_none());
    }
    let resume = NativeResumeRunInput {
        run_id: id,
        callback_task_id: callback,
        response_payload: json!({"rows":(0..45).collect::<Vec<_>>(),"text":"actual callback".repeat(300)}),
        response_mode: Some("blocking".into()),
    };
    let value = resume.project_document_input(&operation()).unwrap();
    assert_eq!(value["path"]["run_id"], id.to_string());
    assert_eq!(value["body"]["callback_task_id"], callback.to_string());
    assert_eq!(
        value["body"]["response_payload"]["rows"]
            .as_array()
            .unwrap()
            .len(),
        45
    );
    assert_eq!(
        value["body"]["response_payload"]["text"],
        "actual callback".repeat(300)
    );
    assert_eq!(
        NativeModelsInput
            .project_document_input(&operation())
            .unwrap(),
        json!({})
    );
    assert_eq!(
        NativeUploadFileInput::document_input_unavailable(&operation()),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    );
}
