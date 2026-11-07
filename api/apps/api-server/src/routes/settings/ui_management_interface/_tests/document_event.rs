use super::*;
fn context(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "GET".into(),
        path: "/fixture".into(),
    }
}
#[test]
fn catalog_download_projects_json_while_retained_asset_is_non_json() {
    let c = context("ui_management.catalog.download");
    assert!(UiManagementInput::document_input_unavailable(&c).is_none());
    assert!(UiManagementOutput::document_output_unavailable(&c).is_none());
    assert!(
        UiManagementOutput::document_output_schema(&c).is_none(),
        "inherit authentic catalog DTO schema"
    );
    let record = CatalogComponentResponse {
        component_code: "chart".into(),
        name: "Actual chart".into(),
        description: "Actual description".into(),
        import_code: "import Chart from './Chart'".into(),
        source_code: "export default function Chart() {}".into(),
        source: "official".into(),
        group: "visual".into(),
        upstream: super::super::super::ui_management::ComponentUpstreamBody {
            identity: "chart".into(),
            version: "1".into(),
        },
        version: "1".into(),
        keywords: vec!["chart".into()],
        catalog_updated_at: "2030-01-01".into(),
        source_locator: "catalog/chart".into(),
        source_checksum: "sha256:actual".into(),
        local_version: None,
    };
    let value = UiManagementOutput::CatalogComponent(record)
        .project_document_output(&c)
        .unwrap();
    assert_eq!(value["name"], "Actual chart");
    assert_eq!(value["source_code"], "export default function Chart() {}");
    let asset = context("ui_management.plugin_settings_module_asset.view");
    assert!(UiManagementInput::document_input_unavailable(&asset).is_none());
    assert!(matches!(
        UiManagementOutput::document_output_unavailable(&asset),
        Some(ApiEventUnavailableReason::BinaryResponse)
    ));
}
