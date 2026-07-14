use anyhow::Result;
use domain::ActorContext;
use uuid::Uuid;

use crate::{errors::ControlPlaneError, ports::AuthRepository};

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
    R: AuthRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn authorize_view(&self, actor_user_id: Uuid) -> Result<SystemRuntimeAccess> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        if !actor.has_permission("system_runtime.view.all")
            && !actor
                .has_permission(access_control::SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_PERMISSION)
        {
            return Err(ControlPlaneError::PermissionDenied("system_runtime.view.all").into());
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
