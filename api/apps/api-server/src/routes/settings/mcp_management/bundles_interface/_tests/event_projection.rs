use super::*;
fn operation(id: &str, method: &str, path: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: method.into(),
        path: path.into(),
    }
}
#[test]
fn binary_classification_is_per_binding_and_preserves_json_siblings() {
    let upload = operation(
        "mcp.bundles.preview",
        "POST",
        "/api/console/mcp/bundles/preview-upload",
    );
    let json = operation(
        "mcp.bundles.preview",
        "POST",
        "/api/console/mcp/bundles/preview-official",
    );
    assert_eq!(
        McpBundlesInput::document_input_unavailable(&upload),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    );
    assert_eq!(McpBundlesInput::document_input_unavailable(&json), None);
    let archive = operation(
        "mcp.bundles.export",
        "POST",
        "/api/console/mcp/bundles/export",
    );
    let defaults = operation(
        "mcp.bundles.export",
        "GET",
        "/api/console/mcp/bundles/export-defaults",
    );
    assert_eq!(
        McpBundlesOutput::document_output_unavailable(&archive),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    );
    assert_eq!(
        McpBundlesOutput::document_output_unavailable(&defaults),
        None
    );
    assert_eq!(
        McpBundlesOutput::ExportDefaults(McpBundleExportDefaults {
            minimum_host_version: "0.6.0".into(),
            current_system_version: "0.6.3".into()
        })
        .project_document_output(&defaults)
        .unwrap(),
        json!({"minimum_host_version":"0.6.0","current_system_version":"0.6.3"})
    );
}
#[test]
fn official_source_and_library_bindings_keep_real_body_and_path_fields() {
    let op = operation(
        "mcp.bundles.preview",
        "POST",
        "/api/console/mcp/bundles/preview-official",
    );
    let value = McpBundlesInput::PreviewOfficial(McpBundleSourceBody::OfficialCatalog(
        OfficialMcpBundleSelector {
            organization: "publisher".into(),
            bundle_id: "catalog".into(),
        },
    ))
    .project_document_input(&op)
    .unwrap();
    assert_eq!(
        value,
        json!({"body":{"organization":"publisher","bundle_id":"catalog"}})
    );
    let op = operation(
        "mcp.bundle_library.sync",
        "POST",
        "/api/console/mcp/bundles/library/:organization/:bundle_id/sync",
    );
    let value = McpBundlesInput::SyncLibrary {
        organization: "publisher".into(),
        bundle_id: "catalog".into(),
        body: McpBundleLibraryVersionBody {
            bundle_version: Some("1.2.3".into()),
        },
    }
    .project_document_input(&op)
    .unwrap();
    assert_eq!(
        value,
        json!({"path":{"organization":"publisher","bundle_id":"catalog"},"body":{"bundle_version":"1.2.3"}})
    );
}
