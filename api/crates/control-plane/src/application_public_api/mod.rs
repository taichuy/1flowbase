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
pub mod workflow_start_http_inputs;

use crate::{
    application::{effective_application_row_scope, ensure_application_console_row_scope},
    ports::ApplicationRepository,
};

pub(crate) async fn ensure_application_view_permission<R>(
    repository: &R,
    actor: &domain::ActorContext,
    application: &domain::ApplicationRecord,
) -> anyhow::Result<()>
where
    R: ApplicationRepository,
{
    if actor.is_root {
        return Ok(());
    }
    let policies = repository
        .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
        .await?;
    ensure_application_console_row_scope(
        actor,
        application,
        effective_application_row_scope(&policies, access_control::APPLICATIONS_VIEW_OPERATION_ID),
    )
    .map_err(Into::into)
}

pub(crate) async fn ensure_application_edit_permission<R>(
    repository: &R,
    actor: &domain::ActorContext,
    application: &domain::ApplicationRecord,
) -> anyhow::Result<()>
where
    R: ApplicationRepository,
{
    if actor.is_root {
        return Ok(());
    }
    let policies = repository
        .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
        .await?;
    ensure_application_console_row_scope(
        actor,
        application,
        effective_application_row_scope(
            &policies,
            access_control::APPLICATIONS_UPDATE_OPERATION_ID,
        ),
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
pub use test_support::{
    ApplicationPublicApiTestCache, ApplicationPublicApiTestHarness,
    ApplicationPublicApiTestRepository,
};
