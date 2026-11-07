use super::*;
#[test]
fn dependency_catalog_has_no_artificial_request_locations_and_preserves_all_entries() {
    let op = ApiEventOperationContext {
        interface_id: "js_dependencies.view".into(),
        method: "GET".into(),
        path: "/api/console/js-dependencies".into(),
    };
    assert_eq!(
        JsDependenciesInput.project_document_input(&op).unwrap(),
        json!({})
    );
    let entries = (0..85)
        .map(|i| JsDependencyCatalogEntryResponse {
            installation_id: format!("installation-{i}"),
            provider_code: "provider-a".into(),
            plugin_id: "plugin-a".into(),
            plugin_version: "1".into(),
            alias: format!("dependency-{i}"),
            package: "package".repeat(100),
            version: "1.0.0".into(),
            target: "node".into(),
            artifact_path: "module.js".into(),
            integrity: "sha256-test".into(),
            permissions: JsDependencyPermissionsResponse {
                network: "allowed".into(),
                filesystem: "none".into(),
                env: "none".into(),
            },
        })
        .collect::<Vec<_>>();
    let expected = serde_json::to_value(&entries).unwrap();
    assert_eq!(
        JsDependenciesOutput(entries)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
}
