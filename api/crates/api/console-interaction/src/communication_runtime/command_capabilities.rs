//! Command availability consumes the same published application policy as assistant runs.
use crate::{error_response::ApiError, routes::assistant::AssistantRunCapabilities};
use control_plane::errors::ControlPlaneError;
use domain::ActorContext;
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

pub(crate) async fn load(
    store: &MainDurableStore,
    actor: &ActorContext,
    application_id: Option<Uuid>,
) -> Result<(AssistantRunCapabilities, Option<String>), ApiError> {
    project(
        crate::routes::assistant::assistant_run_capabilities(store, actor, application_id).await,
    )
}

fn project(
    result: Result<AssistantRunCapabilities, ApiError>,
) -> Result<(AssistantRunCapabilities, Option<String>), ApiError> {
    let capabilities = match result {
        Ok(capabilities) => capabilities,
        // A saved channel remains editable if its publication is removed or access is revoked.
        Err(error)
            if matches!(
                error.0.downcast_ref::<ControlPlaneError>(),
                Some(ControlPlaneError::NotFound(_) | ControlPlaneError::PermissionDenied(_))
            ) =>
        {
            return Ok((
                AssistantRunCapabilities::default(),
                Some("application_unavailable".into()),
            ));
        }
        Err(error) => return Err(error),
    };
    let reason = if !capabilities.model_selection_enabled {
        Some("model_selection_unavailable".into())
    } else if capabilities.models.is_empty() {
        Some("no_models_available".into())
    } else {
        None
    };
    Ok((capabilities, reason))
}

#[cfg(test)]
#[path = "_tests/command_capabilities.rs"]
mod tests;
