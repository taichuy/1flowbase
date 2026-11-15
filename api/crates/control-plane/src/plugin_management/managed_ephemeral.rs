//! Live authority facade for plugin-owned ephemeral JSON state.
use super::HostContributionGrantPolicy;
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
impl PluginEphemeralPort for ManagedPluginEphemeralService {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginEphemeralBinding,
        request: &'a PluginEphemeralRequest,
    ) -> PluginEphemeralFuture<'a> {
        Box::pin(async move {
            request.validate()?;
            let installation_id =
                uuid::Uuid::parse_str(&binding.installation_id).map_err(|_| denied())?;
            let scope_id = uuid::Uuid::parse_str(&binding.scope_id).map_err(|_| denied())?;
            if scope_id != domain::SYSTEM_SCOPE_ID {
                return Err(denied());
            }
            let now_ms = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
            let remaining_ms = i128::from(binding.deadline_unix_ms) - now_ms;
            if remaining_ms <= 0 {
                return Err(denied());
            }
            let subject = plugin_framework::extension_bus::ManagedContributionSubject::new(
                plugin_framework::extension_bus::ManagedInstallationId::new(
                    &binding.installation_id,
                )
                .map_err(|_| denied())?,
                plugin_framework::extension_bus::ManagedWorkspaceId::new(&binding.scope_id)
                    .map_err(|_| denied())?,
                plugin_framework::extension_bus::ContributionId::new(&binding.contribution_id)
                    .map_err(|_| denied())?,
            );
            tokio::time::timeout(
                std::time::Duration::from_millis(remaining_ms as u64),
                async {
                    let lease = self
                        .authority
                        .lock_contribution_authority(&subject)
                        .await
                        .map_err(|_| denied())?;
                    let installation = lease.installation(installation_id).ok_or_else(denied)?;
                    if installation.desired_state != domain::PluginDesiredState::ActiveRequested
                        || installation.organization != binding.publisher_namespace
                        || installation.provider_code != binding.plugin_code
                        || installation.plugin_version != binding.plugin_version
                        || domain::managed_installation_scope(
                            installation,
                            domain::DEFAULT_SCOPE_ID,
                        ) != scope_id
                        || installation
                            .metadata_json
                            .pointer("/managed_service_permissions/secrets")
                            .and_then(serde_json::Value::as_str)
                            != Some("host_managed")
                    {
                        return Err(denied());
                    }
                    let managed: plugin_framework::ManagedManifest =
                        serde_json::from_value(installation.metadata_json["managed"].clone())
                            .map_err(|_| denied())?;
                    let contribution = managed
                        .module
                        .contributions
                        .iter()
                        .find(|c| c.contribution_id.as_str() == binding.contribution_id)
                        .ok_or_else(denied)?;
                    if contribution.point_id.as_str() != plugin_framework::MANAGED_SERVICE_POINT
                        || !contribution
                            .required_permissions
                            .iter()
                            .any(|p| p.as_str() == "ephemeral.manage")
                    {
                        return Err(denied());
                    }
                    HostContributionGrantPolicy::root_composition()
                        .effective_permissions(
                            installation,
                            scope_id,
                            contribution,
                            lease.snapshot(),
                        )
                        .map_err(|_| denied())?;
                    // Revocation and disable cannot cross the actual storage access.
                    // Recheck after the asynchronous lease acquisition, before touching storage.
                    if i128::from(binding.deadline_unix_ms)
                        <= time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000
                    {
                        return Err(PluginEphemeralError::new(
                            "plugin_ephemeral_deadline_exceeded",
                        ));
                    }
                    let key = format!(
                        "plugin_ephemeral:{installation_id}:{scope_id}:{}",
                        request.key()
                    );
                    let result = execute_storage(self.store.as_ref(), &key, request).await;
                    lease.release().await.map_err(|_| denied())?;
                    result
                },
            )
            .await
            .map_err(|_| PluginEphemeralError::new("plugin_ephemeral_deadline_exceeded"))?
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
