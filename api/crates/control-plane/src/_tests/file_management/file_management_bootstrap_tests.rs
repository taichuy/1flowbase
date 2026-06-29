use control_plane::_tests::support::MemoryProvisioningRepository;
use control_plane::file_management::{
    attachments_template_fields, CreateWorkspaceFileTableCommand, FileManagementBootstrapService,
    FileTableProvisioningService,
};
use control_plane::ports::ModelDefinitionRepository;
use domain::{
    DataModelScopeKind, FileTableScopeKind, ScopeDataModelPermissionProfile, SYSTEM_SCOPE_ID,
};
use uuid::Uuid;

#[test]
fn attachments_template_fields_match_the_approved_v1_schema() {
    let codes = attachments_template_fields()
        .into_iter()
        .map(|field| field.code)
        .collect::<Vec<_>>();

    assert_eq!(
        codes,
        vec![
            "title",
            "filename",
            "extname",
            "size",
            "mimetype",
            "path",
            "meta",
            "url",
            "storage_id",
        ]
    );
}

#[tokio::test]
async fn bootstrap_creates_builtin_attachments_once() {
    let repository = MemoryProvisioningRepository::default();
    let service = FileManagementBootstrapService::new(repository.clone());

    let first = service
        .ensure_builtin_attachments(Uuid::now_v7(), Uuid::now_v7(), "attachments")
        .await
        .unwrap();
    let second = service
        .ensure_builtin_attachments(Uuid::now_v7(), first.bound_storage_id, "attachments")
        .await
        .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(first.scope_kind, FileTableScopeKind::System);
    assert_eq!(repository.recorded_file_tables().len(), 1);

    let model = ModelDefinitionRepository::get_model_definition(
        &repository,
        SYSTEM_SCOPE_ID,
        first.model_definition_id,
    )
    .await
    .unwrap()
    .expect("builtin attachments should create a model definition");
    assert_eq!(model.scope_kind, DataModelScopeKind::System);
    assert_eq!(model.scope_id, SYSTEM_SCOPE_ID);
    assert_eq!(model.physical_table_name, "attachments");
    assert_eq!(
        model.protection.owner_kind,
        domain::DataModelOwnerKind::Core
    );
    assert!(model.protection.is_protected);
    let field_codes = model
        .fields
        .iter()
        .map(|field| field.code.as_str())
        .collect::<Vec<_>>();
    assert!(field_codes.contains(&"id"));
    assert!(field_codes.contains(&"scope_id"));
    assert!(field_codes.contains(&"filename"));

    let grants = ModelDefinitionRepository::list_scope_data_model_grants(
        &repository,
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
    )
    .await
    .unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].data_model_id, model.id);
    assert_eq!(
        grants[0].permission_profile,
        ScopeDataModelPermissionProfile::SystemAll
    );
}

#[tokio::test]
async fn workspace_file_tables_create_system_model_and_workspace_grant() {
    let repository = MemoryProvisioningRepository::default();
    let service = FileTableProvisioningService::new(repository.clone());
    let default_storage_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();

    let created = service
        .create_workspace_file_table(CreateWorkspaceFileTableCommand {
            actor_user_id: Uuid::now_v7(),
            workspace_id,
            code: "project_assets".into(),
            title: "Project Assets".into(),
            default_storage_id,
        })
        .await
        .unwrap();

    assert_eq!(created.scope_kind, FileTableScopeKind::Workspace);
    assert_eq!(created.scope_id, workspace_id);
    assert_eq!(created.bound_storage_id, default_storage_id);
    assert_eq!(repository.recorded_file_tables().len(), 1);

    let model = ModelDefinitionRepository::get_model_definition(
        &repository,
        workspace_id,
        created.model_definition_id,
    )
    .await
    .unwrap()
    .expect("workspace file table should create a model definition");
    assert_eq!(model.scope_kind, DataModelScopeKind::System);
    assert_eq!(model.scope_id, SYSTEM_SCOPE_ID);

    let grants = ModelDefinitionRepository::list_scope_data_model_grants(
        &repository,
        DataModelScopeKind::Workspace,
        workspace_id,
    )
    .await
    .unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].data_model_id, model.id);
    assert_eq!(
        grants[0].permission_profile,
        ScopeDataModelPermissionProfile::ScopeAll
    );
}
