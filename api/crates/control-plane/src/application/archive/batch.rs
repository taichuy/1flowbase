use super::*;

#[derive(Debug, Clone, Deserialize)]
pub struct ApplicationArchiveImportSelection {
    pub entry_index: usize,
    pub name: Option<String>,
    pub description: Option<String>,
}

pub struct ApplicationArchivePreviewEntry {
    pub entry_index: usize,
    pub preview: AgentFlowTemplatePreview,
}

pub struct ApplicationArchiveImportOutcome {
    pub entry_index: usize,
    pub result: Result<ImportAgentFlowTemplateResult>,
}

#[derive(Debug, thiserror::Error)]
#[error("application archive import did not finish for {application_id}")]
pub struct ApplicationArchivePartialImport {
    pub application_id: Uuid,
    #[source]
    pub source: anyhow::Error,
}

pub fn validate_archive_application_count(package: &ApplicationArchivePackage) -> Result<()> {
    if package.applications.is_empty()
        || package.applications.len() > MAX_APPLICATION_ARCHIVE_ENTRIES
    {
        return Err(
            ControlPlaneError::InvalidInput("application_archive_application_count").into(),
        );
    }
    Ok(())
}

impl<R> ApplicationArchiveService<R>
where
    R: ApplicationRepository
        + FlowRepository
        + ApplicationApiMappingRepository
        + WorkflowScheduleTriggerRepository
        + Clone,
{
    pub async fn preview_archive_batch(
        &self,
        actor_user_id: Uuid,
        package: ApplicationArchivePackage,
        resources: AgentFlowTemplateResourceSnapshot,
    ) -> Result<Vec<ApplicationArchivePreviewEntry>> {
        validate_archive_application_count(&package)?;
        self.repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        package
            .applications
            .into_iter()
            .enumerate()
            .map(|(entry_index, entry)| {
                Ok(ApplicationArchivePreviewEntry {
                    entry_index,
                    preview: preview_application_template_package(
                        archive_entry_template(entry),
                        &resources,
                    )?,
                })
            })
            .collect()
    }

    pub async fn import_archive_batch(
        &self,
        actor_user_id: Uuid,
        package: ApplicationArchivePackage,
        selections: Vec<ApplicationArchiveImportSelection>,
        resources: AgentFlowTemplateResourceSnapshot,
    ) -> Result<Vec<ApplicationArchiveImportOutcome>> {
        validate_archive_application_count(&package)?;
        let mut indices = BTreeSet::new();
        if selections.is_empty()
            || selections.iter().any(|item| {
                item.entry_index >= package.applications.len() || !indices.insert(item.entry_index)
            })
        {
            return Err(ControlPlaneError::InvalidInput("application_archive_selection").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(&actor)
                .await?;
            ensure_application_non_crud_creation_operation(
                &actor,
                &policies,
                ApplicationNonCrudConsoleOperation::OrchestrationTemplateImport,
            )?;
        }
        // Each application follows the existing import write path. Report every outcome;
        // a later failure must not hide applications already created by this request.
        let mut outcomes = Vec::with_capacity(selections.len());
        for selection in selections {
            let result = self
                .import_archive(ImportApplicationArchiveCommand {
                    actor_user_id,
                    entry: package.applications[selection.entry_index].clone(),
                    name: selection.name,
                    description: selection.description,
                    resources: resources.clone(),
                    source_extension_installation_id: None,
                })
                .await;
            outcomes.push(ApplicationArchiveImportOutcome {
                entry_index: selection.entry_index,
                result,
            });
        }
        Ok(outcomes)
    }
}
