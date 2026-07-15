use anyhow::Result;

use crate::{
    model_definition::{
        AddModelFieldCommand, BatchDeleteModelDefinitionsCommand, CreateModelDefinitionCommand,
        DeleteModelDefinitionCommand, DeleteModelFieldCommand, ModelDefinitionService,
        UpdateModelDefinitionCommand, UpdateModelDefinitionStatusCommand, UpdateModelFieldCommand,
    },
    ports::{ModelDefinitionRepository, RoleConsolePolicyReader, RuntimeRegistrySync},
};

pub struct ModelDefinitionMutationService<R, S> {
    model_definitions: ModelDefinitionService<R>,
    sync: S,
}

impl<R, S> ModelDefinitionMutationService<R, S>
where
    R: ModelDefinitionRepository,
    S: RuntimeRegistrySync,
{
    pub fn new(repository: R, sync: S) -> Self {
        Self {
            model_definitions: ModelDefinitionService::new(repository),
            sync,
        }
    }

    pub fn for_console_operation(
        repository: R,
        sync: S,
        group: domain::ConsolePolicyGroup,
        operation_id: &'static str,
    ) -> Self
    where
        R: RoleConsolePolicyReader + Clone + 'static,
    {
        Self {
            model_definitions: ModelDefinitionService::for_console_operation(
                repository,
                group,
                operation_id,
            ),
            sync,
        }
    }

    pub async fn create_model(
        &self,
        command: CreateModelDefinitionCommand,
    ) -> Result<domain::ModelDefinitionRecord> {
        let model = self.model_definitions.create_model(command).await?;
        self.sync.rebuild().await?;
        Ok(model)
    }

    pub async fn update_model(
        &self,
        command: UpdateModelDefinitionCommand,
    ) -> Result<domain::ModelDefinitionRecord> {
        let model = self.model_definitions.update_model(command).await?;
        self.sync.rebuild().await?;
        Ok(model)
    }

    pub async fn update_model_status(
        &self,
        command: UpdateModelDefinitionStatusCommand,
    ) -> Result<domain::ModelDefinitionRecord> {
        let model = self.model_definitions.update_model_status(command).await?;
        self.sync.rebuild().await?;
        Ok(model)
    }

    pub async fn add_field(
        &self,
        command: AddModelFieldCommand,
    ) -> Result<domain::ModelFieldRecord> {
        let field = self.model_definitions.add_field(command).await?;
        self.sync.rebuild().await?;
        Ok(field)
    }

    pub async fn update_field(
        &self,
        command: UpdateModelFieldCommand,
    ) -> Result<domain::ModelFieldRecord> {
        let field = self.model_definitions.update_field(command).await?;
        self.sync.rebuild().await?;
        Ok(field)
    }

    pub async fn delete_model(&self, command: DeleteModelDefinitionCommand) -> Result<()> {
        self.model_definitions.delete_model(command).await?;
        self.sync.rebuild().await?;
        Ok(())
    }

    pub async fn batch_delete_models(
        &self,
        command: BatchDeleteModelDefinitionsCommand,
    ) -> Result<Vec<uuid::Uuid>> {
        let deleted_model_ids = self.model_definitions.batch_delete_models(command).await?;
        self.sync.rebuild().await?;
        Ok(deleted_model_ids)
    }

    pub async fn delete_field(&self, command: DeleteModelFieldCommand) -> Result<()> {
        self.model_definitions.delete_field(command).await?;
        self.sync.rebuild().await?;
        Ok(())
    }
}
