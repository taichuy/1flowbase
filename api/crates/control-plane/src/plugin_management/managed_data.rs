//! Current authority is held through every managed plugin data callback.
use super::HostContributionGrantPolicy;
use control_plane_contracts::ports::PluginContributionAuthorityRepository;
use domain::{
    ContributionAuthorizationStatus, ContributionResourceScope,
    PluginContributionAuthoritySnapshot, PluginDesiredState, PluginInstallationRecord,
};
use extension_contracts::{
    PluginDataBinding, PluginDataError, PluginDataErrorKind, PluginDataFuture, PluginDataOperation,
    PluginDataPermission, PluginDataPort, PluginDataRequest, PluginDataTarget,
};
use std::sync::Arc;
use uuid::Uuid;

pub struct ManagedPluginDataService {
    authority: Arc<dyn PluginContributionAuthorityRepository>,
    inner: Arc<dyn PluginDataPort>,
}
impl ManagedPluginDataService {
    pub fn new(
        authority: Arc<dyn PluginContributionAuthorityRepository>,
        inner: Arc<dyn PluginDataPort>,
    ) -> Self {
        Self { authority, inner }
    }
}
fn denied() -> PluginDataError {
    PluginDataError {
        kind: PluginDataErrorKind::PermissionDenied,
        code: "plugin_data_authority_denied".into(),
        retryable: false,
    }
}
fn deadline() -> PluginDataError {
    PluginDataError {
        kind: PluginDataErrorKind::DeadlineExceeded,
        code: "plugin_data_deadline".into(),
        retryable: false,
    }
}
impl PluginDataPort for ManagedPluginDataService {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginDataBinding,
        request: &'a PluginDataRequest,
    ) -> PluginDataFuture<'a> {
        Box::pin(async move {
            // Existing providers do not carry managed contribution authority.
            let Some(subject) = binding.managed_subject.as_ref() else {
                return self.inner.execute(binding, request).await;
            };
            request.validate()?;
            let installation_id =
                Uuid::parse_str(subject.installation_id().as_str()).map_err(|_| denied())?;
            let remaining = i128::from(binding.deadline_unix_ms)
                - time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
            if remaining <= 0 {
                return Err(deadline());
            }
            tokio::time::timeout(std::time::Duration::from_millis(remaining as u64), async {
                let lease = self
                    .authority
                    .lock_contribution_authority(subject)
                    .await
                    .map_err(|_| denied())?;
                let installation = lease.installation(installation_id).ok_or_else(denied)?;
                validate_authority(binding, request, installation, lease.snapshot())?;
                // Preserve the lease across asynchronous storage access; revoke/disable serialize here.
                let result = self.inner.execute(binding, request).await;
                lease.release().await.map_err(|_| denied())?;
                result
            })
            .await
            .map_err(|_| deadline())?
        })
    }
}
fn validate_authority(
    binding: &PluginDataBinding,
    request: &PluginDataRequest,
    installation: &PluginInstallationRecord,
    snapshot: &PluginContributionAuthoritySnapshot,
) -> Result<(), PluginDataError> {
    let subject = binding.managed_subject.as_ref().ok_or_else(denied)?;
    let scope = Uuid::parse_str(&binding.workspace_id).map_err(|_| denied())?;
    if subject.installation_id().as_str() != installation.id.to_string()
        || subject.workspace_id().as_str() != binding.workspace_id
        || installation.desired_state != PluginDesiredState::ActiveRequested
        || installation.contract_version != "1flowbase.extension-bus/v1"
        || installation.organization != binding.publisher_namespace
        || installation.provider_code != binding.plugin_code
        || installation.plugin_version != binding.plugin_version
        || domain::managed_installation_scope(installation, scope) != scope
        || binding.storage_binding != "main"
        || installation
            .metadata_json
            .pointer("/managed_service_permissions/storage")
            .and_then(serde_json::Value::as_str)
            != Some("host_managed")
    {
        return Err(denied());
    }
    let managed: plugin_framework::ManagedManifest =
        serde_json::from_value(installation.metadata_json["managed"].clone())
            .map_err(|_| denied())?;
    if managed.module.module_id.as_str() != binding.plugin_code
        || managed.module.module_version.as_str() != binding.plugin_version
    {
        return Err(denied());
    }
    let contribution = managed
        .module
        .contributions
        .iter()
        .find(|c| c.contribution_id == *subject.contribution_id())
        .ok_or_else(denied)?;
    let effective = HostContributionGrantPolicy::root_composition()
        .effective_permissions(installation, scope, contribution, snapshot)
        .map_err(|_| denied())?;
    for operation in &request.operations {
        if !binding.permissions.contains(&operation.permission()) {
            return Err(denied());
        }
        let target = match operation {
            PluginDataOperation::Find { target, .. }
            | PluginDataOperation::FindOne { target, .. }
            | PluginDataOperation::Count { target, .. }
            | PluginDataOperation::Insert { target, .. }
            | PluginDataOperation::Update { target, .. }
            | PluginDataOperation::Delete { target, .. }
            | PluginDataOperation::Upsert { target, .. } => target,
        };
        let PluginDataTarget::OwnedCollection { collection_code } = target else {
            return Err(denied());
        };
        let allowed = snapshot.authorizations.iter().any(|grant| {
            grant.installation_id == installation.id && grant.workspace_id == scope
                && grant.contribution_id == subject.contribution_id().as_str()
                && grant.point_id == contribution.point_id.as_str()
                && grant.status == ContributionAuthorizationStatus::Active
                && grant.permission_contract_id == "plugin-data" && grant.permission_contract_version == "1"
                && effective.iter().any(|p| p.as_str() == grant.permission)
                && (grant.permission == "plugin_data.owned.write" || (operation.permission() == PluginDataPermission::Read && grant.permission == "plugin_data.owned.read"))
                && matches!(&grant.resource_scope, ContributionResourceScope::OwnedCollection { collection_code: granted } if granted == collection_code)
        });
        if !allowed {
            return Err(denied());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "_tests/managed_data_tests.rs"]
mod tests;
