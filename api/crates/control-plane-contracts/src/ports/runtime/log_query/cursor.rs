use super::*;
use sha2::{Digest, Sha256};

/// Application order is authorization-set order, not part of query meaning.
/// Limit is intentionally excluded so callers may change page size.
pub fn application_log_query_fingerprint(
    scope_id: Uuid,
    application_ids: &[Uuid],
    query: &ApplicationLogRecordsQuery,
) -> String {
    let mut applications = application_ids.to_vec();
    applications.sort();
    applications.dedup();
    fingerprint((
        "records",
        scope_id,
        applications,
        &query.filter,
        &query.sort_field,
        query.descending,
    ))
}
pub fn record_trajectory_query_fingerprint(
    application_id: Uuid,
    record_id: Uuid,
    query: &RecordClientTrajectoryQuery,
    sections: &[String],
) -> String {
    fingerprint((
        "trajectory",
        application_id,
        record_id,
        &query.filter,
        &query.keyword,
        sections,
    ))
}
fn fingerprint(value: impl std::fmt::Debug) -> String {
    format!("{:x}", Sha256::digest(format!("{value:?}").as_bytes()))
}
pub fn validate_log_query_cursor_binding(
    version: u8,
    binding: &str,
    expected: &str,
) -> anyhow::Result<()> {
    if version != 1 || binding != expected {
        return Err(
            crate::ControlPlaneContractError::InvalidInput("log_query_cursor_mismatch").into(),
        );
    }
    Ok(())
}
