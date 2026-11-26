//! Caller-scoped host policy for plugin-declared record resources.
use crate::errors::ControlPlaneError;
use anyhow::Result;
use control_plane_contracts::ports::{RecordPermissionRepository, RoleConsolePolicyReader};
use domain::{
    ActorContext, ConsoleOperationId, ConsolePolicyGroup, RecordPermissionCapabilities,
    RecordPermissionKey, RecordPermissionMode, RecordPermissionPolicy, RecordPermissionResourceKey,
    RecordPermissionRole, ReplaceRecordPermissions,
};
use std::collections::BTreeSet;

pub const RECORD_PERMISSIONS_VIEW_OPERATION_ID: &str = "roles.record_permissions.view";
pub const RECORD_PERMISSIONS_REPLACE_OPERATION_ID: &str = "roles.record_permissions.replace";
const MAX_GRANTS: usize = 256;

pub struct RecordPermissionService<R> {
    repository: R,
}
impl<R> RecordPermissionService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    pub fn repository(&self) -> &R {
        &self.repository
    }
}

impl<R: RecordPermissionRepository + RoleConsolePolicyReader> RecordPermissionService<R> {
    fn validate_key(actor: &ActorContext, key: &RecordPermissionKey) -> Result<()> {
        if key.workspace_id != actor.current_workspace_id {
            return Err(ControlPlaneError::PermissionDenied("record_permission_workspace").into());
        }
        if key.record_id.is_empty()
            || key.record_id.len() > 1024
            || key.record_id.chars().any(char::is_control)
            || [
                &key.resource.publisher_namespace,
                &key.resource.plugin_code,
                &key.resource.resource_code,
            ]
            .into_iter()
            .any(|value| {
                value.is_empty() || value.len() > 255 || value.chars().any(char::is_control)
            })
        {
            return Err(ControlPlaneError::InvalidInput("record_permission_key").into());
        }
        Ok(())
    }

    async fn has_operation(&self, actor: &ActorContext, operation: &str) -> Result<bool> {
        if actor.is_root {
            return Ok(true);
        }
        let policies = self
            .repository
            .load_role_console_policies_for_user(actor)
            .await?;
        Ok(Self::allows_operation(&policies, operation))
    }

    fn allows_operation(policies: &[domain::RoleConsolePolicy], operation: &str) -> bool {
        let group =
            ConsolePolicyGroup::settings_feature(access_control::SYSTEM_ROLES_SETTINGS_FEATURE_ID)
                .expect("compiled roles group");
        let operation =
            ConsoleOperationId::try_from(operation).expect("compiled record permissions operation");
        domain::effective_console_simple_operation(policies, &group, &operation)
    }

    async fn require_operation(&self, actor: &ActorContext, operation: &str) -> Result<()> {
        if self.has_operation(actor, operation).await? {
            Ok(())
        } else {
            Err(ControlPlaneError::PermissionDenied("permission_denied").into())
        }
    }

    pub async fn can_manage_permissions(&self, actor: &ActorContext) -> Result<bool> {
        if actor.is_root {
            return Ok(true);
        }
        let policies = self
            .repository
            .load_role_console_policies_for_user(actor)
            .await?;
        Ok(
            Self::allows_operation(&policies, RECORD_PERMISSIONS_VIEW_OPERATION_ID)
                && Self::allows_operation(&policies, RECORD_PERMISSIONS_REPLACE_OPERATION_ID),
        )
    }

    pub async fn get(
        &self,
        actor: &ActorContext,
        key: &RecordPermissionKey,
    ) -> Result<RecordPermissionPolicy> {
        Self::validate_key(actor, key)?;
        self.require_operation(actor, RECORD_PERMISSIONS_VIEW_OPERATION_ID)
            .await?;
        Ok(self
            .repository
            .get_record_permissions(key)
            .await?
            .unwrap_or_default())
    }

    pub async fn roles(&self, actor: &ActorContext) -> Result<Vec<RecordPermissionRole>> {
        if !self
            .has_operation(actor, RECORD_PERMISSIONS_VIEW_OPERATION_ID)
            .await?
            && !self
                .has_operation(actor, RECORD_PERMISSIONS_REPLACE_OPERATION_ID)
                .await?
        {
            return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
        }
        self.repository
            .list_record_permission_roles(actor.current_workspace_id)
            .await
    }

    pub async fn replace(
        &self,
        actor: &ActorContext,
        key: &RecordPermissionKey,
        input: &ReplaceRecordPermissions,
        declared_actions: &[String],
    ) -> Result<RecordPermissionPolicy> {
        Self::validate_key(actor, key)?;
        self.require_operation(actor, RECORD_PERMISSIONS_REPLACE_OPERATION_ID)
            .await?;
        if input.grants.len() > MAX_GRANTS
            || input.expected_revision.is_some_and(|revision| revision < 0)
            || (input.mode == RecordPermissionMode::Inherit && !input.grants.is_empty())
        {
            return Err(ControlPlaneError::InvalidInput("record_permission_policy").into());
        }
        let roles = self
            .repository
            .list_record_permission_roles(actor.current_workspace_id)
            .await?;
        let mut seen_roles = BTreeSet::new();
        for grant in &input.grants {
            if !seen_roles.insert(grant.role_id)
                || !roles.iter().any(|role| role.role_id == grant.role_id)
            {
                return Err(ControlPlaneError::InvalidInput("record_permission_role").into());
            }
            let mut seen_actions = BTreeSet::new();
            if grant
                .actions
                .iter()
                .any(|action| !declared_actions.contains(action) || !seen_actions.insert(action))
            {
                return Err(ControlPlaneError::InvalidInput("record_permission_action").into());
            }
        }
        self.repository
            .replace_record_permissions(key, input, actor.user_id)
            .await
    }

    /// coarse admission and row policy are independent requirements; root comes from Host actor.
    pub async fn authorize(
        &self,
        actor: &ActorContext,
        key: &RecordPermissionKey,
        action: &str,
        operation_allowed: bool,
    ) -> Result<()> {
        Self::validate_key(actor, key)?;
        if actor.is_root {
            return Ok(());
        }
        if !operation_allowed {
            return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
        }
        let policy = self
            .repository
            .get_record_permissions(key)
            .await?
            .unwrap_or_default();
        let roles = self
            .repository
            .record_permission_effective_role_ids(actor)
            .await?;
        if policy.allows(action, &roles) {
            Ok(())
        } else {
            Err(ControlPlaneError::PermissionDenied("record_permission_denied").into())
        }
    }

    /// eligible_actions must come from Host operation authorization for this declared resource.
    pub async fn capabilities(
        &self,
        actor: &ActorContext,
        key: &RecordPermissionKey,
        eligible_actions: &[String],
    ) -> Result<RecordPermissionCapabilities> {
        Self::validate_key(actor, key)?;
        let allowed_actions = if actor.is_root {
            eligible_actions.to_vec()
        } else {
            let policy = self
                .repository
                .get_record_permissions(key)
                .await?
                .unwrap_or_default();
            let roles = self
                .repository
                .record_permission_effective_role_ids(actor)
                .await?;
            eligible_actions
                .iter()
                .filter(|action| policy.allows(action, &roles))
                .cloned()
                .collect()
        };
        Ok(RecordPermissionCapabilities {
            record_id: key.record_id.clone(),
            allowed_actions,
            can_manage_permissions: self.can_manage_permissions(actor).await?,
        })
    }

    pub async fn capabilities_batch(
        &self,
        actor: &ActorContext,
        resource: &RecordPermissionResourceKey,
        record_ids: &[String],
        eligible_actions: &[String],
    ) -> Result<Vec<RecordPermissionCapabilities>> {
        if record_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut key = RecordPermissionKey {
            resource: resource.clone(),
            workspace_id: actor.current_workspace_id,
            record_id: String::new(),
        };
        for id in record_ids {
            key.record_id.clone_from(id);
            Self::validate_key(actor, &key)?;
        }
        let (policies, roles) = if actor.is_root {
            (Default::default(), Vec::new())
        } else {
            let policies = self
                .repository
                .get_record_permissions_batch(actor.current_workspace_id, resource, record_ids)
                .await?;
            let roles = self
                .repository
                .record_permission_effective_role_ids(actor)
                .await?;
            (policies, roles)
        };
        let can_manage_permissions = self.can_manage_permissions(actor).await?;
        Ok(record_ids
            .iter()
            .map(|id| RecordPermissionCapabilities {
                record_id: id.clone(),
                allowed_actions: eligible_actions
                    .iter()
                    .filter(|action| {
                        policies
                            .get(id)
                            .is_none_or(|policy| policy.allows(action, &roles))
                    })
                    .cloned()
                    .collect(),
                can_manage_permissions,
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
