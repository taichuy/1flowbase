use super::*;

impl HostContributionGrantPolicy {
    /// Root's explicit enable authorizes only declared permissions supported by this host.
    fn declared_activation_grants(
        &self,
        installation: &domain::PluginInstallationRecord,
        manifest: &plugin_framework::PluginManifestV1,
    ) -> Result<Vec<GrantContributionPermission>> {
        let managed = manifest
            .managed
            .as_ref()
            .ok_or(ControlPlaneError::InvalidInput(
                "managed_contribution_declaration",
            ))?;
        let collections = manifest
            .data_models
            .iter()
            .flat_map(|model| &model.owned_collections)
            .map(|collection| ContributionResourceScope::OwnedCollection {
                collection_code: collection.collection_code.clone(),
            })
            .collect::<Vec<_>>();
        let mut grants = Vec::new();
        for contribution in &managed.module.contributions {
            for permission in &contribution.required_permissions {
                let permission = permission.as_str();
                let mut candidates = self
                    .rules
                    .iter()
                    .filter(|rule| rule.permission == permission)
                    .map(|rule| (rule.permission_contract_id, rule.resource_scope.clone()))
                    .collect::<Vec<_>>();
                let contract = match permission {
                    "service.execute" => Some("managed-service"),
                    "file.read.content" => Some("file-read"),
                    "file.import.resource" => Some("file-import"),
                    "application.logs.read" | "application.logs.import" => Some("application-logs"),
                    "network.http.request" => Some("http-egress"),
                    "credential.manage" => Some("plugin-credential"),
                    "event.subscribe" | "event.publish" => Some("managed-event"),
                    "plugin_data.owned.write" => Some("plugin-data"),
                    _ => plugin_framework::extension_bus::managed_interface_point_phase(
                        contribution.point_id.as_str(),
                    )
                    .map(|_| "managed-interface"),
                };
                if let Some(contract) = contract {
                    let scopes = if permission == "plugin_data.owned.write" {
                        collections.clone()
                    } else {
                        vec![
                            ContributionResourceScope::System,
                            ContributionResourceScope::Workspace,
                        ]
                    };
                    candidates.extend(scopes.into_iter().map(|scope| (contract, scope)));
                }
                let accepted = candidates
                    .into_iter()
                    .map(|(contract, resource_scope)| GrantContributionPermission {
                        contribution_id: contribution.contribution_id.as_str().to_string(),
                        permission: permission.to_string(),
                        resource_scope,
                        permission_contract_id: contract.to_string(),
                        permission_contract_version: "1".into(),
                    })
                    .filter(|request| self.admits(installation, contribution, request))
                    .collect::<Vec<_>>();
                if accepted.is_empty() {
                    return Err(ControlPlaneError::PermissionDenied(
                        "contribution_permission_not_delegable",
                    )
                    .into());
                }
                grants.extend(accepted);
            }
        }
        Ok(grants)
    }
}

impl<R> PluginContributionAuthorityService<R>
where
    R: PluginContributionAuthorityRepository + PluginRepository + RoleConsolePolicyReader,
{
    /// No separate grant interaction for root. Runtime callbacks retain durable, scoped authority.
    /// The caller supplies a manifest read from the verified, installed artifact.
    pub async fn prepare_activation_authority(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
        manifest: &plugin_framework::PluginManifestV1,
    ) -> Result<()> {
        let installation = self.managed_installation(installation_id).await?;
        if manifest.publisher_namespace != installation.organization
            || manifest.plugin_code()? != installation.provider_code
            || manifest.version != installation.plugin_version
            || serde_json::to_value(&manifest.managed)? != installation.metadata_json["managed"]
        {
            return Err(
                ControlPlaneError::Conflict("managed_contribution_identity_mismatch").into(),
            );
        }
        // Validate the entire declaration before persisting any grant. Existing exact grants
        // are reused; retries after a failed activation are idempotent at the authority owner.
        let grants = self
            .host_policy
            .declared_activation_grants(&installation, manifest)?;
        // The caller has already authorized enable/switch. Inspecting approved capabilities
        // here must not require the separate authorization-view operation.
        let scope_id =
            domain::managed_installation_scope(&installation, actor.current_workspace_id);
        let mut snapshot = if actor.is_root {
            self.query(actor, installation_id).await?
        } else {
            self.repository
                .query_contribution_authority(
                    installation_id,
                    scope_id,
                    &audit_log(
                        Some(actor.current_workspace_id),
                        Some(actor.user_id),
                        "plugin_installation",
                        Some(installation_id),
                        "plugin.activation_authority.checked",
                        json!({}),
                    ),
                )
                .await
                .map_err(|error| {
                    if matches!(
                        error.downcast_ref::<ControlPlaneError>(),
                        Some(ControlPlaneError::PermissionDenied(
                            "contribution_workspace_history_required"
                        ))
                    ) {
                        anyhow::Error::from(ControlPlaneError::PermissionDenied(
                            "managed_contribution_authorization_required",
                        ))
                    } else {
                        error
                    }
                })?
        };
        for grant in grants {
            if snapshot.authorizations.iter().any(|existing| {
                existing.status == domain::ContributionAuthorizationStatus::Active
                    && existing.contribution_id == grant.contribution_id
                    && existing.permission == grant.permission
                    && existing.resource_scope == grant.resource_scope
                    && existing.permission_contract_id == grant.permission_contract_id
                    && existing.permission_contract_version == grant.permission_contract_version
            }) {
                continue;
            }
            if !actor.is_root {
                return Err(ControlPlaneError::PermissionDenied(
                    "managed_contribution_authorization_required",
                )
                .into());
            }
            snapshot = self.grant(actor, installation_id, grant).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "_tests/activation.rs"]
mod tests;
