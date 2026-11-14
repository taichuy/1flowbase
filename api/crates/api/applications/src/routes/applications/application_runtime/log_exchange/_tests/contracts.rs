use super::*;

#[test]
fn exchange_managed_projection_excludes_credentials_destinations_and_facts() {
    let id = Uuid::now_v7();
    let secret = "PRIVATE_SOURCE_VALUE";
    let input = ExchangeInput::OtlpImport {
        application_id: id,
        body: OtlpImportBody {
            otlp: json!({"raw":secret}),
        },
    };
    let projection = input.project_for_managed_hook().unwrap();
    assert_eq!(projection.as_object().unwrap().len(), 3);
    assert!(!projection.to_string().contains(secret));
    let output = ExchangeOutput::Otlp(json!({"raw":secret}));
    assert!(!output
        .project_for_managed_hook()
        .unwrap()
        .to_string()
        .contains(secret));
}

#[test]
fn exchange_has_two_independent_operations_and_strict_bodies() {
    let assembly = route_assembly();
    assert_eq!(assembly.bindings().len(), 2);
    for (binding, operation) in assembly.bindings().iter().zip(OPERATIONS) {
        assert_eq!(binding.route.method, "POST");
        assert_eq!(
            binding.ownership,
            access_control::ConsoleRouteOwnership::ConsoleOperation((*operation).into())
        );
    }
    assert!(serde_json::from_value::<OtlpImportBody>(
        json!({"otlp":{},"secret_key":"unregistered"})
    )
    .is_err());
}

#[tokio::test]
async fn partial_import_error_reports_safe_committed_counts() {
    use axum::response::IntoResponse;
    let response = ApiError(
        control_plane::agent_logs::AgentLogsPartialImportError {
            accepted_events: 2,
            duplicate_events: 1,
            committed_records: 1,
        }
        .into(),
    )
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["code"], "agent_logs_partial_import");
    assert_eq!(
        value["details"],
        json!({"accepted_events":2,"duplicate_events":1,"committed_records":1})
    );
}

#[test]
fn console_import_event_projection_keeps_application_and_typed_facts_without_external_terminal() {
    let application_id = Uuid::now_v7();
    let batch: control_plane::ports::AgentLogsBatch = serde_json::from_value(json!({
        "schema_version":"1flowbase.agent-logs/v1","source_id":"collector","source_client":"fixture",
        "events":[{"event_id":"e1","source_session_id":"s1","source_task_id":"t1",
            "sequence":1,"occurred_at":"2026-11-07T00:00:00Z","kind":"user","content":"question",
            "raw":{"credential":"PRIVATE_RAW_SECRET"}}]
    })).unwrap();
    batch.validate().unwrap();
    let otlp = crate::agent_logs_protocol::batches_to_traces(&[batch]).unwrap();
    let input = ExchangeInput::OtlpImport {
        application_id,
        body: OtlpImportBody {
            otlp: serde_json::to_value(otlp).unwrap(),
        },
    };
    let value = input.project_for_event().unwrap();
    assert_eq!(value["application_id"], application_id.to_string());
    assert_eq!(value["action"], "otlp_import");
    assert_eq!(value["batches"][0]["events"][0]["content"], "question");
    assert_eq!(value["batches"][0]["events"][0]["kind"], "user");
    assert!(!value.to_string().contains("PRIVATE_RAW_SECRET"));
    assert!(
        jsonschema::validator_for(&ExchangeInput::event_projection_schema().unwrap())
            .unwrap()
            .is_valid(&value)
    );
    let record_id = Uuid::now_v7();
    let output = ExchangeOutput::Imported(ImportReceipt {
        accepted_events: 0,
        duplicate_events: 2,
        record_ids: vec![record_id],
    });
    assert_eq!(
        output.project_for_event(),
        Some(json!({"accepted_events":0,"duplicate_events":2,"record_ids":[record_id]}))
    );
    assert!(
        jsonschema::validator_for(&ExchangeOutput::event_projection_schema().unwrap())
            .unwrap()
            .is_valid(&output.project_for_event().unwrap())
    );
    assert!(ExchangeInput::OtlpExport {
        application_id,
        body: OtlpExportBody {
            record_ids: Some(vec![record_id]),
            log_conversation_ids: None
        }
    }
    .project_for_event()
    .is_none());
    assert!(ExchangeOutput::Otlp(json!({"resourceSpans":[]}))
        .project_for_event()
        .is_none());
    assert!(ExchangeInput::OtlpImport {
        application_id,
        body: OtlpImportBody {
            otlp: json!({"resourceSpans":"opaque bytes"})
        }
    }
    .project_for_event()
    .is_none());
}

#[test]
fn console_log_exchange_only_declares_import_as_event_capable() {
    struct UnusedPort;
    impl ConsoleInterfacePort<ExchangeInput, ExchangeOutput> for UnusedPort {
        fn execute<'a>(
            &'a self,
            _: &'a UserPrincipal,
            _: ExchangeInput,
        ) -> ConsoleInterfaceFuture<'a, ExchangeOutput> {
            Box::pin(async {
                Err(ConsoleInterfaceTargetError(
                    control_plane::errors::ControlPlaneError::NotAuthenticated.into(),
                ))
            })
        }
    }
    let registry = console_interface::compile_registry_with_event_observation(
        "api-server.console-application-log-exchange", "graph:console-application-log-exchange-v1",
        super::super::interface::DECLARATIONS, Arc::new(UnusedPort),
        &[crate::routes::application_public_api::agent_logs::event_projection::CONSOLE_IMPORT_OPERATION],
    ).unwrap();
    let capabilities = registry
        .event_capabilities()
        .map(|entry| entry.definition().interface_id().as_str().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(capabilities, ["applications.logs.otlp.import"]);
}
