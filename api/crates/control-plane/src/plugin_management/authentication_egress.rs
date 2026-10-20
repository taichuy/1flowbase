//! Narrow service-authority admission for public authentication HTTP exchanges.
use control_plane_contracts::ports::{
    ContributionAuthorityLease, PluginContributionAuthorityRepository,
};
use extension_contracts::HostServiceError;
use runtime_core::runtime_backend::HttpEgressBinding;

pub async fn authorize_authentication_egress(
    authority: &dyn PluginContributionAuthorityRepository,
    binding: &HttpEgressBinding,
) -> Result<Box<dyn ContributionAuthorityLease>, HostServiceError> {
    let denied = || HostServiceError::new("authentication_egress_authority_denied");
    let installation_id =
        uuid::Uuid::parse_str(binding.subject.installation_id().as_str()).map_err(|_| denied())?;
    let lease = authority
        .lock_authentication_http_authority(&binding.subject)
        .await
        .map_err(|_| denied())?;
    let installation = lease.installation(installation_id).ok_or_else(denied)?;
    validate_authentication_egress(binding, installation, lease.snapshot())?;
    Ok(lease)
}

pub(crate) fn validate_authentication_egress(
    binding: &HttpEgressBinding,
    installation: &domain::PluginInstallationRecord,
    snapshot: &domain::PluginContributionAuthoritySnapshot,
) -> Result<(), HostServiceError> {
    let denied = || HostServiceError::new("authentication_egress_authority_denied");
    if binding.caller.is_some()
        || binding.subject.workspace_id().as_str() != domain::SYSTEM_SCOPE_ID.to_string()
        || binding.deadline_unix_ms
            <= (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
    {
        return Err(denied());
    }
    if installation.id.to_string() != binding.subject.installation_id().as_str()
        || installation.desired_state != domain::PluginDesiredState::ActiveRequested
        || installation.organization != binding.publisher_namespace
        || installation.provider_code != binding.plugin_code
        || installation.plugin_version != binding.plugin_version
        || domain::managed_installation_scope(installation, domain::DEFAULT_SCOPE_ID)
            != domain::SYSTEM_SCOPE_ID
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
        .find(|contribution| &contribution.contribution_id == binding.subject.contribution_id())
        .ok_or_else(denied)?;
    if contribution.point_id.as_str() != extension_contracts::AUTHENTICATION_POINT {
        return Err(denied());
    }
    let permissions = super::HostContributionGrantPolicy::root_composition()
        .effective_permissions(
            installation,
            domain::SYSTEM_SCOPE_ID,
            contribution,
            snapshot,
        )
        .map_err(|_| denied())?;
    for required in [
        extension_contracts::AUTHENTICATION_PERMISSION,
        "network.http.request",
    ] {
        if !permissions
            .iter()
            .any(|permission| permission.as_str() == required)
        {
            return Err(denied());
        }
    }
    Ok(())
}
