use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane::{
    model_definition::{CreateScopeDataModelGrantCommand, ModelDefinitionService},
    ports::{
        AddModelFieldInput, CreateModelDefinitionInput, CreateScopeDataModelGrantInput,
        ModelDefinitionRepository, UpdateModelDefinitionInput, UpdateModelFieldInput,
    },
};
use domain::{
    ActorContext, AuditLogRecord, DataModelScopeKind, ModelDefinitionRecord, ModelFieldRecord,
    ScopeDataModelGrantRecord, ScopeDataModelPermissionProfile, SYSTEM_SCOPE_ID,
};
use uuid::Uuid;

#[derive(Clone)]
struct AclTestRepository {
    actors: Arc<Mutex<HashMap<Uuid, ActorContext>>>,
    models: Arc<Mutex<HashMap<Uuid, ModelDefinitionRecord>>>,
    grants: Arc<Mutex<Vec<ScopeDataModelGrantRecord>>>,
}

impl AclTestRepository {
    fn new(actor: ActorContext, model: ModelDefinitionRecord) -> Self {
        Self {
            actors: Arc::new(Mutex::new(HashMap::from([(actor.user_id, actor)]))),
            models: Arc::new(Mutex::new(HashMap::from([(model.id, model)]))),
            grants: Arc::default(),
        }
    }
}

#[async_trait]
impl ModelDefinitionRepository for AclTestRepository {
    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        self.actors
            .lock()
            .expect("actor lock poisoned")
            .get(&actor_user_id)
            .cloned()
            .ok_or_else(|| anyhow!("missing actor"))
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
        _input: &CreateModelDefinitionInput,
    ) -> Result<ModelDefinitionRecord> {
        unimplemented!("not needed for ACL detail tests")
    }

    async fn update_model_definition(
        &self,
        _input: &UpdateModelDefinitionInput,
    ) -> Result<ModelDefinitionRecord> {
        unimplemented!("not needed for ACL detail tests")
    }

    async fn add_model_field(&self, _input: &AddModelFieldInput) -> Result<ModelFieldRecord> {
        unimplemented!("not needed for ACL detail tests")
    }

    async fn update_model_field(&self, _input: &UpdateModelFieldInput) -> Result<ModelFieldRecord> {
        unimplemented!("not needed for ACL detail tests")
    }

    async fn delete_model_definition(&self, _actor_user_id: Uuid, _model_id: Uuid) -> Result<()> {
        unimplemented!("not needed for ACL detail tests")
    }

    async fn delete_model_field(
        &self,
        _actor_user_id: Uuid,
        _model_id: Uuid,
        _field_id: Uuid,
    ) -> Result<()> {
        unimplemented!("not needed for ACL detail tests")
    }

    async fn publish_model_definition(
        &self,
        _actor_user_id: Uuid,
        _model_id: Uuid,
    ) -> Result<ModelDefinitionRecord> {
        unimplemented!("not needed for ACL detail tests")
    }

    async fn create_scope_data_model_grant(
        &self,
        input: &CreateScopeDataModelGrantInput,
    ) -> Result<ScopeDataModelGrantRecord> {
        let grant = ScopeDataModelGrantRecord {
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

fn sample_model(model_id: Uuid) -> ModelDefinitionRecord {
    ModelDefinitionRecord {
        id: model_id,
        scope_kind: DataModelScopeKind::Workspace,
        scope_id: Uuid::now_v7(),
        code: "orders".to_string(),
        title: "Orders".to_string(),
        physical_table_name: "rtm_workspace_orders".to_string(),
        acl_namespace: "state_model.orders".to_string(),
        audit_namespace: "audit.state_model.orders".to_string(),
        fields: vec![],
        availability_status: domain::MetadataAvailabilityStatus::Available,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_table_id: None,
        external_capability_snapshot: None,
        status: domain::DataModelStatus::Published,
        protection: domain::DataModelProtection::default(),
    }
}

fn sample_system_model(model_id: Uuid) -> ModelDefinitionRecord {
    ModelDefinitionRecord {
        scope_kind: DataModelScopeKind::System,
        scope_id: SYSTEM_SCOPE_ID,
        physical_table_name: "rtm_system_orders".to_string(),
        ..sample_model(model_id)
    }
}

#[tokio::test]
async fn get_model_requires_state_model_visibility() {
    let actor_user_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let service = ModelDefinitionService::new(AclTestRepository::new(
        ActorContext::scoped(
            actor_user_id,
            Uuid::now_v7(),
            "viewer",
            Vec::<String>::new(),
        ),
        sample_model(model_id),
    ));

    let error = service
        .get_model(actor_user_id, model_id)
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("permission denied: permission_denied"));
}

#[tokio::test]
async fn state_model_own_is_treated_as_scope_shared_read() {
    let actor_user_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let service = ModelDefinitionService::new(AclTestRepository::new(
        ActorContext::scoped(
            actor_user_id,
            Uuid::now_v7(),
            "viewer",
            ["state_model.view.own".to_string()],
        ),
        sample_model(model_id),
    ));

    let model = service.get_model(actor_user_id, model_id).await.unwrap();

    assert_eq!(model.id, model_id);
    assert_eq!(model.code, "orders");
}

#[tokio::test]
async fn system_creates_scope_grant_for_single_machine_default_scope() {
    let actor_user_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let repository = AclTestRepository::new(
        ActorContext::root(actor_user_id, SYSTEM_SCOPE_ID, "root"),
        sample_system_model(model_id),
    );
    let service = ModelDefinitionService::new(repository);

    let grant = service
        .create_scope_grant(CreateScopeDataModelGrantCommand {
            actor_user_id,
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_model_id: model_id,
            enabled: true,
            permission_profile: "scope_all".into(),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap();

    assert_eq!(grant.scope_kind, DataModelScopeKind::System);
    assert_eq!(grant.scope_id, SYSTEM_SCOPE_ID);
    assert_eq!(
        grant.permission_profile,
        ScopeDataModelPermissionProfile::ScopeAll
    );
}

#[tokio::test]
async fn scope_grant_rejects_unknown_permission_profile_at_service_boundary() {
    let actor_user_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let service = ModelDefinitionService::new(AclTestRepository::new(
        ActorContext::root(actor_user_id, SYSTEM_SCOPE_ID, "root"),
        sample_system_model(model_id),
    ));

    let error = service
        .create_scope_grant(CreateScopeDataModelGrantCommand {
            actor_user_id,
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_model_id: model_id,
            enabled: true,
            permission_profile: "workspace_admin".into(),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("invalid input: permission_profile"));
}

#[tokio::test]
async fn runtime_scope_grant_loader_converts_current_workspace_persisted_grant() {
    let actor_user_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let repository = AclTestRepository::new(
        ActorContext::scoped(
            actor_user_id,
            workspace_id,
            "admin",
            ["state_model.manage.all".to_string()],
        ),
        sample_system_model(model_id),
    );
    let service = ModelDefinitionService::new(repository);
    service
        .create_scope_grant(CreateScopeDataModelGrantCommand {
            actor_user_id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            data_model_id: model_id,
            enabled: true,
            permission_profile: "scope_all".into(),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap();

    let grant = service
        .load_runtime_scope_grant(
            &ActorContext::scoped(Uuid::now_v7(), workspace_id, "member", Vec::<String>::new()),
            model_id,
        )
        .await
        .unwrap()
        .expect("persisted workspace grant should be selected");

    assert_eq!(grant.data_model_id, model_id);
    assert_eq!(grant.scope_kind, DataModelScopeKind::Workspace);
    assert_eq!(grant.scope_id, workspace_id);
    assert_eq!(
        grant.permission_profile,
        ScopeDataModelPermissionProfile::ScopeAll
    );
}

#[tokio::test]
async fn runtime_scope_grant_loader_does_not_select_system_default_for_non_system_actor() {
    let actor_user_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let repository = AclTestRepository::new(
        ActorContext::root(actor_user_id, SYSTEM_SCOPE_ID, "root"),
        sample_system_model(model_id),
    );
    let service = ModelDefinitionService::new(repository);
    service
        .create_scope_grant(CreateScopeDataModelGrantCommand {
            actor_user_id,
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_model_id: model_id,
            enabled: true,
            permission_profile: "system_all".into(),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap();

    let grant = service
        .load_runtime_scope_grant(
            &ActorContext::scoped(Uuid::now_v7(), workspace_id, "member", Vec::<String>::new()),
            model_id,
        )
        .await
        .unwrap();

    assert!(grant.is_none());
}
