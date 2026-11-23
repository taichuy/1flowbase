//! Live authority checks only; RuntimeExtensionHost owns all scheduling and permits.
use super::HostContributionGrantPolicy;
use control_plane_contracts::ports::PluginContributionAuthorityRepository;
use extension_contracts::{
    PluginExecutionGateAuthority, PluginExecutionGateAuthorityFuture, PluginExecutionGateBinding,
    PluginExecutionGateError,
};
use std::sync::Arc;

pub struct ManagedPluginExecutionGateAuthority {
    authority: Arc<dyn PluginContributionAuthorityRepository>,
}
impl ManagedPluginExecutionGateAuthority {
    pub fn new(authority: Arc<dyn PluginContributionAuthorityRepository>) -> Self {
        Self { authority }
    }
}
fn denied() -> PluginExecutionGateError {
    PluginExecutionGateError::new("execution_gate_authority_denied")
}
impl PluginExecutionGateAuthority for ManagedPluginExecutionGateAuthority {
    fn authorize<'a>(
        &'a self,
        binding: &'a PluginExecutionGateBinding,
    ) -> PluginExecutionGateAuthorityFuture<'a> {
        Box::pin(async move {
            let installation_id = uuid::Uuid::parse_str(binding.subject.installation_id().as_str())
                .map_err(|_| denied())?;
            let scope_id = uuid::Uuid::parse_str(binding.subject.workspace_id().as_str())
                .map_err(|_| denied())?;
            // The actual workspace namespaces queues, while the subject selects installation authority.
            uuid::Uuid::parse_str(&binding.workspace_id).map_err(|_| denied())?;
            let remaining = i128::from(binding.deadline_unix_ms)
                - time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
            if remaining <= 0 {
                return Err(PluginExecutionGateError::new(
                    "execution_gate_deadline_exceeded",
                ));
            }
            let future = async {
                let lease = self
                    .authority
                    .lock_managed_callback_authority(&binding.subject)
                    .await
                    .map_err(|_| denied())?;
                let installation = lease.installation(installation_id).ok_or_else(denied)?;
                if installation.desired_state != domain::PluginDesiredState::ActiveRequested
                    || installation.organization != binding.publisher_namespace
                    || installation.provider_code != binding.plugin_code
                    || installation.plugin_version != binding.plugin_version
                    || domain::managed_installation_scope(installation, domain::DEFAULT_SCOPE_ID)
                        != scope_id
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
                    .find(|c| c.contribution_id == *binding.subject.contribution_id())
                    .ok_or_else(denied)?;
                if !matches!(
                    contribution.point_id.as_str(),
                    plugin_framework::MANAGED_SERVICE_POINT
                        | extension_contracts::WORKSPACE_POLICY_POINT
                ) || !contribution
                    .required_permissions
                    .iter()
                    .any(|p| p.as_str() == "execution.gate")
                {
                    return Err(denied());
                }
                HostContributionGrantPolicy::root_composition()
                    .effective_permissions(installation, scope_id, contribution, lease.snapshot())
                    .map_err(|_| denied())?;
                lease.release().await.map_err(|_| denied())?;
                Ok(())
            };
            if binding.deadline_unix_ms == i64::MAX {
                future.await
            } else {
                tokio::time::timeout(std::time::Duration::from_millis(remaining as u64), future)
                    .await
                    .map_err(|_| {
                        PluginExecutionGateError::new("execution_gate_deadline_exceeded")
                    })?
            }
        })
    }
}
