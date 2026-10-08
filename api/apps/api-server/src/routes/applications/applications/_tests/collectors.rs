use super::*;
#[test]
fn collector_dto_projection_preserves_truthful_local_state_and_null_urls() {
    let collectors = vec![ApplicationCollectorResponse {
        collector_code: "codex-logs-collector".into(),
        source_client: "codex".into(),
        display_name: "Codex".into(),
        description: "Local logs".into(),
        version: "0.2.0".into(),
        execution_target: "client".into(),
        catalog_id: "runtime-extensions:taichuy/codex-logs-collector".into(),
        category: "runtime-extensions".into(),
        installation_status: "missing".into(),
        installed_version: Some("0.1.0".into()),
        extension_installation_id: Some("retained".into()),
        installable: false,
        can_install: false,
        can_update: false,
        asset_base_url: None,
        documentation_url: None,
        shell_installer_url: None,
        powershell_installer_url: None,
    }];
    let value = serde_json::to_value(&collectors[0]).unwrap();
    assert!(value["shell_installer_url"].is_null());
    assert_eq!(value["installation_status"], "missing");
    assert_eq!(value["version"], "0.2.0");
    assert_eq!(value["installed_version"], "0.1.0");
    let output = ApplicationsOutput::Catalog(ApplicationCatalogResponse {
        collectors,
        types: vec![],
        workflow_triggers: vec![],
        tags: vec![],
    });
    let projection = output.project_for_managed_hook().unwrap();
    let validator =
        jsonschema::validator_for(&ApplicationsOutput::managed_projection_schema().unwrap())
            .unwrap();
    assert!(validator.is_valid(&projection));
    let mut missing = projection.clone();
    missing["0"]["collectors"][0]
        .as_object_mut()
        .unwrap()
        .remove("installed_version");
    assert!(!validator.is_valid(&missing));
}
