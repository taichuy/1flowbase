use super::*;
#[test]
fn collector_asset_has_real_path_metadata_and_binary_result_classification() {
    let op = ApiEventOperationContext {
        interface_id: INTERFACE_ID.into(),
        method: "GET".into(),
        path: "/api/public/client-collectors/:organization/:artifact_id/:version/assets/:asset"
            .into(),
    };
    let input = CollectorAssetInput {
        organization: "publisher".into(),
        artifact_id: "collector".into(),
        version: "1.0.0".into(),
        asset: "collector.js".into(),
    };
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"path":{"organization":"publisher","artifact_id":"collector","version":"1.0.0","asset":"collector.js"}})
    );
    assert_eq!(
        CollectorAssetOutput::document_output_unavailable(&op),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    );
}
