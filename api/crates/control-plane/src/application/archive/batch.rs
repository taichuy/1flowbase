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
    if package.applications.is_empty() {
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
        + control_plane_contracts::ports::EventCenterRepository
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sample_entry(index: usize) -> ApplicationArchiveEntry {
        ApplicationArchiveEntry {
            template_id: format!("018f0000-0000-7000-8000-{index:012x}"),
            release_version: 1,
            exported_from_system_version: "0.2.0".into(),
            exported_at: "2026-08-02T00:00:00Z".into(),
            application: ApplicationArchiveApplication {
                application_type: "agent_flow".into(),
                workflow_trigger_type: None,
                name: format!("Application {index}"),
                description: String::new(),
                icon: None,
                icon_type: None,
                icon_background: None,
            },
            flow_document: json!({"graph": {"nodes": [], "edges": []}}),
            dependencies: Vec::new(),
            workflow_trigger_config: None,
        }
    }

    #[test]
    fn validate_archive_application_count_rejects_empty_and_accepts_over_hundred_entries() {
        let empty = ApplicationArchivePackage {
            schema_version: APPLICATION_ARCHIVE_SCHEMA_VERSION.to_string(),
            applications: Vec::new(),
        };
        assert!(validate_archive_application_count(&empty).is_err());

        let large = ApplicationArchivePackage {
            schema_version: APPLICATION_ARCHIVE_SCHEMA_VERSION.to_string(),
            applications: (0..128).map(sample_entry).collect(),
        };
        assert!(validate_archive_application_count(&large).is_ok());
    }
}
