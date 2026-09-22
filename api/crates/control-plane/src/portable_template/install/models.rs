use super::*;
use crate::model_definition::{
    AddModelFieldCommand, CreateModelDefinitionCommand, ModelDefinitionService,
    UpdateModelFieldCommand,
};
impl<R: PortableTemplateInstallRepository> PortableTemplateInstallService<R> {
    pub(super) async fn install_models(
        &self,
        actor: &domain::ActorContext,
        package: &PortableTemplatePackage,
        target: &PortableTemplatePackage,
        result: &mut PortableTemplateInstallResult,
    ) -> Result<()> {
        let owner = ModelDefinitionService::new(self.repository.clone());
        for model in &package.data_models {
            if model.builtin {
                let mapped = target
                    .data_models
                    .iter()
                    .find(|m| m.builtin && m.code == model.code)
                    .context("builtin model missing on target")?;
                result
                    .id_map
                    .insert(model.id.to_string(), mapped.id.to_string());
                for field in &model.fields {
                    let mapped_field = mapped
                        .fields
                        .iter()
                        .find(|f| f.code == field.code)
                        .context("builtin field missing on target")?;
                    result
                        .id_map
                        .insert(field.id.to_string(), mapped_field.id.to_string());
                }
            } else {
                let created = owner
                    .create_model(CreateModelDefinitionCommand {
                        actor_user_id: actor.user_id,
                        scope_kind: model.scope_kind,
                        data_source_instance_id: None,
                        external_resource_key: None,
                        external_table_id: None,
                        external_capabilities: None,
                        template_provider: model.template_provider.clone(),
                        template_code: model.template_code.clone(),
                        template_version: model.template_version.clone(),
                        code: model.code.clone(),
                        title: model.title.clone(),
                        description: model.description.clone(),
                        status: Some(model.status),
                    })
                    .await?;
                result.created("data_model", model.id, created.id);
                for field in model.fields.iter().filter(|f| f.is_system) {
                    let mapped = created
                        .fields
                        .iter()
                        .find(|f| f.code == field.code)
                        .context("recreated platform field missing")?;
                    result
                        .id_map
                        .insert(field.id.to_string(), mapped.id.to_string());
                }
            }
        }
        // All model identities exist before any relation field is created.
        for model in package.data_models.iter().filter(|m| !m.builtin) {
            for field in model.fields.iter().filter(|f| !f.is_system) {
                let created = owner
                    .add_field(AddModelFieldCommand {
                        actor_user_id: actor.user_id,
                        model_id: result.mapped(model.id)?,
                        code: field.code.clone(),
                        title: field.title.clone(),
                        description: field.description.clone(),
                        external_field_key: None,
                        field_kind: field.field_kind,
                        is_required: field.is_required,
                        api_required: Some(field.api_required),
                        is_unique: field.is_unique,
                        default_value: field.default_value.clone().map(|v| result.value(v)),
                        display_interface: field.display_interface.clone(),
                        display_options: result.value(field.display_options.clone()),
                        relation_target_model_id: field
                            .relation_target_model_id
                            .map(|id| result.mapped(id))
                            .transpose()?,
                        relation_options: result.value(field.relation_options.clone()),
                    })
                    .await?;
                result.created("model_field", field.id, created.id);
            }
        }
        Ok(())
    }
    pub(super) async fn fill_model_fields(
        &self,
        actor: &domain::ActorContext,
        package: &PortableTemplatePackage,
        result: &PortableTemplateInstallResult,
    ) -> Result<()> {
        let owner = ModelDefinitionService::new(self.repository.clone());
        // Field options may reference later fields, applications or pages.

        for model in package.data_models.iter().filter(|m| !m.builtin) {
            for field in model.fields.iter().filter(|f| !f.is_system) {
                owner
                    .update_field(UpdateModelFieldCommand {
                        actor_user_id: actor.user_id,
                        model_id: result.mapped(model.id)?,
                        field_id: result.mapped(field.id)?,
                        title: field.title.clone(),
                        description: field.description.clone(),
                        is_required: field.is_required,
                        api_required: Some(field.api_required),
                        is_unique: field.is_unique,
                        default_value: field.default_value.clone().map(|v| result.value(v)),
                        display_interface: field.display_interface.clone(),
                        display_options: result.value(field.display_options.clone()),
                        relation_options: result.value(field.relation_options.clone()),
                    })
                    .await?;
            }
        }
        Ok(())
    }
}
