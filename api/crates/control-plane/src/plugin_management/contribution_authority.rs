use anyhow::Result;
use domain::{ActorContext, ContributionResourceScope, PluginContributionAuthoritySnapshot};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        GrantContributionAuthorizationInput, PluginContributionAuthorityRepository,
        PluginRepository, RevokeContributionAuthorizationInput, RoleConsolePolicyReader,
    },
};

pub const CONTRIBUTION_AUTHORIZATION_GRANT: &str =
    "extension_center.contribution_authorizations.grant";
pub const CONTRIBUTION_AUTHORIZATION_REVOKE: &str =
    "extension_center.contribution_authorizations.revoke";
pub const CONTRIBUTION_AUTHORIZATION_VIEW: &str =
    "extension_center.contribution_authorizations.view";

#[derive(Debug, Clone)]
pub struct GrantContributionPermission {
    pub contribution_id: String,
    pub permission: String,
    pub resource_scope: ContributionResourceScope,
    pub permission_contract_id: String,
    pub permission_contract_version: String,
}
#[derive(Debug, Clone)]
pub struct RevokeContributionPermission {
    pub authorization_id: Uuid,
    pub expected_revision: i64,
}

#[derive(Debug, Clone)]
struct HostContributionPermissionRule {
    point_id: String,
    permission: String,
    permission_contract_id: &'static str,
    point_contract_version: &'static str,
    resource_scope: ContributionResourceScope,
    publisher_artifact: Option<&'static str>,
}

/// Compiled host grant ceilings, never constructed from package permissions or extension points.
#[derive(Debug, Clone)]
pub struct HostContributionGrantPolicy {
    rules: Vec<HostContributionPermissionRule>,
}

impl HostContributionGrantPolicy {
    pub fn root_composition() -> Self {
        let mut rules = [
            "authorization",
            "admission",
            "before",
            "after",
            "failure",
            "completion",
        ]
        .into_iter()
        .map(|phase| HostContributionPermissionRule {
            point_id: format!("1flowbase.model-definitions.create.{phase}"),
            permission: format!("hook.model_definitions.create.{phase}"),
            permission_contract_id: "managed-hook",
            point_contract_version: "1",
            resource_scope: ContributionResourceScope::Workspace,
            publisher_artifact: None,
        })
        .collect::<Vec<_>>();
        for permission in ["event.publish", "event.subscribe"] {
            rules.push(HostContributionPermissionRule {
                point_id: "acme.composition-a.processed".into(),
                permission: permission.into(),
                permission_contract_id: "managed-event",
                point_contract_version: "1",
                resource_scope: ContributionResourceScope::Workspace,
                publisher_artifact: (permission == "event.publish").then_some("acme.composition-a"),
            });
        }
        rules.push(HostContributionPermissionRule {
            point_id: "1flowbase.plugin-data.owned-collection".into(),
            permission: "plugin_data.owned.write".into(),
            permission_contract_id: "plugin-data",
            point_contract_version: "1flowbase.plugin-data-model/v1",
            resource_scope: ContributionResourceScope::OwnedCollection {
                collection_code: "processed_models".into(),
            },
            publisher_artifact: Some("acme.composition-a"),
        });
        Self { rules }
    }

    fn admits(
        &self,
        installation: &domain::PluginInstallationRecord,
        contribution: &plugin_framework::extension_bus::ContributionDescriptor,
        request: &GrantContributionPermission,
    ) -> bool {
        request.permission_contract_version == "1"
            && self.rules.iter().any(|rule| {
                rule.point_id == contribution.point_id.as_str()
                    && rule.point_contract_version == contribution.contract_version.as_str()
                    && rule.permission == request.permission
                    && rule.permission_contract_id == request.permission_contract_id
                    && rule.resource_scope == request.resource_scope
                    && rule.publisher_artifact.is_none_or(|artifact| {
                        installation.organization == "acme"
                            && installation.provider_code == artifact
                    })
            })
    }
}

pub struct PluginContributionAuthorityService<R> {
    repository: R,
    host_policy: HostContributionGrantPolicy,
}
impl<R> PluginContributionAuthorityService<R>
where
    R: PluginContributionAuthorityRepository + PluginRepository + RoleConsolePolicyReader,
{
    pub fn new(repository: R, host_policy: HostContributionGrantPolicy) -> Self {
        Self {
            repository,
            host_policy,
        }
    }

    pub async fn grant(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
        request: GrantContributionPermission,
    ) -> Result<PluginContributionAuthoritySnapshot> {
        self.ensure_operation(actor, CONTRIBUTION_AUTHORIZATION_GRANT)
            .await?;
        let installation = self.scoped_installation(actor, installation_id).await?;
        let managed: plugin_framework::ManagedManifest =
            serde_json::from_value(installation.metadata_json.get("managed").cloned().ok_or(
                ControlPlaneError::InvalidInput("managed_contribution_declaration"),
            )?)?;
        if managed.module.module_id.as_str() != installation.provider_code
            || managed.module.module_version.as_str() != installation.plugin_version
        {
            return Err(
                ControlPlaneError::Conflict("managed_contribution_identity_mismatch").into(),
            );
        }
        let contribution = managed
            .module
            .contributions
            .iter()
            .find(|contribution| contribution.contribution_id.as_str() == request.contribution_id)
            .ok_or(ControlPlaneError::NotFound("managed_contribution"))?;
        if !self
            .host_policy
            .admits(&installation, contribution, &request)
            || !contribution
                .required_permissions
                .iter()
                .any(|permission| permission.as_str() == request.permission)
        {
            return Err(ControlPlaneError::PermissionDenied(
                "contribution_permission_not_delegable",
            )
            .into());
        }
        let event = audit_log(
            Some(actor.current_workspace_id),
            Some(actor.user_id),
            "plugin_installation",
            Some(installation_id),
            "plugin.contribution_authorization.granted",
            json!({
                "contribution_id": request.contribution_id, "point_id": contribution.point_id,
                "permission": request.permission, "resource_scope": request.resource_scope,
                "permission_contract_id": request.permission_contract_id, "permission_contract_version": request.permission_contract_version,
            }),
        );
        self.repository
            .grant_contribution_authorization(&GrantContributionAuthorizationInput {
                expected_installation_updated_at: installation.updated_at,
                installation_id,
                workspace_id: actor.current_workspace_id,
                contribution_id: request.contribution_id,
                point_id: contribution.point_id.as_str().to_string(),
                permission: request.permission,
                resource_scope: request.resource_scope,
                permission_contract_id: request.permission_contract_id,
                permission_contract_version: request.permission_contract_version,
                actor_user_id: actor.user_id,
                audit_log: event,
            })
            .await
    }

    pub async fn revoke(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
        request: RevokeContributionPermission,
    ) -> Result<PluginContributionAuthoritySnapshot> {
        self.ensure_operation(actor, CONTRIBUTION_AUTHORIZATION_REVOKE)
            .await?;
        self.scoped_installation(actor, installation_id).await?;
        if request.expected_revision < 0 {
            return Err(ControlPlaneError::InvalidInput("expected_revision").into());
        }
        self.repository.revoke_contribution_authorization(&RevokeContributionAuthorizationInput {
            installation_id, workspace_id: actor.current_workspace_id, authorization_id: request.authorization_id,
            expected_revision: request.expected_revision, actor_user_id: actor.user_id,
            audit_log: audit_log(Some(actor.current_workspace_id), Some(actor.user_id), "plugin_installation", Some(installation_id), "plugin.contribution_authorization.revoked", json!({"authorization_id": request.authorization_id, "expected_revision": request.expected_revision})),
        }).await
    }

    pub async fn query(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
    ) -> Result<PluginContributionAuthoritySnapshot> {
        self.ensure_operation(actor, CONTRIBUTION_AUTHORIZATION_VIEW)
            .await?;
        self.scoped_installation(actor, installation_id).await?;
        self.repository
            .query_contribution_authority(
                installation_id,
                actor.current_workspace_id,
                &audit_log(
                    Some(actor.current_workspace_id),
                    Some(actor.user_id),
                    "plugin_installation",
                    Some(installation_id),
                    "plugin.contribution_authorization.viewed",
                    json!({}),
                ),
            )
            .await
    }

    async fn ensure_operation(&self, actor: &ActorContext, operation: &str) -> Result<()> {
        if actor.is_root {
            return Ok(());
        }
        let policies = self
            .repository
            .load_role_console_policies_for_user(actor)
            .await?;
        let group = domain::ConsolePolicyGroup::settings_feature("system.extension-center")?;
        let operation = domain::ConsoleOperationId::try_from(operation)?;
        if domain::effective_console_simple_operation(&policies, &group, &operation) {
            Ok(())
        } else {
            Err(ControlPlaneError::PermissionDenied("permission_denied").into())
        }
    }

    async fn scoped_installation(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
    ) -> Result<domain::PluginInstallationRecord> {
        let installation = self
            .repository
            .get_installation(installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        if installation.category == domain::ExtensionCategory::HostExtensions
            || !self
                .repository
                .list_assignments(actor.current_workspace_id)
                .await?
                .iter()
                .any(|assignment| assignment.installation_id == installation_id)
        {
            return Err(ControlPlaneError::PermissionDenied(
                "contribution_workspace_assignment_required",
            )
            .into());
        }
        Ok(installation)
    }
}
