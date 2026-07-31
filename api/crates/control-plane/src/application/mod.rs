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
        ApplicationEnvironmentVariableInput, ApplicationManagementPage, ApplicationManagementQuery,
        ApplicationManagementRepository, ApplicationRepository, ApplicationVisibility,
        CreateApplicationInput, CreateApplicationTagInput, CreateWorkflowTriggerConfig,
        DeleteApplicationInput, JsDependencyRepository,
        ReplaceApplicationEnvironmentVariablesInput, ReplaceApplicationJsDependencySelectionInput,
        ReplaceInstallationJsDependenciesInput, UpdateApplicationInput,
    },
};

pub mod console_policy_migration;
mod non_crud_console_access;

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

pub struct ApplicationService<R> {
    repository: R,
}

impl<R> ApplicationService<R>
where
    R: ApplicationRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_applications(
        &self,
        actor_user_id: Uuid,
    ) -> Result<Vec<domain::ApplicationRecord>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        let visibility = if actor.is_root {
            ApplicationVisibility::All
        } else {
            let policies = self
                .repository
                .load_role_console_policies_for_user(actor_user_id, actor.current_workspace_id)
                .await?;
            resolve_application_console_visibility(&policies, APPLICATIONS_LIST_ROUTE_OPERATION_ID)?
        };

        let applications = self
            .repository
            .list_applications(actor.current_workspace_id, actor_user_id, visibility)
            .await?;

        Ok(applications
            .into_iter()
            .map(with_product_capability_sections)
            .collect())
    }

    pub async fn list_application_management(
        &self,
        actor_user_id: Uuid,
        query: ApplicationManagementQuery,
    ) -> Result<ApplicationManagementPage>
    where
        R: ApplicationManagementRepository,
    {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(actor_user_id, actor.current_workspace_id)
                .await?;
            let operation_id = domain::ConsoleOperationId::try_from(
                access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION,
            )
            .expect("compiled applications management operation id must be valid");
            if !domain::effective_console_simple_operation(
                &policies,
                &applications_console_group(),
                &operation_id,
            ) {
                return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
            }
        }
        validate_application_management_filter(&query.filter)?;

        self.repository
            .list_application_management(actor.current_workspace_id, &query)
            .await
    }

    pub async fn create_application(
        &self,
        command: CreateApplicationCommand,
    ) -> Result<domain::ApplicationRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(
                    command.actor_user_id,
                    actor.current_workspace_id,
                )
                .await?;
            let operation_id =
                domain::ConsoleOperationId::try_from(APPLICATIONS_CREATE_ROUTE_OPERATION_ID)
                    .expect("compiled applications create operation id must be valid");
            if !domain::effective_console_simple_operation(
                &policies,
                &applications_console_group(),
                &operation_id,
            ) {
                return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
            }
        }

        self.create_application_record(&actor, command).await
    }

    pub(crate) async fn create_application_from_authorized_template_import(
        &self,
        actor: &domain::ActorContext,
        command: CreateApplicationCommand,
    ) -> Result<domain::ApplicationRecord> {
        if actor.user_id != command.actor_user_id {
            return Err(ControlPlaneError::InvalidInput("actor_user_id").into());
        }

        self.create_application_record(actor, command).await
    }

    async fn create_application_record(
        &self,
        actor: &domain::ActorContext,
        command: CreateApplicationCommand,
    ) -> Result<domain::ApplicationRecord> {
        let created = self
            .repository
            .create_application(&CreateApplicationInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                application_type: command.application_type,
                workflow_trigger_type: create_application_workflow_trigger_type(
                    command.application_type,
                    command.workflow_trigger_type,
                ),
                workflow_trigger_config: command.workflow_trigger_config,
                name: command.name,
                description: command.description,
                icon: command.icon,
                icon_type: command.icon_type,
                icon_background: command.icon_background,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "application",
                Some(created.id),
                "application.created",
                serde_json::json!({
                    "application_type": created.application_type.as_str(),
                    "workflow_trigger_type": created.workflow_trigger_type.map(|value| value.as_str()),
                    "name": created.name,
                }),
            ))
            .await?;

        Ok(with_product_capability_sections(created))
    }

    pub async fn update_application(
        &self,
        command: UpdateApplicationCommand,
    ) -> Result<domain::ApplicationRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(
                    command.actor_user_id,
                    actor.current_workspace_id,
                )
                .await?;
            ensure_application_console_row_scope(
                &actor,
                &application,
                effective_application_row_scope(
                    &policies,
                    access_control::APPLICATIONS_UPDATE_OPERATION_ID,
                ),
            )?;
        }

        let updated = self
            .repository
            .update_application(&UpdateApplicationInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                application_id: command.application_id,
                name: normalize_required_text(&command.name, "name")?,
                description: command.description.trim().to_string(),
                tag_ids: dedupe_tag_ids(command.tag_ids),
                icon: command.icon.map(normalize_optional_text),
                icon_type: command.icon_type.map(normalize_optional_text),
                icon_background: command.icon_background.map(normalize_optional_text),
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "application",
                Some(updated.id),
                "application.updated",
                serde_json::json!({
                    "name": updated.name,
                    "tag_count": updated.tags.len(),
                }),
            ))
            .await?;

        Ok(with_product_capability_sections(updated))
    }

    pub async fn delete_application(&self, command: DeleteApplicationCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(
                    command.actor_user_id,
                    actor.current_workspace_id,
                )
                .await?;
            ensure_application_console_row_scope(
                &actor,
                &application,
                effective_application_row_scope(
                    &policies,
                    access_control::APPLICATIONS_DELETE_OPERATION_ID,
                ),
            )?;
        }

        self.repository
            .delete_application(&DeleteApplicationInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                application_id: command.application_id,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "application",
                Some(application.id),
                "application.deleted",
                serde_json::json!({
                    "application_type": application.application_type.as_str(),
                    "name": application.name,
                }),
            ))
            .await?;

        Ok(())
    }

    pub async fn list_application_tags(
        &self,
        actor_user_id: Uuid,
    ) -> Result<Vec<domain::ApplicationTagCatalogEntry>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(actor_user_id, actor.current_workspace_id)
                .await?;
            ensure_application_console_simple_operation(
                &policies,
                APPLICATION_TAGS_LIST_ROUTE_OPERATION_ID,
            )?;
        }

        self.repository
            .list_application_tags(
                actor.current_workspace_id,
                actor_user_id,
                ApplicationVisibility::All,
            )
            .await
    }

    pub async fn create_application_tag(
        &self,
        command: CreateApplicationTagCommand,
    ) -> Result<domain::ApplicationTagCatalogEntry> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;

        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(
                    command.actor_user_id,
                    actor.current_workspace_id,
                )
                .await?;
            ensure_application_console_simple_operation(
                &policies,
                APPLICATION_TAGS_CREATE_ROUTE_OPERATION_ID,
            )?;
        }

        let tag = self
            .repository
            .create_application_tag(&CreateApplicationTagInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                name: normalize_required_text(&command.name, "name")?,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "application_tag",
                Some(tag.id),
                "application.tag_created",
                serde_json::json!({
                    "name": tag.name,
                }),
            ))
            .await?;

        Ok(tag)
    }

    pub async fn get_application(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
    ) -> Result<domain::ApplicationRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        let visibility = if actor.is_root {
            ApplicationVisibility::All
        } else {
            let policies = self
                .repository
                .load_role_console_policies_for_user(actor_user_id, actor.current_workspace_id)
                .await?;
            resolve_application_console_visibility(
                &policies,
                access_control::APPLICATIONS_VIEW_OPERATION_ID,
            )?
        };
        let application = self
            .repository
            .get_application_for_visibility(
                actor.current_workspace_id,
                application_id,
                actor_user_id,
                visibility,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;

        Ok(with_product_capability_sections(application))
    }

    /// Loads a real application in the actor's current workspace and applies its independent
    /// non-CRUD `Simple` grant.
    pub async fn load_application_for_non_crud_console_operation(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
        operation: ApplicationNonCrudConsoleOperation,
    ) -> Result<domain::ApplicationRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        let policies = if actor.is_root {
            Vec::new()
        } else {
            self.repository
                .load_role_console_policies_for_user(actor_user_id, actor.current_workspace_id)
                .await?
        };
        ensure_existing_application_non_crud_console_operation(
            &actor,
            &application,
            &policies,
            operation,
        )?;

        Ok(application)
    }

    pub async fn list_application_environment_variables(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        let application = self
            .load_visible_application(&actor, actor_user_id, application_id)
            .await?;

        self.repository
            .list_application_environment_variables(actor.current_workspace_id, application.id)
            .await
    }

    pub async fn replace_application_environment_variables(
        &self,
        command: ReplaceApplicationEnvironmentVariablesCommand,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(
                    command.actor_user_id,
                    actor.current_workspace_id,
                )
                .await?;
            ensure_application_console_row_scope(
                &actor,
                &application,
                effective_application_row_scope(
                    &policies,
                    access_control::APPLICATIONS_UPDATE_OPERATION_ID,
                ),
            )?;
        }

        let variables = normalize_environment_variables(command.variables)?;
        let replaced = self
            .repository
            .replace_application_environment_variables(
                &ReplaceApplicationEnvironmentVariablesInput {
                    actor_user_id: command.actor_user_id,
                    workspace_id: actor.current_workspace_id,
                    application_id: command.application_id,
                    variables,
                },
            )
            .await?;

        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "application",
                Some(command.application_id),
                "application.environment_variables_replaced",
                serde_json::json!({
                    "variable_count": replaced.len(),
                }),
            ))
            .await?;

        Ok(replaced)
    }

    async fn load_visible_application(
        &self,
        actor: &domain::ActorContext,
        actor_user_id: Uuid,
        application_id: Uuid,
    ) -> Result<domain::ApplicationRecord> {
        let visibility = if actor.is_root {
            ApplicationVisibility::All
        } else {
            let policies = self
                .repository
                .load_role_console_policies_for_user(actor_user_id, actor.current_workspace_id)
                .await?;
            resolve_application_console_visibility(
                &policies,
                access_control::APPLICATIONS_VIEW_OPERATION_ID,
            )?
        };
        let application = self
            .repository
            .get_application_for_visibility(
                actor.current_workspace_id,
                application_id,
                actor_user_id,
                visibility,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;

        Ok(application)
    }
}

fn validate_application_management_filter(
    filter: &domain::ResourceFilterExpr,
) -> Result<(), ControlPlaneError> {
    match filter {
        domain::ResourceFilterExpr::All(items) | domain::ResourceFilterExpr::Any(items) => {
            for item in items {
                validate_application_management_filter(item)?;
            }
            Ok(())
        }
        domain::ResourceFilterExpr::Field {
            field, operator, ..
        } => {
            let operator_allowed = match field.as_str() {
                "id" | "name" => matches!(
                    operator,
                    domain::ResourceFilterOperator::Eq
                        | domain::ResourceFilterOperator::Ne
                        | domain::ResourceFilterOperator::Includes
                        | domain::ResourceFilterOperator::NotIncludes
                        | domain::ResourceFilterOperator::In
                ),
                "application_type"
                | "workflow_trigger_type"
                | "publication_status"
                | "created_by" => matches!(
                    operator,
                    domain::ResourceFilterOperator::Eq
                        | domain::ResourceFilterOperator::Ne
                        | domain::ResourceFilterOperator::In
                ),
                "tags.id" => matches!(
                    operator,
                    domain::ResourceFilterOperator::Eq | domain::ResourceFilterOperator::In
                ),
                _ => false,
            };

            if operator_allowed {
                Ok(())
            } else {
                Err(ControlPlaneError::InvalidInput("filter"))
            }
        }
    }
}

fn applications_console_group() -> domain::ConsolePolicyGroup {
    domain::ConsolePolicyGroup::settings_feature(
        access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID,
    )
    .expect("compiled applications settings feature id must be valid")
}

pub(crate) fn effective_application_row_scope(
    policies: &[domain::RoleConsolePolicy],
    operation_id: &str,
) -> domain::ConsoleOperationRowScope {
    let operation_id = domain::ConsoleOperationId::try_from(operation_id)
        .expect("compiled applications row operation id must be valid");
    domain::effective_console_row_scope(policies, &applications_console_group(), &operation_id)
}

pub(crate) fn resolve_application_console_visibility(
    policies: &[domain::RoleConsolePolicy],
    operation_id: &str,
) -> Result<ApplicationVisibility, ControlPlaneError> {
    match effective_application_row_scope(policies, operation_id) {
        domain::ConsoleOperationRowScope::ScopeAll => Ok(ApplicationVisibility::All),
        domain::ConsoleOperationRowScope::Own => Ok(ApplicationVisibility::Own),
        domain::ConsoleOperationRowScope::Disabled => {
            Err(ControlPlaneError::PermissionDenied("permission_denied"))
        }
    }
}

pub(crate) fn ensure_application_console_row_scope(
    actor: &domain::ActorContext,
    application: &domain::ApplicationRecord,
    scope: domain::ConsoleOperationRowScope,
) -> Result<(), ControlPlaneError> {
    match scope {
        domain::ConsoleOperationRowScope::ScopeAll => Ok(()),
        domain::ConsoleOperationRowScope::Own if application.created_by == actor.user_id => Ok(()),
        domain::ConsoleOperationRowScope::Own | domain::ConsoleOperationRowScope::Disabled => {
            Err(ControlPlaneError::PermissionDenied("permission_denied"))
        }
    }
}

pub(crate) fn ensure_application_console_simple_operation(
    policies: &[domain::RoleConsolePolicy],
    operation_id: &str,
) -> Result<(), ControlPlaneError> {
    let operation_id = domain::ConsoleOperationId::try_from(operation_id)
        .expect("compiled applications simple operation id must be valid");
    if domain::effective_console_simple_operation(
        policies,
        &applications_console_group(),
        &operation_id,
    ) {
        Ok(())
    } else {
        Err(ControlPlaneError::PermissionDenied("permission_denied"))
    }
}

fn normalize_required_text(value: &str, field: &'static str) -> Result<String, ControlPlaneError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(ControlPlaneError::InvalidInput(field));
    }

    Ok(normalized.to_string())
}

fn normalize_optional_text(value: String) -> Option<String> {
    let normalized = value.trim();
    (!normalized.is_empty()).then(|| normalized.to_string())
}

fn dedupe_tag_ids(tag_ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for tag_id in tag_ids {
        if seen.insert(tag_id) {
            deduped.push(tag_id);
        }
    }

    deduped
}

fn normalize_environment_variables(
    variables: Vec<ApplicationEnvironmentVariableInput>,
) -> Result<Vec<ApplicationEnvironmentVariableInput>, ControlPlaneError> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(variables.len());

    for variable in variables {
        let name = normalize_environment_variable_name(&variable.name)?;
        if !seen.insert(name.clone()) {
            return Err(ControlPlaneError::InvalidInput("environment_variable.name"));
        }

        let value_type = normalize_environment_variable_value_type(&variable.value_type)?;
        ensure_environment_variable_value_matches_type(&value_type, &variable.value)?;
        normalized.push(ApplicationEnvironmentVariableInput {
            name,
            value_type,
            value: variable.value,
            description: variable.description.trim().to_string(),
        });
    }

    Ok(normalized)
}

fn normalize_environment_variable_name(value: &str) -> Result<String, ControlPlaneError> {
    let name = value.trim();
    let mut chars = name.chars();

    if !chars.next().is_some_and(|ch| ch.is_ascii_alphabetic()) {
        return Err(ControlPlaneError::InvalidInput("environment_variable.name"));
    }

    if !chars.all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(ControlPlaneError::InvalidInput("environment_variable.name"));
    }

    Ok(name.to_string())
}

fn normalize_environment_variable_value_type(value: &str) -> Result<String, ControlPlaneError> {
    let value_type = value.trim();
    let allowed = [
        "string",
        "number",
        "boolean",
        "object",
        "array[string]",
        "array[number]",
        "array[boolean]",
        "array[object]",
    ];

    if allowed.contains(&value_type) {
        Ok(value_type.to_string())
    } else {
        Err(ControlPlaneError::InvalidInput(
            "environment_variable.value_type",
        ))
    }
}

fn ensure_environment_variable_value_matches_type(
    value_type: &str,
    value: &serde_json::Value,
) -> Result<(), ControlPlaneError> {
    let valid = match value_type {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array[string]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_string)),
        "array[number]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_number)),
        "array[boolean]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_boolean)),
        "array[object]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_object)),
        _ => false,
    };

    if valid {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput(
            "environment_variable.value",
        ))
    }
}

#[derive(Default)]
struct InMemoryApplicationRepositoryInner {
    applications: HashMap<Uuid, domain::ApplicationRecord>,
    environment_variables: HashMap<Uuid, Vec<domain::ApplicationEnvironmentVariable>>,
    js_dependencies: Vec<domain::JsDependencyRegistryEntry>,
    js_dependency_selections:
        HashMap<(Uuid, String, String), domain::ApplicationJsDependencySelection>,
    tags: HashMap<Uuid, domain::ApplicationTagCatalogEntry>,
    permissions: Vec<String>,
    console_policies: Vec<domain::RoleConsolePolicy>,
    actor_is_root: bool,
    workspace_id: Uuid,
    tenant_id: Uuid,
    audit_events: Vec<String>,
}

#[derive(Clone)]
pub struct InMemoryApplicationRepository {
    inner: Arc<Mutex<InMemoryApplicationRepositoryInner>>,
}

impl InMemoryApplicationRepository {
    pub fn with_permissions(permissions: Vec<&str>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InMemoryApplicationRepositoryInner {
                applications: HashMap::new(),
                environment_variables: HashMap::new(),
                js_dependencies: Vec::new(),
                js_dependency_selections: HashMap::new(),
                tags: HashMap::new(),
                permissions: permissions.into_iter().map(str::to_string).collect(),
                console_policies: Vec::new(),
                actor_is_root: false,
                workspace_id: Uuid::nil(),
                tenant_id: Uuid::nil(),
                audit_events: Vec::new(),
            })),
        }
    }

    pub fn with_console_policies(policies: Vec<domain::RoleConsolePolicy>) -> Self {
        let repository = Self::with_permissions(Vec::new());
        repository
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .console_policies = policies;
        repository
    }

    fn insert_application(&self, actor_user_id: Uuid, name: &str) -> domain::ApplicationRecord {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let application = build_application_record(
            Uuid::now_v7(),
            CreateApplicationInput {
                workflow_trigger_config: None,
                actor_user_id,
                workspace_id: inner.workspace_id,
                application_type: domain::ApplicationType::AgentFlow,
                workflow_trigger_type: None,
                name: name.to_string(),
                description: String::new(),
                icon: None,
                icon_type: None,
                icon_background: None,
            },
        );
        inner
            .applications
            .insert(application.id, application.clone());
        application
    }

    fn insert_application_in_workspace(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        name: &str,
    ) -> domain::ApplicationRecord {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let application = build_application_record(
            Uuid::now_v7(),
            CreateApplicationInput {
                workflow_trigger_config: None,
                actor_user_id,
                workspace_id,
                application_type: domain::ApplicationType::AgentFlow,
                workflow_trigger_type: None,
                name: name.to_string(),
                description: String::new(),
                icon: None,
                icon_type: None,
                icon_background: None,
            },
        );
        inner
            .applications
            .insert(application.id, application.clone());
        application
    }
}

#[async_trait]
impl JsDependencyRepository for InMemoryApplicationRepository {
    async fn replace_installation_js_dependencies(
        &self,
        input: &ReplaceInstallationJsDependenciesInput,
    ) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        inner
            .js_dependencies
            .retain(|entry| entry.installation_id != input.installation_id);
        inner
            .js_dependencies
            .extend(
                input
                    .entries
                    .iter()
                    .map(|entry| domain::JsDependencyRegistryEntry {
                        installation_id: input.installation_id,
                        provider_code: input.provider_code.clone(),
                        plugin_id: input.plugin_id.clone(),
                        plugin_version: input.plugin_version.clone(),
                        alias: entry.alias.clone(),
                        package: entry.package.clone(),
                        version: entry.version.clone(),
                        target: entry.target.clone(),
                        artifact_path: entry.artifact_path.clone(),
                        integrity: entry.integrity.clone(),
                        permissions: entry.permissions.clone(),
                    }),
            );
        Ok(())
    }

    async fn list_workspace_js_dependencies(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::JsDependencyRegistryEntry>> {
        Ok(self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .js_dependencies
            .clone())
    }
}

#[async_trait]
impl crate::ports::ApplicationJsDependencySelectionRepository for InMemoryApplicationRepository {
    async fn list_application_js_dependency_selections(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationJsDependencySelection>> {
        let mut selections = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .js_dependency_selections
            .values()
            .filter(|selection| {
                selection.workspace_id == workspace_id && selection.application_id == application_id
            })
            .cloned()
            .collect::<Vec<_>>();
        selections.sort_by(|left, right| {
            left.alias
                .cmp(&right.alias)
                .then(left.target.cmp(&right.target))
        });

        Ok(selections)
    }

    async fn replace_application_js_dependency_selection(
        &self,
        input: &ReplaceApplicationJsDependencySelectionInput,
    ) -> Result<domain::ApplicationJsDependencySelection> {
        let selection = domain::ApplicationJsDependencySelection {
            workspace_id: input.workspace_id,
            application_id: input.application_id,
            installation_id: input.installation_id,
            provider_code: input.provider_code.clone(),
            plugin_id: input.plugin_id.clone(),
            plugin_version: input.plugin_version.clone(),
            alias: input.alias.clone(),
            package: input.package.clone(),
            version: input.version.clone(),
            target: input.target.clone(),
            artifact_path: input.artifact_path.clone(),
            artifact_hash: input.artifact_hash.clone(),
            integrity: input.integrity.clone(),
            permissions: input.permissions.clone(),
        };
        self.inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .js_dependency_selections
            .insert(
                (
                    input.application_id,
                    input.alias.clone(),
                    input.target.clone(),
                ),
                selection.clone(),
            );

        Ok(selection)
    }
}

#[async_trait]
impl ApplicationRepository for InMemoryApplicationRepository {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        let inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");

        if inner.actor_is_root {
            Ok(domain::ActorContext::root_in_scope(
                actor_user_id,
                inner.tenant_id,
                inner.workspace_id,
                "root",
            ))
        } else {
            Ok(domain::ActorContext::scoped_in_scope(
                actor_user_id,
                inner.tenant_id,
                inner.workspace_id,
                "member",
                inner.permissions.iter().cloned(),
            ))
        }
    }

    async fn load_role_console_policies_for_user(
        &self,
        _actor_user_id: Uuid,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .console_policies
            .clone())
    }

    async fn list_applications(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationRecord>> {
        let mut applications = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .applications
            .values()
            .filter(|application| application.workspace_id == workspace_id)
            .filter(|application| {
                matches!(visibility, ApplicationVisibility::All)
                    || application.created_by == actor_user_id
            })
            .cloned()
            .collect::<Vec<_>>();
        applications.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then(right.id.cmp(&left.id))
        });

        Ok(applications)
    }

    async fn create_application(
        &self,
        input: &CreateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        let application = build_application_record(Uuid::now_v7(), input.clone());
        self.inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .applications
            .insert(application.id, application.clone());

        Ok(application)
    }

    async fn update_application(
        &self,
        input: &UpdateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let tags = input
            .tag_ids
            .iter()
            .map(|tag_id| inner.tags.get(tag_id).cloned())
            .collect::<Option<Vec<_>>>()
            .ok_or(ControlPlaneError::InvalidInput("tag_ids"))?
            .into_iter()
            .map(|tag| domain::ApplicationTag {
                id: tag.id,
                name: tag.name,
            })
            .collect::<Vec<_>>();
        let application = inner
            .applications
            .get_mut(&input.application_id)
            .ok_or(ControlPlaneError::NotFound("application"))?;
        application.name = input.name.clone();
        application.description = input.description.clone();
        if let Some(icon) = &input.icon {
            application.icon = icon.clone();
        }
        if let Some(icon_type) = &input.icon_type {
            application.icon_type = icon_type.clone();
        }
        if let Some(icon_background) = &input.icon_background {
            application.icon_background = icon_background.clone();
        }
        application.updated_at = time::OffsetDateTime::now_utc();
        application.tags = tags;

        Ok(application.clone())
    }

    async fn delete_application(&self, input: &DeleteApplicationInput) -> Result<()> {
        let deleted = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .applications
            .remove(&input.application_id);

        if deleted.is_none() {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        Ok(())
    }

    async fn get_application(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Option<domain::ApplicationRecord>> {
        let application = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .applications
            .get(&application_id)
            .cloned()
            .filter(|application| application.workspace_id == workspace_id);

        Ok(application)
    }

    async fn get_application_for_visibility(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Option<domain::ApplicationRecord>> {
        let application = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .applications
            .get(&application_id)
            .cloned()
            .filter(|application| application.workspace_id == workspace_id)
            .filter(|application| {
                matches!(visibility, ApplicationVisibility::All)
                    || application.created_by == actor_user_id
            });
        Ok(application)
    }

    async fn list_application_tags(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationTagCatalogEntry>> {
        let inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let mut tags = inner.tags.values().cloned().collect::<Vec<_>>();
        for tag in &mut tags {
            tag.application_count = inner
                .applications
                .values()
                .filter(|application| application.workspace_id == workspace_id)
                .filter(|application| {
                    matches!(visibility, ApplicationVisibility::All)
                        || application.created_by == actor_user_id
                })
                .filter(|application| application.tags.iter().any(|item| item.id == tag.id))
                .count() as i64;
        }
        tags.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));

        Ok(tags)
    }

    async fn create_application_tag(
        &self,
        input: &CreateApplicationTagInput,
    ) -> Result<domain::ApplicationTagCatalogEntry> {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        if let Some(existing) = inner
            .tags
            .values()
            .find(|tag| tag.name.eq_ignore_ascii_case(&input.name))
            .cloned()
        {
            return Ok(existing);
        }

        let tag = domain::ApplicationTagCatalogEntry {
            id: Uuid::now_v7(),
            name: input.name.clone(),
            application_count: 0,
        };
        inner.tags.insert(tag.id, tag.clone());

        Ok(tag)
    }

    async fn list_application_environment_variables(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let application = inner
            .applications
            .get(&application_id)
            .filter(|application| application.workspace_id == workspace_id);

        if application.is_none() {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        Ok(inner
            .environment_variables
            .get(&application_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn replace_application_environment_variables(
        &self,
        input: &ReplaceApplicationEnvironmentVariablesInput,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let application = inner
            .applications
            .get(&input.application_id)
            .filter(|application| application.workspace_id == input.workspace_id);

        if application.is_none() {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        let updated_at = time::OffsetDateTime::now_utc();
        let variables = input
            .variables
            .iter()
            .map(|variable| domain::ApplicationEnvironmentVariable {
                application_id: input.application_id,
                name: variable.name.clone(),
                value_type: variable.value_type.clone(),
                value: variable.value.clone(),
                description: variable.description.clone(),
                updated_at,
            })
            .collect::<Vec<_>>();
        inner
            .environment_variables
            .insert(input.application_id, variables.clone());

        Ok(variables)
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        self.inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .audit_events
            .push(event.event_code.clone());
        Ok(())
    }
}

#[async_trait]
impl ApplicationManagementRepository for InMemoryApplicationRepository {
    async fn list_application_management(
        &self,
        _workspace_id: Uuid,
        query: &ApplicationManagementQuery,
    ) -> Result<ApplicationManagementPage> {
        Ok(ApplicationManagementPage {
            items: Vec::new(),
            total: 0,
            page: query.page,
            page_size: query.page_size,
        })
    }
}

fn build_application_record(id: Uuid, input: CreateApplicationInput) -> domain::ApplicationRecord {
    domain::ApplicationRecord {
        id,
        workspace_id: input.workspace_id,
        application_type: input.application_type,
        workflow_trigger_type: input.workflow_trigger_type,
        name: input.name,
        description: input.description,
        icon: input.icon,
        icon_type: input.icon_type,
        icon_background: input.icon_background,
        created_by: input.actor_user_id,
        updated_at: time::OffsetDateTime::now_utc(),
        tags: Vec::new(),
        sections: planned_sections(input.application_type, input.workflow_trigger_type),
    }
}

fn create_application_workflow_trigger_type(
    application_type: domain::ApplicationType,
    workflow_trigger_type: Option<domain::WorkflowTriggerType>,
) -> Option<domain::WorkflowTriggerType> {
    match application_type {
        domain::ApplicationType::AgentFlow => None,
        domain::ApplicationType::Workflow => {
            Some(workflow_trigger_type.unwrap_or(domain::WorkflowTriggerType::Extension))
        }
    }
}

fn planned_sections(
    application_type: domain::ApplicationType,
    workflow_trigger_type: Option<domain::WorkflowTriggerType>,
) -> domain::ApplicationSections {
    let mut sections = domain::ApplicationSections {
        orchestration: domain::ApplicationOrchestrationSection {
            status: "planned".to_string(),
            subject_kind: application_type.as_str().to_string(),
            subject_status: "unconfigured".to_string(),
            current_subject_id: None,
            current_draft_id: None,
        },
        api: domain::ApplicationApiSection {
            status: "planned".to_string(),
            credential_kind: "application_api_key".to_string(),
            invoke_routing_mode: "api_key_bound_application".to_string(),
            invoke_path_template: Some("/api/agent/v1/runs".to_string()),
            api_capability_status: "not_published".to_string(),
            credentials_status: "missing".to_string(),
        },
        logs: domain::ApplicationLogsSection {
            status: "planned".to_string(),
            runs_capability_status: "planned".to_string(),
            run_object_kind: "application_run".to_string(),
            log_retention_status: "planned".to_string(),
        },
        monitoring: domain::ApplicationMonitoringSection {
            status: "planned".to_string(),
            metrics_capability_status: "planned".to_string(),
            metrics_object_kind: "application_metrics".to_string(),
            tracing_config_status: "planned".to_string(),
        },
    };
    sections.api = product_api_section(application_type, workflow_trigger_type, sections.api);
    sections
}

fn with_product_capability_sections(
    mut application: domain::ApplicationRecord,
) -> domain::ApplicationRecord {
    application.sections.api = product_api_section(
        application.application_type,
        application.workflow_trigger_type,
        application.sections.api,
    );
    application
}

fn product_api_section(
    application_type: domain::ApplicationType,
    workflow_trigger_type: Option<domain::WorkflowTriggerType>,
    agent_flow_api: domain::ApplicationApiSection,
) -> domain::ApplicationApiSection {
    match (application_type, workflow_trigger_type) {
        (domain::ApplicationType::AgentFlow, _) => agent_flow_api,
        (domain::ApplicationType::Workflow, Some(domain::WorkflowTriggerType::Extension)) => {
            domain::ApplicationApiSection {
                status: "available".to_string(),
                credential_kind: "user_or_public".to_string(),
                invoke_routing_mode: "published_workflow_operation".to_string(),
                invoke_path_template: Some("/api/ex/{operation}".to_string()),
                api_capability_status: "available".to_string(),
                credentials_status: "not_required".to_string(),
            }
        }
        (domain::ApplicationType::Workflow, _) => domain::ApplicationApiSection {
            status: "unavailable".to_string(),
            credential_kind: "not_applicable".to_string(),
            invoke_routing_mode: "not_available".to_string(),
            invoke_path_template: None,
            api_capability_status: "unavailable".to_string(),
            credentials_status: "not_applicable".to_string(),
        },
    }
}

impl ApplicationService<InMemoryApplicationRepository> {
    pub fn for_tests() -> Self {
        Self::for_tests_with_console_policies(
            vec![
                "application.view.all",
                "application.create.all",
                "application.edit.all",
            ],
            vec![domain::RoleConsolePolicy::new(
                Uuid::now_v7(),
                vec![domain::RoleConsoleGroupPolicy::full(
                    applications_console_group(),
                )],
            )],
        )
    }

    pub fn for_tests_with_permissions(permissions: Vec<&str>) -> Self {
        Self::new(InMemoryApplicationRepository::with_permissions(permissions))
    }

    pub fn for_tests_with_console_policies(
        permissions: Vec<&str>,
        policies: Vec<domain::RoleConsolePolicy>,
    ) -> Self {
        let repository = InMemoryApplicationRepository::with_permissions(permissions);
        repository
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .console_policies = policies;
        Self::new(repository)
    }

    pub fn for_tests_as_root() -> Self {
        let repository = InMemoryApplicationRepository::with_permissions(Vec::new());
        repository
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .actor_is_root = true;
        Self::new(repository)
    }

    pub fn seed_foreign_application(&self, name: &str) -> domain::ApplicationRecord {
        self.repository.insert_application(Uuid::now_v7(), name)
    }

    pub fn seed_application_for_actor(
        &self,
        actor_user_id: Uuid,
        name: &str,
    ) -> domain::ApplicationRecord {
        self.repository.insert_application(actor_user_id, name)
    }

    pub fn seed_application_in_workspace(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        name: &str,
    ) -> domain::ApplicationRecord {
        self.repository
            .insert_application_in_workspace(workspace_id, actor_user_id, name)
    }

    pub fn seed_js_dependency_catalog_entry(&self, entry: domain::JsDependencyRegistryEntry) {
        self.repository
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .js_dependencies
            .push(entry);
    }

    pub fn repository_for_tests(&self) -> InMemoryApplicationRepository {
        self.repository.clone()
    }

    pub fn audit_events(&self) -> Vec<String> {
        self.repository
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .audit_events
            .clone()
    }
}
