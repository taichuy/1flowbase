use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::ports::{
    AddModelFieldInput, CreateFileStorageInput, CreateFileTableRegistrationInput,
    CreateModelDefinitionInput, CreateScopeDataModelGrantInput, FileManagementRepository,
    ModelDefinitionRepository, UpdateFileStorageBindingInput, UpdateModelDefinitionInput,
    UpdateModelFieldInput,
};
use domain::{
    ActorContext, AuditLogRecord, DataModelScopeKind, FileStorageRecord, FileTableRecord,
    FileTableScopeKind, MetadataAvailabilityStatus, ModelDefinitionRecord, ModelFieldRecord,
    ScopeDataModelGrantRecord,
};

fn memory_physical_table_name(input: &CreateModelDefinitionInput) -> String {
    if input.scope_kind == domain::DataModelScopeKind::System
        && input.source_kind == domain::DataModelSourceKind::MainSource
        && input.protection.owner_kind == domain::DataModelOwnerKind::Core
        && input.protection.is_protected
        && matches!(input.code.as_str(), "attachments" | "users" | "roles")
    {
        return input.code.clone();
    }

    format!("rtm_{}_{}", input.scope_kind.as_str(), input.code)
}

#[derive(Clone, Default)]
pub struct MemoryProvisioningRepository {
    models: Arc<Mutex<HashMap<Uuid, ModelDefinitionRecord>>>,
    grants: Arc<Mutex<Vec<ScopeDataModelGrantRecord>>>,
    pub file_tables: Arc<Mutex<Vec<FileTableRecord>>>,
}

impl MemoryProvisioningRepository {
    pub fn recorded_file_tables(&self) -> Vec<FileTableRecord> {
        self.file_tables
            .lock()
            .expect("file table lock should be free in assertions")
            .clone()
    }
}

#[async_trait]
impl FileManagementRepository for MemoryProvisioningRepository {
    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        Ok(ActorContext::root(actor_user_id, Uuid::nil(), "root"))
    }

    async fn find_file_table_by_code(&self, code: &str) -> Result<Option<FileTableRecord>> {
        Ok(self
            .file_tables
            .lock()
            .expect("file table lock poisoned")
            .iter()
            .find(|record| record.code == code)
            .cloned())
    }

    async fn get_file_table(&self, file_table_id: Uuid) -> Result<Option<FileTableRecord>> {
        Ok(self
            .file_tables
            .lock()
            .expect("file table lock poisoned")
            .iter()
            .find(|record| record.id == file_table_id)
            .cloned())
    }

    async fn create_file_storage(
        &self,
        _input: &CreateFileStorageInput,
    ) -> Result<FileStorageRecord> {
        anyhow::bail!("not implemented for provisioning tests")
    }

    async fn create_file_table_registration(
        &self,
        input: &CreateFileTableRegistrationInput,
    ) -> Result<FileTableRecord> {
        let now = OffsetDateTime::now_utc();
        let record = FileTableRecord {
            id: input.file_table_id,
            code: input.code.clone(),
            title: input.title.clone(),
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            model_definition_id: input.model_definition_id,
            bound_storage_id: input.bound_storage_id,
            is_builtin: input.is_builtin,
            is_default: input.is_default,
            status: "active".to_string(),
            created_by: input.actor_user_id,
            updated_by: input.actor_user_id,
            created_at: now,
            updated_at: now,
        };
        self.file_tables
            .lock()
            .expect("file table lock poisoned")
            .push(record.clone());
        Ok(record)
    }

    async fn list_file_storages(&self) -> Result<Vec<FileStorageRecord>> {
        Ok(vec![])
    }

    async fn get_default_file_storage(&self) -> Result<Option<FileStorageRecord>> {
        Ok(None)
    }

    async fn get_file_storage(&self, _storage_id: Uuid) -> Result<Option<FileStorageRecord>> {
        Ok(None)
    }

    async fn list_visible_file_tables(&self, workspace_id: Uuid) -> Result<Vec<FileTableRecord>> {
        Ok(self
            .file_tables
            .lock()
            .expect("file table lock poisoned")
            .iter()
            .filter(|record| {
                matches!(record.scope_kind, FileTableScopeKind::System)
                    || record.scope_id == workspace_id
            })
            .cloned()
            .collect())
    }

    async fn update_file_table_binding(
        &self,
        _input: &UpdateFileStorageBindingInput,
    ) -> Result<FileTableRecord> {
        anyhow::bail!("not implemented for provisioning tests")
    }
}

#[async_trait]
impl ModelDefinitionRepository for MemoryProvisioningRepository {
    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        Ok(ActorContext::root(actor_user_id, Uuid::nil(), "root"))
    }

    async fn list_model_definitions(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<ModelDefinitionRecord>> {
        Ok(self
            .models
            .lock()
            .expect("model lock poisoned")
            .values()
            .cloned()
            .collect())
    }

    async fn get_model_definition(
        &self,
        _workspace_id: Uuid,
        model_id: Uuid,
    ) -> Result<Option<ModelDefinitionRecord>> {
        Ok(self
            .models
            .lock()
            .expect("model lock poisoned")
            .get(&model_id)
            .cloned())
    }

    async fn create_model_definition(
        &self,
        input: &CreateModelDefinitionInput,
    ) -> Result<ModelDefinitionRecord> {
        let model = ModelDefinitionRecord {
            id: Uuid::now_v7(),
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            code: input.code.clone(),
            title: input.title.clone(),
            physical_table_name: memory_physical_table_name(input),
            acl_namespace: format!("state_model.{}", input.code),
            audit_namespace: format!("audit.state_model.{}", input.code),
            fields: vec![],
            availability_status: MetadataAvailabilityStatus::Available,
            data_source_instance_id: input.data_source_instance_id,
            source_kind: input.source_kind,
            external_resource_key: input.external_resource_key.clone(),
            external_table_id: input.external_table_id.clone(),
            external_capability_snapshot: None,
            status: input.status,
            api_exposure_status: input.api_exposure_status,
            protection: input.protection.clone(),
        };
        self.models
            .lock()
            .expect("model lock poisoned")
            .insert(model.id, model.clone());
        Ok(model)
    }

    async fn update_model_definition(
        &self,
        input: &UpdateModelDefinitionInput,
    ) -> Result<ModelDefinitionRecord> {
        let mut models = self.models.lock().expect("model lock poisoned");
        let model = models
            .get_mut(&input.model_id)
            .expect("model should exist for test updates");
        model.title = input.title.clone();
        Ok(model.clone())
    }

    async fn add_model_field(&self, input: &AddModelFieldInput) -> Result<ModelFieldRecord> {
        let mut models = self.models.lock().expect("model lock poisoned");
        let model = models
            .get_mut(&input.model_id)
            .expect("model should exist for field inserts");
        let field = ModelFieldRecord {
            id: Uuid::now_v7(),
            data_model_id: input.model_id,
            code: input.code.clone(),
            title: input.title.clone(),
            physical_column_name: input
                .physical_column_name
                .clone()
                .unwrap_or_else(|| input.code.replace('-', "_")),
            external_field_key: input.external_field_key.clone(),
            field_kind: input.field_kind,
            is_system: input.is_system,
            is_writable: input.is_writable,
            is_required: input.is_required,
            is_unique: input.is_unique,
            default_value: input.default_value.clone(),
            display_interface: input.display_interface.clone(),
            display_options: input.display_options.clone(),
            relation_target_model_id: input.relation_target_model_id,
            relation_options: input.relation_options.clone(),
            sort_order: model.fields.len() as i32,
            availability_status: MetadataAvailabilityStatus::Available,
        };
        model.fields.push(field.clone());
        Ok(field)
    }

    async fn update_model_field(&self, input: &UpdateModelFieldInput) -> Result<ModelFieldRecord> {
        let mut models = self.models.lock().expect("model lock poisoned");
        let model = models
            .get_mut(&input.model_id)
            .expect("model should exist for field updates");
        let field = model
            .fields
            .iter_mut()
            .find(|field| field.id == input.field_id)
            .expect("field should exist for updates");
        field.title = input.title.clone();
        field.is_required = input.is_required;
        field.is_unique = input.is_unique;
        field.default_value = input.default_value.clone();
        field.display_interface = input.display_interface.clone();
        field.display_options = input.display_options.clone();
        field.relation_options = input.relation_options.clone();
        Ok(field.clone())
    }

    async fn delete_model_definition(&self, _actor_user_id: Uuid, model_id: Uuid) -> Result<()> {
        self.models
            .lock()
            .expect("model lock poisoned")
            .remove(&model_id);
        Ok(())
    }

    async fn delete_model_field(
        &self,
        _actor_user_id: Uuid,
        model_id: Uuid,
        field_id: Uuid,
    ) -> Result<()> {
        let mut models = self.models.lock().expect("model lock poisoned");
        if let Some(model) = models.get_mut(&model_id) {
            model.fields.retain(|field| field.id != field_id);
        }
        Ok(())
    }

    async fn publish_model_definition(
        &self,
        _actor_user_id: Uuid,
        model_id: Uuid,
    ) -> Result<ModelDefinitionRecord> {
        Ok(self
            .models
            .lock()
            .expect("model lock poisoned")
            .get(&model_id)
            .expect("model should exist for publish")
            .clone())
    }

    async fn create_scope_data_model_grant(
        &self,
        input: &CreateScopeDataModelGrantInput,
    ) -> Result<ScopeDataModelGrantRecord> {
        self.models
            .lock()
            .expect("model lock poisoned")
            .get(&input.data_model_id)
            .filter(|model| matches!(model.scope_kind, DataModelScopeKind::System))
            .expect("grant target should be a system model");

        let now = OffsetDateTime::now_utc();
        let grant = ScopeDataModelGrantRecord {
            id: input.grant_id,
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            data_model_id: input.data_model_id,
            enabled: input.enabled,
            permission_profile: input.permission_profile,
            created_by: input.created_by,
            created_at: now,
            updated_at: now,
        };
        self.grants
            .lock()
            .expect("grant lock poisoned")
            .push(grant.clone());
        Ok(grant)
    }

    async fn list_scope_data_model_grants(
        &self,
        scope_kind: DataModelScopeKind,
        scope_id: Uuid,
    ) -> Result<Vec<ScopeDataModelGrantRecord>> {
        Ok(self
            .grants
            .lock()
            .expect("grant lock poisoned")
            .iter()
            .filter(|grant| grant.scope_kind == scope_kind && grant.scope_id == scope_id)
            .cloned()
            .collect())
    }

    async fn append_audit_log(&self, _event: &AuditLogRecord) -> Result<()> {
        Ok(())
    }
}
