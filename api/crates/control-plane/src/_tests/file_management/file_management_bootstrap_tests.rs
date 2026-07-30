use control_plane::_tests::support::MemoryProvisioningRepository;
use control_plane::file_management::{
    attachments_template_fields, file_metadata_title_references, project_attachments_model_titles,
    project_builtin_file_table_title, CreateWorkspaceFileTableCommand,
    FileManagementBootstrapService, FileTableProvisioningService,
};
use control_plane::i18n_catalog::CatalogResolver;
use control_plane::ports::{
    CatalogResolutionCandidate, CatalogResolutionRepository, ModelDefinitionRepository,
};
use domain::{
    DataModelScopeKind, FileTableScopeKind, ScopeDataModelPermissionProfile, SYSTEM_SCOPE_ID,
};
use uuid::Uuid;

#[derive(Clone, Copy)]
struct FileMetadataTranslationFixture;

#[async_trait::async_trait]
impl CatalogResolutionRepository for FileMetadataTranslationFixture {
    async fn find_catalog_resolution_candidate(
        &self,
        _workspace_id: Uuid,
        identity: &domain::CatalogMessageIdentity,
        locale: &domain::CatalogLocale,
    ) -> anyhow::Result<CatalogResolutionCandidate> {
        Ok(CatalogResolutionCandidate {
            root_override: None,
            active_official: (locale.as_str() == "zh_Hans")
                .then(|| format!("zh:{}", identity.key())),
        })
    }
}

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

#[test]
fn ac_010_file_metadata_inventory_has_10_stable_english_references() {
    let references = file_metadata_title_references();
    assert_eq!(references.len(), 10);
    assert_eq!(
        references
            .iter()
            .map(|reference| reference.historical_default)
            .collect::<Vec<_>>(),
        vec![
            "Attachments",
            "标题",
            "文件名",
            "扩展名",
            "大小",
            "MIME 类型",
            "存储路径",
            "元数据",
            "缓存地址",
            "存储器 ID",
        ]
    );
    assert_eq!(
        references
            .iter()
            .map(|reference| (reference.resource_code, reference.field_code, reference.key))
            .collect::<Vec<_>>(),
        vec![
            ("attachments", None, "Attachments"),
            ("attachments", Some("title"), "Title"),
            ("attachments", Some("filename"), "Filename"),
            ("attachments", Some("extname"), "Extension"),
            ("attachments", Some("size"), "Size"),
            ("attachments", Some("mimetype"), "MIME Type"),
            ("attachments", Some("path"), "Storage Path"),
            ("attachments", Some("meta"), "Metadata"),
            ("attachments", Some("url"), "Cached URL"),
            ("attachments", Some("storage_id"), "Storage ID"),
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
async fn ac_012_013_file_metadata_projection_localizes_defaults_and_preserves_custom_titles() {
    let repository = MemoryProvisioningRepository::default();
    let table = FileManagementBootstrapService::new(repository.clone())
        .ensure_builtin_attachments(Uuid::now_v7(), Uuid::now_v7(), "attachments")
        .await
        .unwrap();
    let mut model = ModelDefinitionRepository::get_model_definition(
        &repository,
        SYSTEM_SCOPE_ID,
        table.model_definition_id,
    )
    .await
    .unwrap()
    .unwrap();
    model
        .fields
        .iter_mut()
        .find(|field| field.code == "title")
        .unwrap()
        .title = "Administrator File Label".into();
    model
        .fields
        .iter_mut()
        .find(|field| field.code == "filename")
        .unwrap()
        .title = "文件名".into();

    let workspace_id = Uuid::now_v7();
    let locale = domain::CatalogLocale::new("zh_Hans").unwrap();
    let resolver = CatalogResolver::new(FileMetadataTranslationFixture, workspace_id);
    project_attachments_model_titles(&resolver, workspace_id, &locale, &mut model)
        .await
        .unwrap();
    assert_eq!(model.title, "zh:Attachments");
    assert_eq!(
        model
            .fields
            .iter()
            .find(|field| field.code == "filename")
            .unwrap()
            .title,
        "zh:Filename"
    );
    assert_eq!(
        model
            .fields
            .iter()
            .find(|field| field.code == "title")
            .unwrap()
            .title,
        "Administrator File Label"
    );

    let mut english_model = model.clone();
    english_model
        .fields
        .iter_mut()
        .find(|field| field.code == "size")
        .unwrap()
        .title = "大小".into();
    let en_us = domain::CatalogLocale::new("en_US").unwrap();
    project_attachments_model_titles(&resolver, workspace_id, &en_us, &mut english_model)
        .await
        .unwrap();
    assert_eq!(
        english_model
            .fields
            .iter()
            .find(|field| field.code == "size")
            .unwrap()
            .title,
        "Size"
    );

    let mut localized_table = table.clone();
    localized_table.title.clear();
    project_builtin_file_table_title(&resolver, workspace_id, &locale, &mut localized_table)
        .await
        .unwrap();
    assert_eq!(localized_table.title, "zh:Attachments");

    let mut customized_table = table;
    customized_table.title = "Legal Documents".into();
    project_builtin_file_table_title(&resolver, workspace_id, &locale, &mut customized_table)
        .await
        .unwrap();
    assert_eq!(customized_table.title, "Legal Documents");
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
