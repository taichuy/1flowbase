use super::*;

#[async_trait]
impl ApplicationRepository for ApplicationPublicApiTestRepository {
    async fn record_application_extension_source(
        &self,
        _workspace_id: Uuid,
        _application_id: Uuid,
        _extension_installation_id: Uuid,
        _actor_user_id: Uuid,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn has_application_extension_source(
        &self,
        _workspace_id: Uuid,
        _extension_installation_id: Uuid,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        AuthRepository::load_actor_context_for_user(self, actor_user_id).await
    }

    async fn load_role_console_policies_for_user(
        &self,
        _actor_user_id: Uuid,
        _workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .console_policies
            .clone())
    }

    async fn list_applications(
        &self,
        _workspace_id: Uuid,
        _actor_user_id: Uuid,
        _visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationRecord>> {
        anyhow::bail!("list_applications not implemented")
    }

    async fn create_application(
        &self,
        _input: &CreateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        anyhow::bail!("create_application not implemented")
    }

    async fn update_application(
        &self,
        _input: &UpdateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        anyhow::bail!("update_application not implemented")
    }

    async fn delete_application(&self, _input: &DeleteApplicationInput) -> Result<()> {
        anyhow::bail!("delete_application not implemented")
    }

    async fn get_application(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Option<domain::ApplicationRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .applications
            .get(&application_id)
            .cloned()
            .filter(|application| application.workspace_id == workspace_id))
    }

    async fn get_application_for_visibility(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> anyhow::Result<Option<domain::ApplicationRecord>> {
        let application = self.get_application(workspace_id, application_id).await?;
        Ok(application.filter(|application| {
            matches!(visibility, ApplicationVisibility::All)
                || application.created_by == actor_user_id
        }))
    }

    async fn list_application_tags(
        &self,
        _workspace_id: Uuid,
        _actor_user_id: Uuid,
        _visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationTagCatalogEntry>> {
        Ok(Vec::new())
    }

    async fn create_application_tag(
        &self,
        _input: &CreateApplicationTagInput,
    ) -> Result<domain::ApplicationTagCatalogEntry> {
        anyhow::bail!("create_application_tag not implemented")
    }

    async fn list_application_environment_variables(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let application = inner
            .applications
            .get(&application_id)
            .filter(|application| application.workspace_id == workspace_id);
        if application.is_none() {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        Ok(inner
            .application_environment_variables
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
            .expect("application public api test repo mutex poisoned");
        let application = inner
            .applications
            .get(&input.application_id)
            .filter(|application| application.workspace_id == input.workspace_id);
        if application.is_none() {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        let updated_at = OffsetDateTime::now_utc();
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
            .application_environment_variables
            .insert(input.application_id, variables.clone());

        Ok(variables)
    }

    async fn append_audit_log(&self, _event: &domain::AuditLogRecord) -> Result<()> {
        Ok(())
    }
}
