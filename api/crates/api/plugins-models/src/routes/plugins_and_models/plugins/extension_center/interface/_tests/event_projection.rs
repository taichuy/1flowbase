use super::*;
fn operation(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/settings/extension-center/install".into(),
    }
}
#[test]
fn extension_install_risk_outcome_matches_real_error_body_without_internal_enum_wrapper() {
    let op = operation("extension_center.install");
    let challenge = ExtensionRiskChallengeResponse {
        warnings: vec![ExtensionRiskWarningResponse {
            code: "unsigned".into(),
            message: "Unverified artifact".into(),
            overridable: true,
        }],
        compatibility: None,
    };
    let output = ExtensionCenterOutput::Install(ExtensionInstallOutcome::Challenge(challenge));
    assert_eq!(
        output.project_document_output(&op).unwrap(),
        json!({"status":409,"code":"extension_risk_confirmation_required","message":"Extension installation requires risk confirmation.","risk_challenge":{"warnings":[{"code":"unsigned","message":"Unverified artifact","overridable":true}],"compatibility":null}})
    );
    let schema = ExtensionCenterOutput::document_output_schema(&op)
        .unwrap()
        .to_string();
    assert!(schema.contains("risk_challenge"));
    assert!(schema.contains("installation"));
}
#[test]
fn managed_state_keeps_contract_identity_lineage_and_all_delivery_rows() {
    use control_plane_contracts::ports::{
        ManagedExecutionState, ManagedFrozenExecutionTarget, ManagedLifecycleDelivery,
    };
    let op = operation("extension_center.managed_execution.view");
    let state = ManagedExecutionState {
        installation_id: Uuid::nil(),
        workspace_id: Uuid::nil(),
        executions: vec![],
        deliveries: (0..70)
            .map(|i| ManagedLifecycleDelivery {
                event_id: Uuid::nil(),
                contract_id: format!("contract-{i}"),
                contract_version: "1".into(),
                subscriber_id: format!("subscriber-{i}"),
                target: ManagedFrozenExecutionTarget {
                    graph_fingerprint: "fingerprint".repeat(100),
                    handler_id: "handler-a".into(),
                    handler_version: "2".into(),
                },
                status: "paused".into(),
                pause_reason: Some("Explicit reason".into()),
                ownership: "owner-a".into(),
            })
            .collect(),
        deliveries_truncated: true,
        next_cursor: Some("business-pagination-cursor".into()),
    };
    let expected = serde_json::to_value(&state).unwrap();
    assert_eq!(
        ExtensionCenterOutput::ManagedExecution(state)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
    let schema = ExtensionCenterOutput::document_output_schema(&op)
        .unwrap()
        .to_string();
    assert!(schema.contains("contract_id"));
    assert!(schema.contains("contract_version"));
    assert!(schema.contains("next_cursor"));
}
#[test]
fn extension_catalog_filters_keep_only_documented_parameters_and_upload_remains_binary() {
    let op = operation("extension_center.catalog.view");
    let input = ExtensionCenterInput::ListCatalog {
        category: "mcp".into(),
        query: ExtensionCatalogGatewayQuery {
            slot_code: None,
            q: Some("connector".into()),
            limit: Some(20),
            cursor: None,
        },
    };
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"path":{"category":"mcp"},"query":{"q":"connector","limit":20}})
    );
    let upload = operation("extension_center.install.upload");
    assert_eq!(
        ExtensionCenterInput::document_input_unavailable(&upload),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    );
    assert_eq!(
        ExtensionCenterOutput::document_output_unavailable(&upload),
        None
    );
}
