use super::*;
fn context(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "GET".into(),
        path: "/fixture".into(),
    }
}

#[test]
fn process_events_omit_command_credentials_without_changing_http_dto() {
    let record = SystemProcessResponse {
        pid: 42,
        parent_pid: None,
        name: "worker".into(),
        command: Some("--password=credential-sentinel".into()),
        user: Some("service".into()),
        status: "running".into(),
        cpu_usage_percent: 1.0,
        cpu_usage_single_core_percent: 2.0,
        memory_bytes: 1024,
        memory_usage_percent: 0.1,
        start_time_unix_seconds: 100,
        terminable: false,
        backend_process: true,
    };
    assert!(to_value(&record).unwrap()["command"]
        .as_str()
        .unwrap()
        .contains("credential-sentinel"));
    let c = context("system.runtime_processes.view");
    let value = SystemInterfaceOutput::ProcessList(SystemRuntimeProcessListResponse {
        process_total: 1,
        processes: vec![record],
    })
    .project_document_output(&c)
    .unwrap();
    assert_eq!(value["processes"][0]["name"], "worker");
    assert_eq!(value["processes"][0]["pid"], 42);
    assert!(value["processes"][0].get("command").is_none());
    assert!(
        SystemInterfaceOutput::document_output_schema(&c).unwrap()["properties"]["processes"]
            ["items"]["properties"]
            .get("command")
            .is_none()
    );
}
#[test]
fn runtime_input_does_not_project_locale_headers() {
    let c = context("system.runtime_profile.view");
    let input = SystemInterfaceInput::RuntimeProfile {
        query_locale: None,
        explicit_header_locale: Some("header-sentinel".into()),
        accept_language: Some("header-sentinel".into()),
    };
    assert_eq!(
        input.project_document_input(&c).unwrap(),
        json!({"query":{}})
    );
}
