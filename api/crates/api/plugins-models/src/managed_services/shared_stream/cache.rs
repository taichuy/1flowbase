use super::broker::SourceKey;
use crate::error_response::ApiError;
use control_plane::errors::ControlPlaneError as Error;
use control_plane_contracts::ports::EphemeralKvStore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct CachedEvent {
    pub revision: String,
    pub expires_unix_ms: i64,
    pub event: Value,
}
pub(super) fn now_ms() -> i64 {
    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
}
pub(super) fn ttl_ms(interval: u64) -> Result<i64, ApiError> {
    interval
        .checked_mul(3)
        .and_then(|v| i64::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| Error::InvalidInput("managed_service_interval").into())
}
pub(super) fn key(source: &SourceKey, revision: &str) -> String {
    // Opaque hashes keep user record IDs and resolver fingerprints out of inspection keys.
    let bytes = serde_json::to_vec(&(source, revision)).expect("source key serializes");
    let mut encoded = [0u8; 64];
    format!(
        "host:managed-service:shared-latest:v1:{}",
        base16ct::lower::encode_str(&Sha256::digest(bytes), &mut encoded)
            .expect("SHA256 hex buffer")
    )
}
fn projection_key(source: &SourceKey, revision: &str, operation: &str) -> String {
    let mut scoped = source.clone();
    scoped.interface = format!("{}:projection:{}", source.interface, operation);
    key(&scoped, revision)
}
pub(super) async fn put_projection(
    store: &dyn EphemeralKvStore,
    source: &SourceKey,
    revision: &str,
    operation: &str,
    interval: u64,
    value: Value,
) -> Result<(), ApiError> {
    let mut scoped = source.clone();
    scoped.interface = format!("{}:projection:{}", source.interface, operation);
    put(store, &scoped, revision, interval, value).await
}
pub(super) async fn get_projection(
    store: &dyn EphemeralKvStore,
    source: &SourceKey,
    revision: &str,
    operation: &str,
) -> Result<Option<Value>, ApiError> {
    let mut scoped = source.clone();
    scoped.interface = format!("{}:projection:{}", source.interface, operation);
    get(store, &scoped, revision).await
}
pub(super) async fn invalidate(
    store: &dyn EphemeralKvStore,
    source: &SourceKey,
    revision: &str,
    projections: &[String],
) -> Result<(), ApiError> {
    store
        .delete(&key(source, revision))
        .await
        .map_err(|_| Error::UpstreamUnavailable("managed_service_shared_cache"))?;
    invalidate_projections(store, source, revision, projections).await
}
pub(super) async fn invalidate_projections(
    store: &dyn EphemeralKvStore,
    source: &SourceKey,
    revision: &str,
    projections: &[String],
) -> Result<(), ApiError> {
    for operation in projections {
        store
            .delete(&projection_key(source, revision, operation))
            .await
            .map_err(|_| Error::UpstreamUnavailable("managed_service_shared_cache"))?;
    }
    Ok(())
}
pub(super) async fn put(
    store: &dyn EphemeralKvStore,
    source: &SourceKey,
    revision: &str,
    interval: u64,
    event: Value,
) -> Result<(), ApiError> {
    let ttl = ttl_ms(interval)?;
    let envelope = CachedEvent {
        revision: revision.to_owned(),
        expires_unix_ms: now_ms()
            .checked_add(ttl)
            .ok_or(Error::InvalidInput("managed_service_interval"))?,
        event,
    };
    store
        .set_json(
            &key(source, revision),
            serde_json::to_value(envelope)?,
            Some(time::Duration::milliseconds(ttl)),
        )
        .await
        .map_err(|_| Error::UpstreamUnavailable("managed_service_shared_cache").into())
}
pub(super) async fn get(
    store: &dyn EphemeralKvStore,
    source: &SourceKey,
    revision: &str,
) -> Result<Option<Value>, ApiError> {
    let value = store
        .get_json(&key(source, revision))
        .await
        .map_err(|_| Error::UpstreamUnavailable("managed_service_shared_cache"))?;
    let envelope = value.and_then(|v| serde_json::from_value::<CachedEvent>(v).ok());
    Ok(envelope
        .filter(|v| v.revision == revision && v.expires_unix_ms > now_ms())
        .map(|v| v.event))
}
