use super::*;

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
                .load_role_console_policies_for_user(&actor)
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
                .load_role_console_policies_for_user(&actor)
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
                .load_role_console_policies_for_user(&actor)
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
                .load_role_console_policies_for_user(&actor)
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
                .load_role_console_policies_for_user(&actor)
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
                .load_role_console_policies_for_user(&actor)
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
                .load_role_console_policies_for_user(&actor)
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
                .load_role_console_policies_for_user(&actor)
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
                .load_role_console_policies_for_user(&actor)
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
                .load_role_console_policies_for_user(&actor)
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
                .load_role_console_policies_for_user(&actor)
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
