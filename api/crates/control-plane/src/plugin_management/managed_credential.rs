//! Live authority facade for outbound plugin-owned credentials.
use super::{execute_with_leased_system_service_authority, SystemServiceCallback};
use control_plane_contracts::ports::{
    PluginContributionAuthorityRepository, PluginCredentialRepository,
};
use extension_contracts::{
    PluginCredentialBinding, PluginCredentialError, PluginCredentialFuture, PluginCredentialPort,
    PluginCredentialRequest,
};
use std::sync::Arc;

pub struct ManagedPluginCredentialService {
    authority: Arc<dyn PluginContributionAuthorityRepository>,
    repository: Arc<dyn PluginCredentialRepository>,
}
impl ManagedPluginCredentialService {
    pub fn new(
        authority: Arc<dyn PluginContributionAuthorityRepository>,
        repository: Arc<dyn PluginCredentialRepository>,
    ) -> Self {
        Self {
            authority,
            repository,
        }
    }
}
fn denied() -> PluginCredentialError {
    PluginCredentialError::new("plugin_credential_authority_denied")
}
fn deadline_exceeded() -> PluginCredentialError {
    PluginCredentialError::new("plugin_credential_deadline_exceeded")
}
impl PluginCredentialPort for ManagedPluginCredentialService {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginCredentialBinding,
        request: &'a PluginCredentialRequest,
    ) -> PluginCredentialFuture<'a> {
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
                    permission: "credential.manage",
                },
                denied,
                deadline_exceeded,
                |_, installation, installations| {
                    let resolved = super::resolve_plugin_credential_binding(
                        installation,
                        installations,
                        binding,
                        request,
                    )
                    .map_err(|_| denied())?;
                    Ok(async move {
                        self.repository
                            .execute_plugin_credential(&resolved, request)
                            .await
                    })
                },
            )
            .await
        })
    }
}
