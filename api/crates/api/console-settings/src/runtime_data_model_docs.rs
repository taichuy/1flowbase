use crate::error_response::ApiError;
pub use api_host_kernel::runtime_data_model_docs::*;
use control_plane::{errors::ControlPlaneError, model_definition::ModelDefinitionService};
use uuid::Uuid;
pub async fn ready_models_with(
    store: storage_durable_postgres::MainDurableStore,
    actor_user_id: Uuid,
) -> Result<Vec<domain::ModelDefinitionRecord>, ApiError> {
    let models = match ModelDefinitionService::new(store)
        .list_models(actor_user_id)
        .await
    {
        Ok(models) => models,
        Err(error) => {
            if let Some(ControlPlaneError::PermissionDenied(_)) =
                error.downcast_ref::<ControlPlaneError>()
            {
                return Ok(vec![]);
            }
            return Err(error.into());
        }
    };
    let mut models = models
        .into_iter()
        .filter(|model| model.status == domain::DataModelStatus::Published)
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.code.cmp(&right.code));
    Ok(models)
}
pub async fn ready_model_with(
    store: storage_durable_postgres::MainDurableStore,
    actor_user_id: Uuid,
    model_id: Uuid,
) -> Result<Option<domain::ModelDefinitionRecord>, ApiError> {
    let model = match ModelDefinitionService::new(store)
        .get_model(actor_user_id, model_id)
        .await
    {
        Ok(model) => model,
        Err(error) => {
            if let Some(ControlPlaneError::PermissionDenied(_) | ControlPlaneError::NotFound(_)) =
                error.downcast_ref::<ControlPlaneError>()
            {
                return Ok(None);
            }
            return Err(error.into());
        }
    };
    if model.status != domain::DataModelStatus::Published {
        return Ok(None);
    }
    Ok(Some(model))
}
