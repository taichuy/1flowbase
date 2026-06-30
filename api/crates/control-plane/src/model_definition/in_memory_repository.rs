use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use domain::DataModelScopeKind;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{
        AddModelFieldInput, CreateModelDefinitionInput, CreateScopeDataModelGrantInput,
        ModelDefinitionRepository, ReconcileSystemModelDefinitionInput,
        ReconcileSystemModelFieldInput, UpdateModelDefinitionInput,
        UpdateModelDefinitionStatusInput, UpdateModelFieldInput, UpdateScopeDataModelGrantInput,
    },
};

use super::{
    naming::{build_physical_column_name, build_physical_table_name, registered_system_table_name},
    service::ModelDefinitionService,
};

#[derive(Default, Clone)]
pub struct InMemoryModelDefinitionRepository {
    models: Arc<Mutex<HashMap<Uuid, domain::ModelDefinitionRecord>>>,
    data_source_defaults: Arc<Mutex<HashMap<(Uuid, Uuid), domain::DataSourceDefaults>>>,
    main_source_defaults: Arc<Mutex<HashMap<Uuid, domain::DataSourceDefaults>>>,
    grants: Arc<Mutex<Vec<domain::ScopeDataModelGrantRecord>>>,
    audit_logs: Arc<Mutex<Vec<domain::AuditLogRecord>>>,
}

impl InMemoryModelDefinitionRepository {
    pub fn with_data_source_defaults(
        data_source_instance_id: Uuid,
        defaults: domain::DataSourceDefaults,
    ) -> Self {
        Self {
            models: Arc::default(),
            data_source_defaults: Arc::new(Mutex::new(HashMap::from([(
                (Uuid::nil(), data_source_instance_id),
                defaults,
            )]))),
            main_source_defaults: Arc::default(),
            grants: Arc::default(),
            audit_logs: Arc::default(),
        }
    }

    pub fn with_main_source_defaults(defaults: domain::DataSourceDefaults) -> Self {
        Self {
            models: Arc::default(),
            data_source_defaults: Arc::default(),
            main_source_defaults: Arc::new(Mutex::new(HashMap::from([(Uuid::nil(), defaults)]))),
            grants: Arc::default(),
            audit_logs: Arc::default(),
        }
    }

    pub fn replace_grant_permission_profile_for_tests(
        &self,
        data_model_id: Uuid,
        permission_profile: domain::ScopeDataModelPermissionProfile,
    ) {
        let mut grants = self.grants.lock().expect("in-memory grant lock poisoned");
        for grant in grants
            .iter_mut()
            .filter(|grant| grant.data_model_id == data_model_id)
        {
            grant.permission_profile = permission_profile;
        }
    }

    pub fn audit_events(&self) -> Vec<String> {
        self.audit_logs
            .lock()
            .expect("in-memory audit log lock poisoned")
            .iter()
            .map(|event| event.event_code.clone())
            .collect()
    }

    fn upsert_placeholder(&self, model_id: Uuid) -> domain::ModelDefinitionRecord {
        let mut models = self.models.lock().expect("in-memory model lock poisoned");
        let entry = models
            .entry(model_id)
            .or_insert_with(|| domain::ModelDefinitionRecord {
                id: model_id,
                scope_kind: DataModelScopeKind::Workspace,
                scope_id: Uuid::nil(),
                code: if model_id.is_nil() {
                    "nil".to_string()
                } else {
                    format!("model_{}", model_id.simple())
                },
                title: "Runtime Model".to_string(),
                physical_table_name: format!("rtm_workspace_00000000_{}", model_id.simple()),
                acl_namespace: "state_model.runtime_model".to_string(),
                audit_namespace: "audit.state_model.runtime_model".to_string(),
                fields: vec![],
                availability_status: domain::MetadataAvailabilityStatus::Available,
                data_source_instance_id: None,
                source_kind: domain::DataModelSourceKind::MainSource,
                external_resource_key: None,
                external_table_id: None,
                external_capability_snapshot: None,
                status: domain::DataModelStatus::Published,
                api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
                protection: domain::DataModelProtection::default(),
            });
        entry.clone()
    }
}

#[async_trait]
impl ModelDefinitionRepository for InMemoryModelDefinitionRepository {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        Ok(domain::ActorContext::root(
            actor_user_id,
            Uuid::nil(),
            "root",
        ))
    }

    async fn list_model_definitions(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::ModelDefinitionRecord>> {
        let models = self.models.lock().expect("in-memory model lock poisoned");
        Ok(models.values().cloned().collect())
    }

    async fn get_model_definition(
        &self,
        workspace_id: Uuid,
        model_id: Uuid,
    ) -> Result<Option<domain::ModelDefinitionRecord>> {
        let models = self.models.lock().expect("in-memory model lock poisoned");
        Ok(models
            .get(&model_id)
            .filter(|model| {
                workspace_id.is_nil()
                    || !matches!(model.scope_kind, DataModelScopeKind::Workspace)
                    || model.scope_id == workspace_id
            })
            .cloned())
    }

    async fn get_data_source_defaults(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
    ) -> Result<domain::DataSourceDefaults> {
        self.data_source_defaults
            .lock()
            .expect("in-memory data source defaults lock poisoned")
            .get(&(workspace_id, data_source_instance_id))
            .copied()
            .ok_or_else(|| ControlPlaneError::NotFound("data_source_instance").into())
    }

    async fn get_main_source_defaults(
        &self,
        workspace_id: Uuid,
    ) -> Result<domain::DataSourceDefaults> {
        Ok(self
            .main_source_defaults
            .lock()
            .expect("in-memory main source defaults lock poisoned")
            .get(&workspace_id)
            .copied()
            .unwrap_or_default())
    }

    async fn create_model_definition(
        &self,
        input: &CreateModelDefinitionInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        let model = domain::ModelDefinitionRecord {
            id: Uuid::now_v7(),
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            data_source_instance_id: input.data_source_instance_id,
            source_kind: input.source_kind,
            external_resource_key: input.external_resource_key.clone(),
            external_table_id: input.external_table_id.clone(),
            external_capability_snapshot: input.external_capability_snapshot.clone(),
            code: input.code.clone(),
            title: input.title.clone(),
            physical_table_name: registered_system_table_name(
                input.scope_kind,
                input.source_kind,
                &input.protection,
                &input.code,
            )
            .map(str::to_string)
            .unwrap_or_else(|| build_physical_table_name(input.scope_kind, &input.code)),
            acl_namespace: format!("state_model.{}", input.code),
            audit_namespace: format!("audit.state_model.{}", input.code),
            fields: vec![],
            availability_status: domain::MetadataAvailabilityStatus::Available,
            status: input.status,
            api_exposure_status: input.api_exposure_status,
            protection: input.protection.clone(),
        };
        self.models
            .lock()
            .expect("in-memory model lock poisoned")
            .insert(model.id, model.clone());
        Ok(model)
    }

    async fn update_model_definition(
        &self,
        input: &UpdateModelDefinitionInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        let mut models = self.models.lock().expect("in-memory model lock poisoned");
        let model = models
            .get_mut(&input.model_id)
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        model.title = input.title.clone();
        model.external_table_id = input.external_table_id.clone();
        Ok(model.clone())
    }

    async fn reconcile_system_model_definition(
        &self,
        input: &ReconcileSystemModelDefinitionInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        let mut models = self.models.lock().expect("in-memory model lock poisoned");
        let model = models
            .get_mut(&input.model_id)
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        model.title = input.title.clone();
        model.physical_table_name = input.physical_table_name.clone();
        model.status = input.status;
        model.api_exposure_status = input.api_exposure_status;
        model.protection = input.protection.clone();
        Ok(model.clone())
    }

    async fn update_model_definition_status(
        &self,
        input: &UpdateModelDefinitionStatusInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        let mut models = self.models.lock().expect("in-memory model lock poisoned");
        let model = models
            .get_mut(&input.model_id)
            .filter(|model| {
                input.workspace_id.is_nil()
                    || !matches!(model.scope_kind, DataModelScopeKind::Workspace)
                    || model.scope_id == input.workspace_id
            })
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        model.status = input.status;
        model.api_exposure_status = input.api_exposure_status;
        Ok(model.clone())
    }

    async fn add_model_field(
        &self,
        input: &AddModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        let mut models = self.models.lock().expect("in-memory model lock poisoned");
        let model = models
            .get_mut(&input.model_id)
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let field = domain::ModelFieldRecord {
            id: Uuid::now_v7(),
            data_model_id: input.model_id,
            code: input.code.clone(),
            title: input.title.clone(),
            description: input.description.clone(),
            physical_column_name: input
                .physical_column_name
                .clone()
                .unwrap_or_else(|| build_physical_column_name(&input.code)),
            external_field_key: input.external_field_key.clone(),
            field_kind: input.field_kind,
            is_system: input.is_system,
            is_writable: input.is_writable,
            is_required: input.is_required,
            api_required: input.api_required,
            is_unique: input.is_unique,
            default_value: input.default_value.clone(),
            display_interface: input.display_interface.clone(),
            display_options: input.display_options.clone(),
            relation_target_model_id: input.relation_target_model_id,
            relation_options: input.relation_options.clone(),
            sort_order: model.fields.len() as i32,
            availability_status: domain::MetadataAvailabilityStatus::Available,
        };
        model.fields.push(field.clone());
        Ok(field)
    }

    async fn update_model_field(
        &self,
        input: &UpdateModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        let mut models = self.models.lock().expect("in-memory model lock poisoned");
        let model = models
            .get_mut(&input.model_id)
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let field = model
            .fields
            .iter_mut()
            .find(|field| field.id == input.field_id)
            .ok_or(ControlPlaneError::NotFound("model_field"))?;
        field.title = input.title.clone();
        field.description = input.description.clone();
        field.is_required = input.is_required;
        field.api_required = input.api_required;
        field.is_unique = input.is_unique;
        field.default_value = input.default_value.clone();
        field.display_interface = input.display_interface.clone();
        field.display_options = input.display_options.clone();
        field.relation_options = input.relation_options.clone();

        Ok(field.clone())
    }

    async fn reconcile_system_model_field(
        &self,
        input: &ReconcileSystemModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        let mut models = self.models.lock().expect("in-memory model lock poisoned");
        let model = models
            .get_mut(&input.model_id)
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let field = model
            .fields
            .iter_mut()
            .find(|field| field.id == input.field_id)
            .ok_or(ControlPlaneError::NotFound("model_field"))?;
        field.title = input.title.clone();
        field.description = input.description.clone();
        field.physical_column_name = input.physical_column_name.clone();
        field.external_field_key = input.external_field_key.clone();
        field.field_kind = input.field_kind;
        field.is_system = input.is_system;
        field.is_writable = input.is_writable;
        field.is_required = input.is_required;
        field.api_required = input.api_required;
        field.is_unique = input.is_unique;
        field.default_value = input.default_value.clone();
        field.display_interface = input.display_interface.clone();
        field.display_options = input.display_options.clone();
        field.relation_target_model_id = input.relation_target_model_id;
        field.relation_options = input.relation_options.clone();
        field.sort_order = input.sort_order;
        field.availability_status = input.availability_status;
        Ok(field.clone())
    }

    async fn delete_model_definition(&self, _actor_user_id: Uuid, model_id: Uuid) -> Result<()> {
        let removed = self
            .models
            .lock()
            .expect("in-memory model lock poisoned")
            .remove(&model_id);
        if removed.is_some() {
            Ok(())
        } else {
            Err(ControlPlaneError::NotFound("model_definition").into())
        }
    }

    async fn delete_model_field(
        &self,
        _actor_user_id: Uuid,
        model_id: Uuid,
        field_id: Uuid,
    ) -> Result<()> {
        let mut models = self.models.lock().expect("in-memory model lock poisoned");
        let model = models
            .get_mut(&model_id)
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let original_len = model.fields.len();
        model.fields.retain(|field| field.id != field_id);
        if model.fields.len() == original_len {
            Err(ControlPlaneError::NotFound("model_field").into())
        } else {
            Ok(())
        }
    }

    async fn publish_model_definition(
        &self,
        _actor_user_id: Uuid,
        model_id: Uuid,
    ) -> Result<domain::ModelDefinitionRecord> {
        Ok(self.upsert_placeholder(model_id))
    }

    async fn create_scope_data_model_grant(
        &self,
        input: &CreateScopeDataModelGrantInput,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        self.models
            .lock()
            .expect("in-memory model lock poisoned")
            .get(&input.data_model_id)
            .filter(|model| matches!(model.scope_kind, DataModelScopeKind::System))
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;

        let grant = domain::ScopeDataModelGrantRecord {
            id: input.grant_id,
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            data_model_id: input.data_model_id,
            enabled: input.enabled,
            permission_profile: input.permission_profile,
            created_by: input.created_by,
            created_at: time::OffsetDateTime::now_utc(),
            updated_at: time::OffsetDateTime::now_utc(),
        };
        self.grants
            .lock()
            .expect("in-memory grant lock poisoned")
            .push(grant.clone());
        Ok(grant)
    }

    async fn update_scope_data_model_grant(
        &self,
        input: &UpdateScopeDataModelGrantInput,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        self.models
            .lock()
            .expect("in-memory model lock poisoned")
            .get(&input.data_model_id)
            .filter(|model| matches!(model.scope_kind, DataModelScopeKind::System))
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;

        let mut grants = self.grants.lock().expect("in-memory grant lock poisoned");
        let grant = grants
            .iter_mut()
            .find(|grant| grant.id == input.grant_id && grant.data_model_id == input.data_model_id)
            .ok_or(ControlPlaneError::NotFound("scope_data_model_grant"))?;
        grant.enabled = input.enabled;
        grant.permission_profile = input.permission_profile;
        grant.updated_at = time::OffsetDateTime::now_utc();
        Ok(grant.clone())
    }

    async fn get_scope_data_model_grant(
        &self,
        data_model_id: Uuid,
        grant_id: Uuid,
    ) -> Result<Option<domain::ScopeDataModelGrantRecord>> {
        Ok(self
            .grants
            .lock()
            .expect("in-memory grant lock poisoned")
            .iter()
            .find(|grant| grant.id == grant_id && grant.data_model_id == data_model_id)
            .cloned())
    }

    async fn delete_scope_data_model_grant(
        &self,
        data_model_id: Uuid,
        grant_id: Uuid,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        let mut grants = self.grants.lock().expect("in-memory grant lock poisoned");
        let index = grants
            .iter()
            .position(|grant| grant.id == grant_id && grant.data_model_id == data_model_id)
            .ok_or(ControlPlaneError::NotFound("scope_data_model_grant"))?;
        Ok(grants.remove(index))
    }

    async fn list_scope_data_model_grants(
        &self,
        scope_kind: DataModelScopeKind,
        scope_id: Uuid,
    ) -> Result<Vec<domain::ScopeDataModelGrantRecord>> {
        Ok(self
            .grants
            .lock()
            .expect("in-memory grant lock poisoned")
            .iter()
            .filter(|grant| grant.scope_kind == scope_kind && grant.scope_id == scope_id)
            .cloned()
            .collect())
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        self.audit_logs
            .lock()
            .expect("in-memory audit log lock poisoned")
            .push(event.clone());
        Ok(())
    }
}

impl ModelDefinitionService<InMemoryModelDefinitionRepository> {
    pub fn for_tests() -> Self {
        Self::new(InMemoryModelDefinitionRepository::default())
    }
}
