use super::*;
fn operation(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/plugins/families/:provider_code/upgrade-latest".into(),
    }
}
#[test]
fn optional_upgrade_body_is_absent_when_request_did_not_supply_one() {
    let op = operation("plugins.families.upgrade");
    assert_eq!(
        PluginInterfaceInput::UpgradeLatest {
            provider_code: "provider-a".into(),
            body: None
        }
        .project_document_input(&op)
        .unwrap(),
        json!({"path":{"provider_code":"provider-a"}})
    );
    let input = PluginInterfaceInput::ModelUpgradeLatest {
        provider_code: "provider-a".into(),
        body: Some(UpgradeLatestPluginFamilyBody {
            compatibility_override: None,
            risk_override: Some(PluginRiskOverrideBody {
                reason: "Accepted risk".into(),
                acknowledged_warnings: vec!["unsigned".into()],
            }),
        }),
    };
    assert_eq!(
        input.project_document_input(&op).unwrap()["body"]["risk_override"],
        json!({"reason":"Accepted risk","acknowledged_warnings":["unsigned"]})
    );
}
#[test]
fn catalog_keeps_documented_locale_and_binary_upload_only_disables_request_phase() {
    let op = operation("plugins.official_catalog.view");
    let input = PluginInterfaceInput::ListOfficial {
        query: OfficialPluginCatalogQuery {
            plugin_type: None,
            locale: Some("zh_Hans".into()),
            q: Some("catalog".into()),
            cursor: None,
            limit: None,
        },
        locale: ConsoleLocaleHints::from_headers(&axum::http::HeaderMap::new()),
    };
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"query":{"locale":"zh_Hans","q":"catalog"}})
    );
    let upload = operation("model_provider_plugins.install.upload");
    assert_eq!(
        PluginInterfaceInput::document_input_unavailable(&upload),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    );
    assert_eq!(
        PluginInterfaceOutput::document_output_unavailable(&upload),
        None
    );
}
#[test]
fn plugin_task_metadata_is_actual_business_json_with_no_summary_limit() {
    let op = operation("plugins.tasks.view");
    let tasks = (0..75).map(|i|PluginTaskResponse { id:format!("task-{i}"), installation_id:None, workspace_id:None, provider_code:"provider-a".into(), task_kind:"install".into(), status:"succeeded".into(), status_message:Some("Result".repeat(200)), detail_json:json!({"secret_label":"Public field title","items":(0..90).collect::<Vec<_>>()}), created_at:"2026-11-07".into(), updated_at:"2026-11-07".into(), finished_at:None }).collect::<Vec<_>>();
    let expected = serde_json::to_value(&tasks).unwrap();
    assert_eq!(
        PluginInterfaceOutput::Tasks(tasks)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
}
