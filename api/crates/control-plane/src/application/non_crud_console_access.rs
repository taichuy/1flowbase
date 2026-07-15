use crate::errors::ControlPlaneError;

use super::ensure_application_console_simple_operation;

/// Stable application operation identities authorized independently from CRUD row policies.
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
}

/// Authorizes a non-CRUD action on a real application already loaded in the actor's workspace.
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

    ensure_application_console_simple_operation(policies, operation.operation_id())
}

/// Template import has no persisted target row; creation still stamps workspace and actor
/// server-side, but `applications.create` is an independent role switch.
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

    ensure_application_console_simple_operation(policies, operation.operation_id())
}
