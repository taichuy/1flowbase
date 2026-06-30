pub mod api_keys;
pub mod callback_resume;
pub mod callback_tool_ids;
pub mod client_protocol_envelope;
pub mod compat;
pub mod conversations;
pub mod mapping;
pub mod model_catalog;
pub mod native;
pub mod publications;
pub mod run_service;
pub mod workflow_extension;
pub mod workflow_schedule;

use crate::errors::ControlPlaneError;

pub(crate) fn ensure_application_view_permission(
    actor: &domain::ActorContext,
    application: &domain::ApplicationRecord,
) -> std::result::Result<(), ControlPlaneError> {
    if actor.is_root || actor.has_permission("application.view.all") {
        return Ok(());
    }

    if actor.has_permission("application.view.own") && application.created_by == actor.user_id {
        return Ok(());
    }

    Err(ControlPlaneError::PermissionDenied("permission_denied"))
}

pub(crate) fn ensure_application_edit_permission(
    actor: &domain::ActorContext,
    application: &domain::ApplicationRecord,
) -> std::result::Result<(), ControlPlaneError> {
    if actor.is_root || actor.has_permission("application.edit.all") {
        return Ok(());
    }

    if actor.has_permission("application.edit.own") && application.created_by == actor.user_id {
        return Ok(());
    }

    Err(ControlPlaneError::PermissionDenied("permission_denied"))
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
pub use test_support::{
    ApplicationPublicApiTestCache, ApplicationPublicApiTestHarness,
    ApplicationPublicApiTestRepository,
};
