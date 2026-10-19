use super::*;
use crate::routes::_tests::time_response_cases::assert_timestamp_mapping;

#[test]
fn model_catalog_response_timestamp_contract() {
    assert_timestamp_mapping(|at| {
        let response = to_model_catalog_response(ModelProviderModelCatalog {
            provider_instance_id: Uuid::nil(),
            refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
            source: domain::ModelProviderCatalogSource::Static,
            last_error_message: None,
            refreshed_at: Some(at),
            models: vec![],
        })?;
        Ok(vec![response
            .refreshed_at
            .expect("present catalog timestamp")])
    });
    assert_eq!(format_optional_time(None).unwrap(), None);
}
