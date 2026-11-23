use std::fmt;

use domain::ActorContext;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDataAction {
    View,
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAccessScope {
    pub scope_id: Option<Uuid>,
    pub owner_user_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeScopeGrant {
    /// Trusted installation metadata, not supplied by a runtime request.
    pub bootstrap_workspace_id: Uuid,
    pub data_model_id: Uuid,
    pub scope_kind: domain::DataModelScopeKind,
    pub scope_id: Uuid,
    pub enabled: bool,
    pub permission_profile: domain::ScopeDataModelPermissionProfile,
}

impl RuntimeScopeGrant {
    pub fn allows_system_rows(&self, actor: &ActorContext) -> bool {
        actor.is_root
            && (actor.current_workspace_id == domain::SYSTEM_SCOPE_ID
                || actor.current_workspace_id == self.bootstrap_workspace_id)
    }

    fn is_system_grant(&self) -> bool {
        self.scope_kind == domain::DataModelScopeKind::System
            && self.scope_id == domain::SYSTEM_SCOPE_ID
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAclError {
    PermissionDenied(&'static str),
}

impl fmt::Display for RuntimeAclError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied(reason) => write!(f, "permission denied: {reason}"),
        }
    }
}

impl std::error::Error for RuntimeAclError {}

pub fn resolve_access_scope(
    actor: &ActorContext,
    action: RuntimeDataAction,
    data_model_id: Uuid,
    scope_grant: Option<&RuntimeScopeGrant>,
) -> Result<RuntimeAccessScope, RuntimeAclError> {
    let grant = scope_grant.ok_or(RuntimeAclError::PermissionDenied(
        "data_model_scope_not_granted",
    ))?;
    if !grant.enabled || grant.data_model_id != data_model_id {
        return Err(RuntimeAclError::PermissionDenied(
            "data_model_scope_not_granted",
        ));
    }

    let may_access_system_rows = grant.allows_system_rows(actor);
    let scope_id = match grant.permission_profile {
        domain::ScopeDataModelPermissionProfile::SystemAll => {
            if may_access_system_rows {
                None
            } else if actor.is_root
                || (action == RuntimeDataAction::View && grant.is_system_grant())
            {
                Some(actor.current_workspace_id)
            } else {
                return Err(RuntimeAclError::PermissionDenied(
                    "system_all_requires_system_actor",
                ));
            }
        }
        _ if may_access_system_rows || grant.scope_id == actor.current_workspace_id => {
            Some(grant.scope_id)
        }
        _ if actor.is_root && grant.is_system_grant() => Some(actor.current_workspace_id),
        _ => {
            return Err(RuntimeAclError::PermissionDenied(
                "data_model_scope_not_granted",
            ))
        }
    };
    Ok(RuntimeAccessScope {
        scope_id,
        owner_user_id: (grant.permission_profile == domain::ScopeDataModelPermissionProfile::Owner
            && action != RuntimeDataAction::Create)
            .then_some(actor.user_id),
    })
}
