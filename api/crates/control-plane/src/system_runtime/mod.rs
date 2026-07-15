use anyhow::Result;
use domain::ActorContext;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{AuthRepository, RoleConsolePolicyReader},
};

const SYSTEM_RUNTIME_PROFILE_VIEW_OPERATION_ID: &str = "system.runtime_profile.view";

#[derive(Debug)]
pub struct SystemRuntimeAccess {
    pub actor: ActorContext,
    pub preferred_locale: Option<String>,
}

pub struct SystemRuntimeService<R> {
    repository: R,
}

impl<R> SystemRuntimeService<R>
where
    R: AuthRepository + RoleConsolePolicyReader,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn authorize_view(&self, actor_user_id: Uuid) -> Result<SystemRuntimeAccess> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
                .await?;
            let operation_id =
                domain::ConsoleOperationId::try_from(SYSTEM_RUNTIME_PROFILE_VIEW_OPERATION_ID)
                    .expect("compiled system runtime operation id must be valid");
            let group = domain::ConsolePolicyGroup::settings_feature(
                access_control::SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_ID,
            )
            .expect("compiled system runtime settings feature id must be valid");
            if !domain::effective_console_simple_operation(&policies, &group, &operation_id) {
                return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
            }
        }

        let user = self
            .repository
            .find_user_by_id(actor_user_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("user"))?;

        Ok(SystemRuntimeAccess {
            actor,
            preferred_locale: user.preferred_locale,
        })
    }
}
