use super::*;
use crate::routes::_tests::time_response_cases::assert_timestamp_mapping;

#[test]
fn resource_catalog_response_timestamp_contract() {
    assert_timestamp_mapping(|at| {
        let response = to_resources_response(DataSourceResourcesView {
            entries: vec![],
            refresh_status: domain::DataSourceCatalogRefreshStatus::Ready,
            last_error_message: None,
            refreshed_at: Some(at),
        })?;
        Ok(vec![response
            .refreshed_at
            .expect("present catalog timestamp")])
    });
    let response = to_resources_response(DataSourceResourcesView {
        entries: vec![],
        refresh_status: domain::DataSourceCatalogRefreshStatus::Idle,
        last_error_message: None,
        refreshed_at: None,
    })
    .unwrap();
    assert_eq!(response.refreshed_at, None);
}
