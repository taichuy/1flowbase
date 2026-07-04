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
    pub data_model_id: Uuid,
    pub scope_kind: domain::DataModelScopeKind,
    pub scope_id: Uuid,
    pub enabled: bool,
    pub permission_profile: domain::ScopeDataModelPermissionProfile,
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

    match grant.permission_profile {
        domain::ScopeDataModelPermissionProfile::Owner => {
            ensure_actor_in_granted_scope(actor, grant.scope_id)?;
            Ok(RuntimeAccessScope {
                scope_id: Some(grant.scope_id),
                owner_user_id: (!matches!(action, RuntimeDataAction::Create))
                    .then_some(actor.user_id),
            })
        }
        domain::ScopeDataModelPermissionProfile::ScopeAll => {
            ensure_actor_in_granted_scope(actor, grant.scope_id)?;
            Ok(RuntimeAccessScope {
                scope_id: Some(grant.scope_id),
                owner_user_id: None,
            })
        }
        domain::ScopeDataModelPermissionProfile::SystemAll => {
            if actor.is_root {
                Ok(RuntimeAccessScope {
                    scope_id: None,
                    owner_user_id: None,
                })
            } else {
                Err(RuntimeAclError::PermissionDenied(
                    "system_all_requires_system_actor",
                ))
            }
        }
    }
}

fn ensure_actor_in_granted_scope(
    actor: &ActorContext,
    scope_id: Uuid,
) -> Result<(), RuntimeAclError> {
    if actor.is_root || actor.current_workspace_id == scope_id {
        Ok(())
    } else {
        Err(RuntimeAclError::PermissionDenied(
            "data_model_scope_not_granted",
        ))
    }
}
