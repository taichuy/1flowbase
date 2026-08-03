use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

const APPLICATIONS_CREATE_ROUTE_OPERATION_ID: &str = "create_application";
const APPLICATIONS_LIST_ROUTE_OPERATION_ID: &str = "list_applications";
const APPLICATION_TAGS_CREATE_ROUTE_OPERATION_ID: &str = "create_application_tag";
const APPLICATION_TAGS_LIST_ROUTE_OPERATION_ID: &str = "get_application_catalog";

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        ApplicationArchiveRelease, ApplicationArchiveReleaseDigest,
        ApplicationEnvironmentVariableInput, ApplicationManagementPage, ApplicationManagementQuery,
        ApplicationManagementRepository, ApplicationRepository, ApplicationVisibility,
        CreateApplicationInput, CreateApplicationTagInput, CreateWorkflowTriggerConfig,
        DeleteApplicationInput, JsDependencyRepository,
        ReplaceApplicationEnvironmentVariablesInput, ReplaceApplicationJsDependencySelectionInput,
        ReplaceInstallationJsDependenciesInput, UpdateApplicationInput,
    },
};

#[cfg(test)]
mod _tests;
mod archive;
pub mod console_policy_migration;
mod non_crud_console_access;

pub use archive::{
    ApplicationArchiveApplication, ApplicationArchiveEntry, ApplicationArchivePackage,
    ApplicationArchiveService, ExportApplicationArchiveCommand, ImportApplicationArchiveCommand,
    PreviewApplicationArchiveCommand, WorkflowTriggerTemplateConfig,
    APPLICATION_ARCHIVE_SCHEMA_VERSION,
};

pub use non_crud_console_access::ApplicationNonCrudConsoleOperation;
pub(crate) use non_crud_console_access::{
    ensure_application_non_crud_creation_operation,
    ensure_existing_application_non_crud_console_operation,
};

pub struct CreateApplicationCommand {
    pub actor_user_id: Uuid,
    pub application_type: domain::ApplicationType,
    pub workflow_trigger_type: Option<domain::WorkflowTriggerType>,
    pub workflow_trigger_config: Option<CreateWorkflowTriggerConfig>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
}

pub struct UpdateApplicationCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub name: String,
    pub description: String,
    pub tag_ids: Vec<Uuid>,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
}

pub struct DeleteApplicationCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
}

pub struct CreateApplicationTagCommand {
    pub actor_user_id: Uuid,
    pub name: String,
}

pub struct ReplaceApplicationEnvironmentVariablesCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub variables: Vec<ApplicationEnvironmentVariableInput>,
}

mod application_service;
mod in_memory_repository;
mod validation_policy;

pub use application_service::ApplicationService;
pub use in_memory_repository::InMemoryApplicationRepository;
use validation_policy::*;
pub(crate) use validation_policy::{
    effective_application_row_scope, ensure_application_console_row_scope,
    ensure_application_console_simple_operation, resolve_application_console_visibility,
};
