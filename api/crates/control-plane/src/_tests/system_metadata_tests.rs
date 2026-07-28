use control_plane::_tests::support::MemoryProvisioningRepository;
use control_plane::file_management::file_metadata_title_references;
use control_plane::i18n_catalog::CatalogResolver;
use control_plane::ports::{
    AddModelFieldInput, CatalogResolutionCandidate, CatalogResolutionRepository,
    CreateModelDefinitionInput, ModelDefinitionRepository,
};
use control_plane::system_metadata::{
    project_system_metadata_titles, role_metadata_template, system_metadata_title_references,
    user_metadata_template, SystemMetadataBootstrapService, SYSTEM_METADATA_CATALOG_MODULE,
};
use domain::{
    DataModelProtection, DataModelScopeKind, DataModelSourceKind, DataModelStatus, ModelFieldKind,
    ScopeDataModelPermissionProfile, SYSTEM_SCOPE_ID,
};
use uuid::Uuid;

#[derive(Clone, Copy)]
struct MetadataTranslationFixture {
    provide_zh_hans: bool,
}

#[async_trait::async_trait]
impl CatalogResolutionRepository for MetadataTranslationFixture {
    async fn find_catalog_resolution_candidate(
        &self,
        _workspace_id: Uuid,
        identity: &domain::CatalogMessageIdentity,
        locale: &domain::CatalogLocale,
    ) -> anyhow::Result<CatalogResolutionCandidate> {
        Ok(CatalogResolutionCandidate {
            root_override: None,
            active_official: (self.provide_zh_hans && locale.as_str() == "zh_Hans")
                .then(|| format!("zh:{}", identity.msgid())),
        })
    }
}

#[test]
fn user_and_role_metadata_templates_match_system_table_contract() {
    let user_codes = user_metadata_template()
        .fields
        .into_iter()
        .map(|field| field.code)
        .collect::<Vec<_>>();
    assert_eq!(
        user_codes,
        vec![
            "id",
            "created_by",
            "updated_by",
            "account",
            "email",
            "phone",
            "name",
            "nickname",
            "avatar_url",
            "introduction",
            "preferred_locale",
            "meta",
            "default_display_role",
            "email_login_enabled",
            "phone_login_enabled",
            "status",
            "created_at",
            "updated_at",
        ]
    );

    let role_codes = role_metadata_template()
        .fields
        .into_iter()
        .map(|field| field.code)
        .collect::<Vec<_>>();
    assert_eq!(
        role_codes,
        vec![
            "id",
            "created_by",
            "updated_by",
            "scope_id",
            "scope_kind",
            "workspace_id",
            "code",
            "name",
            "introduction",
            "is_builtin",
            "is_editable",
            "auto_grant_new_permissions",
            "is_default_member_role",
            "system_kind",
            "created_at",
            "updated_at",
        ]
    );

    let mut user_contract_codes = domain::builtin_data_model_contract("users")
        .expect("users builtin contract")
        .system_field_codes
        .to_vec();
    let mut role_contract_codes = domain::builtin_data_model_contract("roles")
        .expect("roles builtin contract")
        .system_field_codes
        .to_vec();
    let mut sorted_user_codes = user_codes;
    let mut sorted_role_codes = role_codes;
    sorted_user_codes.sort_unstable();
    sorted_role_codes.sort_unstable();
    user_contract_codes.sort_unstable();
    role_contract_codes.sort_unstable();
    assert_eq!(sorted_user_codes, user_contract_codes);
    assert_eq!(sorted_role_codes, role_contract_codes);
}

#[test]
fn ac_010_system_metadata_inventory_has_36_stable_english_references() {
    let references = system_metadata_title_references();
    assert_eq!(references.len(), 36);
    assert!(references
        .iter()
        .all(|reference| reference.module == SYSTEM_METADATA_CATALOG_MODULE));
    assert_eq!(
        references
            .iter()
            .map(|reference| reference.historical_default)
            .collect::<Vec<_>>(),
        vec![
            "用户",
            "用户 ID",
            "创建人",
            "更新人",
            "账号",
            "邮箱",
            "手机号",
            "姓名",
            "昵称",
            "头像",
            "简介",
            "偏好语言",
            "元数据",
            "默认展示角色",
            "邮箱登录",
            "手机登录",
            "状态",
            "创建时间",
            "更新时间",
            "角色",
            "角色 ID",
            "创建人",
            "更新人",
            "作用域 ID",
            "作用域",
            "工作区 ID",
            "角色标识",
            "角色名称",
            "简介",
            "内置角色",
            "可编辑",
            "自动授予新权限",
            "默认成员角色",
            "系统角色类型",
            "创建时间",
            "更新时间",
        ]
    );
    assert_eq!(
        references
            .iter()
            .map(|reference| (reference.model_code, reference.field_code, reference.msgid))
            .collect::<Vec<_>>(),
        vec![
            ("users", None, "Users"),
            ("users", Some("id"), "User ID"),
            ("users", Some("created_by"), "Created By"),
            ("users", Some("updated_by"), "Updated By"),
            ("users", Some("account"), "Account"),
            ("users", Some("email"), "Email"),
            ("users", Some("phone"), "Phone"),
            ("users", Some("name"), "Name"),
            ("users", Some("nickname"), "Nickname"),
            ("users", Some("avatar_url"), "Avatar"),
            ("users", Some("introduction"), "Introduction"),
            ("users", Some("preferred_locale"), "Preferred Language"),
            ("users", Some("meta"), "Metadata"),
            (
                "users",
                Some("default_display_role"),
                "Default Display Role"
            ),
            ("users", Some("email_login_enabled"), "Email Login"),
            ("users", Some("phone_login_enabled"), "Phone Login"),
            ("users", Some("status"), "Status"),
            ("users", Some("created_at"), "Created At"),
            ("users", Some("updated_at"), "Updated At"),
            ("roles", None, "Roles"),
            ("roles", Some("id"), "Role ID"),
            ("roles", Some("created_by"), "Created By"),
            ("roles", Some("updated_by"), "Updated By"),
            ("roles", Some("scope_id"), "Scope ID"),
            ("roles", Some("scope_kind"), "Scope"),
            ("roles", Some("workspace_id"), "Workspace ID"),
            ("roles", Some("code"), "Role Code"),
            ("roles", Some("name"), "Role Name"),
            ("roles", Some("introduction"), "Introduction"),
            ("roles", Some("is_builtin"), "Builtin Role"),
            ("roles", Some("is_editable"), "Editable"),
            (
                "roles",
                Some("auto_grant_new_permissions"),
                "Automatically Grant New Permissions",
            ),
            (
                "roles",
                Some("is_default_member_role"),
                "Default Member Role"
            ),
            ("roles", Some("system_kind"), "System Role Type"),
            ("roles", Some("created_at"), "Created At"),
            ("roles", Some("updated_at"), "Updated At"),
        ]
    );
}

#[test]
fn ac_010_machine_readable_metadata_consumer_inventory_matches_all_46_refs() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/metadata_i18n_consumers.json")).unwrap();
    let expected = fixture.as_array().unwrap();
    let actual = system_metadata_title_references()
        .into_iter()
        .map(|reference| {
            serde_json::json!({
                "module": reference.module,
                "msgid": reference.msgid,
                "resource": reference.model_code,
                "field": reference.field_code,
            })
        })
        .chain(
            file_metadata_title_references()
                .into_iter()
                .map(|reference| {
                    serde_json::json!({
                        "module": reference.module,
                        "msgid": reference.msgid,
                        "resource": reference.resource_code,
                        "field": reference.field_code,
                    })
                }),
        )
        .collect::<Vec<_>>();

    assert_eq!(actual.len(), 46);
    assert_eq!(&actual, expected);
}

#[tokio::test]
async fn bootstrap_creates_builtin_user_and_role_models_once() {
    let repository = MemoryProvisioningRepository::default();
    let service = SystemMetadataBootstrapService::new(repository.clone());

    let first = service
        .ensure_builtin_user_and_role_models(Uuid::now_v7())
        .await
        .unwrap();
    let second = service
        .ensure_builtin_user_and_role_models(Uuid::now_v7())
        .await
        .unwrap();

    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 2);
    assert_eq!(first[0].id, second[0].id);
    assert_eq!(first[1].id, second[1].id);

    let models = ModelDefinitionRepository::list_model_definitions(&repository, SYSTEM_SCOPE_ID)
        .await
        .unwrap();
    let users = models
        .iter()
        .find(|model| model.code == "users")
        .expect("users metadata model should exist");
    let roles = models
        .iter()
        .find(|model| model.code == "roles")
        .expect("roles metadata model should exist");

    assert_eq!(models.len(), 2);
    assert_eq!(users.title, "Users");
    assert_eq!(roles.title, "Roles");
    assert_eq!(users.scope_kind, DataModelScopeKind::System);
    assert_eq!(users.scope_id, SYSTEM_SCOPE_ID);
    assert_eq!(users.source_kind, DataModelSourceKind::MainSource);
    assert_eq!(
        users.protection.owner_kind,
        domain::DataModelOwnerKind::Core
    );
    assert!(users.protection.is_protected);
    assert_eq!(
        roles.protection.owner_kind,
        domain::DataModelOwnerKind::Core
    );
    assert!(roles.protection.is_protected);
    assert_eq!(users.physical_table_name, "users");
    assert_eq!(roles.physical_table_name, "roles");
    assert_eq!(users.fields.len(), 18);
    assert_eq!(roles.fields.len(), 16);
    assert!(users
        .fields
        .iter()
        .all(|field| field.is_system && !field.is_writable));
    assert!(roles
        .fields
        .iter()
        .all(|field| field.is_system && !field.is_writable));

    let grants = ModelDefinitionRepository::list_scope_data_model_grants(
        &repository,
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
    )
    .await
    .unwrap();
    assert_eq!(grants.len(), 2);
    assert!(grants.iter().all(|grant| {
        grant.permission_profile == ScopeDataModelPermissionProfile::SystemAll
            && (grant.data_model_id == users.id || grant.data_model_id == roles.id)
    }));
}

#[tokio::test]
async fn bootstrap_repairs_existing_partial_system_metadata_models() {
    let repository = MemoryProvisioningRepository::default();
    let actor_user_id = Uuid::now_v7();
    let partial_users = repository
        .create_model_definition(&CreateModelDefinitionInput {
            actor_user_id,
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_source_instance_id: None,
            source_kind: DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: DataModelStatus::Draft,
            protection: DataModelProtection::default(),
            code: "users".into(),
            title: "用户".into(),
        })
        .await
        .unwrap();
    repository
        .add_model_field(&AddModelFieldInput {
            actor_user_id,
            model_id: partial_users.id,
            physical_column_name: Some("wrong_account_column".into()),
            external_field_key: None,
            code: "account".into(),
            title: "Custom account title".into(),
            description: Some("User edited account description".into()),
            field_kind: ModelFieldKind::Text,
            is_system: true,
            is_writable: true,
            apply_physical_schema: false,
            is_required: false,
            api_required: false,
            is_unique: false,
            default_value: None,
            display_interface: Some("input".into()),
            display_options: serde_json::json!({ "width": 240 }),
            relation_target_model_id: None,
            relation_options: serde_json::json!({ "stale": true }),
        })
        .await
        .unwrap();
    repository
        .add_model_field(&AddModelFieldInput {
            actor_user_id,
            model_id: partial_users.id,
            physical_column_name: None,
            external_field_key: None,
            code: "custom_note".into(),
            title: "Custom note".into(),
            description: Some("User-added field".into()),
            field_kind: ModelFieldKind::Text,
            is_system: false,
            is_writable: true,
            apply_physical_schema: false,
            is_required: false,
            api_required: false,
            is_unique: false,
            default_value: None,
            display_interface: Some("textarea".into()),
            display_options: serde_json::json!({ "rows": 4 }),
            relation_target_model_id: None,
            relation_options: serde_json::json!({}),
        })
        .await
        .unwrap();

    SystemMetadataBootstrapService::new(repository.clone())
        .ensure_builtin_user_and_role_models(actor_user_id)
        .await
        .unwrap();

    let models = ModelDefinitionRepository::list_model_definitions(&repository, SYSTEM_SCOPE_ID)
        .await
        .unwrap();
    let repaired_users = models
        .iter()
        .find(|model| model.id == partial_users.id)
        .expect("partial users model should be repaired in place");
    let user_field_codes = repaired_users
        .fields
        .iter()
        .map(|field| field.code.as_str())
        .collect::<Vec<_>>();

    assert_eq!(user_field_codes.len(), 19);
    assert!(user_field_codes.contains(&"id"));
    assert!(user_field_codes.contains(&"account"));
    assert!(user_field_codes.contains(&"created_at"));
    assert!(user_field_codes.contains(&"custom_note"));
    assert_eq!(repaired_users.physical_table_name, "users");
    assert_eq!(repaired_users.title, "用户");
    assert_eq!(
        repaired_users.protection.owner_kind,
        domain::DataModelOwnerKind::Core
    );
    assert!(repaired_users.protection.is_protected);
    assert_eq!(repaired_users.status, DataModelStatus::Published);

    let account_field = repaired_users
        .fields
        .iter()
        .find(|field| field.code == "account")
        .expect("account field should remain");
    assert_eq!(account_field.title, "Custom account title");
    assert_eq!(
        account_field.description.as_deref(),
        Some("User edited account description")
    );
    assert_eq!(account_field.display_interface.as_deref(), Some("input"));
    assert_eq!(
        account_field.display_options,
        serde_json::json!({ "width": 240 })
    );
    assert_eq!(account_field.physical_column_name, "account");
    assert_eq!(account_field.field_kind, ModelFieldKind::String);
    assert!(account_field.is_system);
    assert!(!account_field.is_writable);
    assert!(account_field.is_required);
    assert!(account_field.is_unique);
    assert_eq!(account_field.relation_options, serde_json::json!({}));

    let custom_note_field = repaired_users
        .fields
        .iter()
        .find(|field| field.code == "custom_note")
        .expect("user-added field should not be removed");
    assert!(!custom_note_field.is_system);
    assert_eq!(custom_note_field.title, "Custom note");

    let grants = ModelDefinitionRepository::list_scope_data_model_grants(
        &repository,
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
    )
    .await
    .unwrap();
    assert!(grants
        .iter()
        .any(|grant| grant.data_model_id == partial_users.id));
}

#[tokio::test]
async fn ac_012_013_system_metadata_projection_localizes_defaults_and_preserves_custom_titles() {
    let repository = MemoryProvisioningRepository::default();
    let mut users = SystemMetadataBootstrapService::new(repository)
        .ensure_builtin_user_and_role_models(Uuid::now_v7())
        .await
        .unwrap()
        .into_iter()
        .find(|model| model.code == "users")
        .expect("users metadata model should exist");
    users.title = "用户".into();
    users
        .fields
        .iter_mut()
        .find(|field| field.code == "email")
        .expect("email field")
        .title = "邮箱".into();
    users
        .fields
        .iter_mut()
        .find(|field| field.code == "account")
        .expect("account field")
        .title = "Administrator Account Label".into();

    let workspace_id = Uuid::now_v7();
    let zh_hans = domain::CatalogLocale::new("zh_Hans").unwrap();
    project_system_metadata_titles(
        &CatalogResolver::new(
            MetadataTranslationFixture {
                provide_zh_hans: true,
            },
            workspace_id,
        ),
        workspace_id,
        &zh_hans,
        &mut users,
    )
    .await
    .unwrap();

    assert_eq!(users.title, "zh:Users");
    assert_eq!(
        users
            .fields
            .iter()
            .find(|field| field.code == "email")
            .unwrap()
            .title,
        "zh:Email"
    );
    assert_eq!(
        users
            .fields
            .iter()
            .find(|field| field.code == "account")
            .unwrap()
            .title,
        "Administrator Account Label"
    );

    let roles = role_metadata_template();
    let mut role_record =
        SystemMetadataBootstrapService::new(MemoryProvisioningRepository::default())
            .ensure_builtin_user_and_role_models(Uuid::now_v7())
            .await
            .unwrap()
            .into_iter()
            .find(|model| model.code == roles.code)
            .unwrap();
    role_record.title = "角色".into();
    role_record
        .fields
        .iter_mut()
        .find(|field| field.code == "id")
        .expect("role id field")
        .title = "角色 ID".into();
    let en_us = domain::CatalogLocale::new("en_US").unwrap();
    project_system_metadata_titles(
        &CatalogResolver::new(
            MetadataTranslationFixture {
                provide_zh_hans: false,
            },
            workspace_id,
        ),
        workspace_id,
        &en_us,
        &mut role_record,
    )
    .await
    .unwrap();
    assert_eq!(role_record.title, "Roles");
    assert_eq!(
        role_record
            .fields
            .iter()
            .find(|field| field.code == "id")
            .unwrap()
            .title,
        "Role ID"
    );
}
