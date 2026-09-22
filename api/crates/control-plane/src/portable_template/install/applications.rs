use super::*;
use crate::{
    application::{ApplicationService, CreateApplicationCommand},
    application_public_api::{
        mapping::{ApplicationApiMappingService, ReplaceApplicationApiMappingCommand},
        publications::{ApplicationPublicationService, PublishApplicationCommand},
    },
    flow::{FlowService, SaveFlowDraftCommand},
};
impl<R: PortableTemplateInstallRepository> PortableTemplateInstallService<R> {
    pub(super) async fn create_applications(
        &self,
        actor: &domain::ActorContext,
        package: &PortableTemplatePackage,
        result: &mut PortableTemplateInstallResult,
    ) -> Result<()> {
        for app in &package.applications {
            let extension = app
                .published
                .as_ref()
                .map(|p| &p.mapping)
                .or(app.mapping.as_ref())
                .and_then(|m| m.extension.as_ref());
            let trigger = if let Some(schedule) = &app.schedule {
                Some(CreateWorkflowTriggerConfig::Schedule {
                    cron: schedule.cron.clone(),
                    timezone: schedule.timezone.clone(),
                    input_payload: result.value(schedule.input_payload.clone()),
                })
            } else {
                extension.map(|e| CreateWorkflowTriggerConfig::Extension {
                    subpath: e.slug.clone(),
                    http_method: e.method.as_str().into(),
                    response_mode: e.response_mode.as_str().into(),
                })
            };
            let created = ApplicationService::new(self.repository.clone())
                .create_application(CreateApplicationCommand {
                    actor_user_id: actor.user_id,
                    application_type: app.application_type,
                    workflow_trigger_type: app.workflow_trigger_type,
                    workflow_trigger_config: trigger,
                    name: app.name.clone(),
                    description: app.description.clone(),
                    icon: app.icon.clone(),
                    icon_type: app.icon_type.clone(),
                    icon_background: app.icon_background.clone(),
                })
                .await?;
            result.created("application", app.id, created.id);
            let state = FlowService::new(self.repository.clone())
                .get_or_create_editor_state(actor.user_id, created.id)
                .await?;
            for document in std::iter::once(&app.flow_document)
                .chain(app.published.iter().map(|p| &p.flow_document))
            {
                if let Some(source) = document
                    .pointer("/meta/flowId")
                    .and_then(serde_json::Value::as_str)
                {
                    result
                        .id_map
                        .insert(source.to_owned(), state.flow.id.to_string());
                }
            }
        }
        Ok(())
    }
    pub(super) async fn fill_applications(
        &self,
        actor: &domain::ActorContext,
        package: &PortableTemplatePackage,
        result: &mut PortableTemplateInstallResult,
    ) -> Result<()> {
        let flows = FlowService::new(self.repository.clone());
        let mappings = ApplicationApiMappingService::new(self.repository.clone());
        let publications = ApplicationPublicationService::new(self.repository.clone());
        for app in &package.applications {
            let application_id = result.mapped(app.id)?;
            if let Some(published) = &app.published {
                flows
                    .save_draft(SaveFlowDraftCommand {
                        actor_user_id: actor.user_id,
                        application_id,
                        document: result.value(published.flow_document.clone()),
                        change_kind: domain::FlowChangeKind::Logical,
                        summary: "Install portable published definition".into(),
                    })
                    .await?;
                let mapping = serde_json::from_value(
                    result.value(serde_json::to_value(&published.mapping)?),
                )?;
                publications
                    .publish_active_version(PublishApplicationCommand {
                        actor_user_id: actor.user_id,
                        application_id,
                        mapping,
                        api_enabled: published.api_enabled,
                    })
                    .await?;
            }
            // Restore the source draft after publishing its independent frozen definition.
            flows
                .save_draft(SaveFlowDraftCommand {
                    actor_user_id: actor.user_id,
                    application_id,
                    document: result.value(app.flow_document.clone()),
                    change_kind: domain::FlowChangeKind::Logical,
                    summary: "Install portable draft definition".into(),
                })
                .await?;
            if let Some(mapping) = &app.mapping {
                mappings
                    .replace_mapping_draft(ReplaceApplicationApiMappingCommand {
                        actor_user_id: actor.user_id,
                        application_id,
                        mapping: serde_json::from_value(
                            result.value(serde_json::to_value(mapping)?),
                        )?,
                    })
                    .await?;
            }
        }
        Ok(())
    }
}
