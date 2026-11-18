use serde_json::json;

use crate::application_public_api::{
    ApplicationApiMappingConfig, WorkflowExtensionHttpMethod, WorkflowExtensionResponseMode,
};

#[test]
fn root_1894_application_api_mapping_serde_shape_remains_stable() {
    let mapping: ApplicationApiMappingConfig = serde_json::from_value(json!({
        "input": {
            "query_target": "node-start.query",
            "model_target": null,
            "inputs_target": "node-start",
            "history_target": null,
            "attachments_target": null
        },
        "output": {
            "answer_selector": "node-end.answer",
            "usage_selector": null,
            "files_selector": null,
            "error_selector": null
        },
        "extension": {
            "slug": "reports/{report_id}",
            "method": "POST",
            "response_mode": "async"
        }
    }))
    .unwrap();

    let extension = mapping.extension.as_ref().unwrap();
    assert!(extension.invoke_role_codes.is_empty());
    assert_eq!(extension.method, WorkflowExtensionHttpMethod::Post);
    assert_eq!(
        extension.response_mode,
        WorkflowExtensionResponseMode::Async
    );
    assert_eq!(
        serde_json::to_value(mapping).unwrap()["extension"]["method"],
        "POST"
    );

    let native = ApplicationApiMappingConfig::default_native();
    assert_eq!(native.input.query_target, "node-start.query");
    assert!(native.extension.is_none());
}

#[test]
fn workflow_invocation_roles_round_trip_without_changing_legacy_snapshots() {
    use crate::application_public_api::WorkflowExtensionApiConfig;
    let original = json!({"slug":"demo/check","method":"POST","response_mode":"sync"});
    let legacy: WorkflowExtensionApiConfig = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(legacy).unwrap(), original);
    let configured = json!({"slug":"demo/check","method":"POST","response_mode":"sync", "invoke_role_codes":["demo_user"]});
    let config: WorkflowExtensionApiConfig = serde_json::from_value(configured.clone()).unwrap();
    assert_eq!(config.invoke_role_codes, ["demo_user"]);
    assert_eq!(serde_json::to_value(config).unwrap(), configured);
}
