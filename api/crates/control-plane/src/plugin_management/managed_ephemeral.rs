//! Live authority facade for plugin-owned ephemeral JSON state.
use super::{execute_with_leased_system_service_authority, SystemServiceCallback};
use control_plane_contracts::ports::{EphemeralKvStore, PluginContributionAuthorityRepository};
use extension_contracts::{
    PluginEphemeralBinding, PluginEphemeralError, PluginEphemeralFuture, PluginEphemeralPort,
    PluginEphemeralRequest,
};
use std::sync::Arc;

pub struct ManagedPluginEphemeralService {
    authority: Arc<dyn PluginContributionAuthorityRepository>,
    store: Arc<dyn EphemeralKvStore>,
}
impl ManagedPluginEphemeralService {
    pub fn new(
        authority: Arc<dyn PluginContributionAuthorityRepository>,
        store: Arc<dyn EphemeralKvStore>,
    ) -> Self {
        Self { authority, store }
    }
}
fn denied() -> PluginEphemeralError {
    PluginEphemeralError::new("plugin_ephemeral_authority_denied")
}
fn deadline_exceeded() -> PluginEphemeralError {
    PluginEphemeralError::new("plugin_ephemeral_deadline_exceeded")
}
impl PluginEphemeralPort for ManagedPluginEphemeralService {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginEphemeralBinding,
        request: &'a PluginEphemeralRequest,
    ) -> PluginEphemeralFuture<'a> {
        Box::pin(async move {
            request.validate()?;
            execute_with_leased_system_service_authority(
                self.authority.as_ref(),
                SystemServiceCallback {
                    installation_id: &binding.installation_id,
                    scope_id: &binding.scope_id,
                    contribution_id: &binding.contribution_id,
                    publisher_namespace: &binding.publisher_namespace,
                    plugin_code: &binding.plugin_code,
                    plugin_version: &binding.plugin_version,
                    deadline_unix_ms: binding.deadline_unix_ms,
                    permission: "ephemeral.manage",
                },
                denied,
                deadline_exceeded,
                |installation_id, _, _| {
                    // Recheck after asynchronous lease acquisition, before touching storage.
                    if i128::from(binding.deadline_unix_ms)
                        <= time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000
                    {
                        return Err(deadline_exceeded());
                    }
                    let key = format!(
                        "plugin_ephemeral:{installation_id}:{}:{}",
                        domain::SYSTEM_SCOPE_ID,
                        request.key()
                    );
                    Ok(async move { execute_storage(self.store.as_ref(), &key, request).await })
                },
            )
            .await
        })
    }
}

// Storage adapters own expiration; this service never persists a parallel durable copy.
async fn execute_storage(
    store: &dyn EphemeralKvStore,
    key: &str,
    request: &PluginEphemeralRequest,
) -> Result<extension_contracts::PluginEphemeralResponse, PluginEphemeralError> {
    let value = match request {
        PluginEphemeralRequest::Get { .. } => store.get_json(key).await,
        PluginEphemeralRequest::Delete { .. } => store.delete(key).await.map(|()| None),
        PluginEphemeralRequest::Put { value, ttl_ms, .. } => {
            let ttl = i64::try_from(*ttl_ms)
                .ok()
                .map(time::Duration::milliseconds)
                .filter(|ttl| *ttl > time::Duration::ZERO)
                .filter(|ttl| time::OffsetDateTime::now_utc().checked_add(*ttl).is_some())
                .ok_or_else(|| PluginEphemeralError::new("plugin_ephemeral_ttl_invalid"))?;
            store
                .set_json(key, value.clone(), Some(ttl))
                .await
                .map(|()| None)
        }
    }
    .map_err(|_| PluginEphemeralError::new("plugin_ephemeral_storage_failed"))?;
    Ok(extension_contracts::PluginEphemeralResponse { value })
}
