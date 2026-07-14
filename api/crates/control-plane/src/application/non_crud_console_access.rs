use crate::errors::ControlPlaneError;

use super::{
    effective_application_row_scope, ensure_application_console_row_scope,
    ensure_application_console_simple_operation,
};

/// Stable application operation identities whose route-level `Simple` grant must remain
/// intersected with the pre-console-policy application access rule.
///
/// The mapping is intentionally local to the application domain rather than the console
/// registry: the registry remains `Simple`, while this service boundary preserves the
/// persisted application owner/workspace checks that protected these paths before #1259.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationNonCrudConsoleOperation {
    ApiSetEnabled,
    Publish,
    Run,
    LogsExport,
    LogsImport,
    OrchestrationTemplateExport,
    OrchestrationTemplateImport,
    OrchestrationVersionRestore,
}

impl ApplicationNonCrudConsoleOperation {
    fn operation_id(self) -> &'static str {
        match self {
            Self::ApiSetEnabled => access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
            Self::Publish => access_control::APPLICATIONS_PUBLISH_OPERATION_ID,
            Self::Run => access_control::APPLICATIONS_RUN_OPERATION_ID,
            Self::LogsExport => access_control::APPLICATIONS_LOGS_EXPORT_OPERATION_ID,
            Self::LogsImport => access_control::APPLICATIONS_LOGS_IMPORT_OPERATION_ID,
            Self::OrchestrationTemplateExport => {
                access_control::APPLICATIONS_ORCHESTRATION_TEMPLATE_EXPORT_OPERATION_ID
            }
            Self::OrchestrationTemplateImport => {
                access_control::APPLICATIONS_ORCHESTRATION_TEMPLATE_IMPORT_OPERATION_ID
            }
            Self::OrchestrationVersionRestore => {
                access_control::APPLICATIONS_ORCHESTRATION_VERSION_RESTORE_OPERATION_ID
            }
        }
    }

    fn persisted_application_row_operation(self) -> Option<&'static str> {
        match self {
            // `ensure_application_edit_permission` guarded both paths before #1259.
            Self::ApiSetEnabled | Self::Publish => {
                Some(access_control::APPLICATIONS_UPDATE_OPERATION_ID)
            }
            // These paths previously entered through `ApplicationService::get_application` /
            // `ensure_application_visible`, which enforced the persisted view scope.
            Self::Run
            | Self::LogsExport
            | Self::LogsImport
            | Self::OrchestrationTemplateExport
            | Self::OrchestrationVersionRestore => {
                Some(access_control::APPLICATIONS_VIEW_OPERATION_ID)
            }
            // Import creates a fresh application and therefore has no target row.
            Self::OrchestrationTemplateImport => None,
        }
    }
}

/// Requires both the operation's `Simple` grant and its historic persisted-row prerequisite.
/// Callers must load the application by the actor's current workspace before invoking this
/// function; the workspace assertion is kept here as the domain boundary's defense in depth.
pub(crate) fn ensure_existing_application_non_crud_console_operation(
    actor: &domain::ActorContext,
    application: &domain::ApplicationRecord,
    policies: &[domain::RoleConsolePolicy],
    operation: ApplicationNonCrudConsoleOperation,
) -> Result<(), ControlPlaneError> {
    if application.workspace_id != actor.current_workspace_id {
        return Err(ControlPlaneError::NotFound("application"));
    }
    if actor.is_root {
        return Ok(());
    }

    let row_operation = operation
        .persisted_application_row_operation()
        .ok_or(ControlPlaneError::InvalidInput("application_operation"))?;
    ensure_application_console_simple_operation(policies, operation.operation_id())?;
    ensure_application_console_row_scope(
        actor,
        application,
        effective_application_row_scope(policies, row_operation),
    )
}

/// Template import has no persisted target row. Its historical equivalent was application
/// creation, which stamps the current workspace and actor server-side; require both grants.
pub(crate) fn ensure_application_non_crud_creation_operation(
    actor: &domain::ActorContext,
    policies: &[domain::RoleConsolePolicy],
    operation: ApplicationNonCrudConsoleOperation,
) -> Result<(), ControlPlaneError> {
    if operation != ApplicationNonCrudConsoleOperation::OrchestrationTemplateImport {
        return Err(ControlPlaneError::InvalidInput("application_operation"));
    }
    if actor.is_root {
        return Ok(());
    }

    ensure_application_console_simple_operation(policies, operation.operation_id())?;
    ensure_application_console_simple_operation(
        policies,
        access_control::APPLICATIONS_CREATE_OPERATION_ID,
    )
}
