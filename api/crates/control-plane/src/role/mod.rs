use std::collections::{BTreeMap, BTreeSet};

use access_control::{
    ensure_permission, ConsoleAuthorization, ConsoleOperationCompiledInventory,
    ConsoleOperationInventoryEntry, ConsolePolicyGroup as RegisteredConsolePolicyGroup,
    ResourceAccessRegistration, ResourceAccessScopeKind, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION,
};
use anyhow::Result;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        AuthRepository, CreateWorkspaceRoleInput, FrontstagePageRepository,
        ReplaceRoleDataPolicyInput, RoleDataModelPolicyInput, RoleDataPolicyDefaultsInput,
        RoleDataPolicyView, RoleRepository, UpdateWorkspaceRoleInput,
    },
};

pub mod console_policy_migration;
mod console_policy_validation;

use console_policy_validation::{
    complete_stored_console_policy, role_console_policy_groups_from_input,
    CompiledConsolePolicyOperationIndex, CompiledConsolePolicyOperationKind, ConsolePolicyGroupKey,
};
pub use console_policy_validation::{
    ConsolePolicyAuthorization, ConsolePolicyCatalog, ConsolePolicyCatalogAction,
    ConsolePolicyCatalogGroup, ConsolePolicyCatalogOperation, ConsolePolicyCatalogResource,
};

pub struct CreateRoleCommand {
    pub actor_user_id: Uuid,
    pub code: String,
    pub name: String,
    pub introduction: String,
    pub auto_grant_new_permissions: bool,
    pub is_default_member_role: bool,
}

pub struct UpdateRoleCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
    pub name: String,
    pub introduction: String,
    pub auto_grant_new_permissions: Option<bool>,
    pub is_default_member_role: Option<bool>,
}

pub struct DeleteRoleCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
}

pub struct ReplaceRolePermissionsCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
    pub permission_codes: Vec<String>,
}

pub struct ReplaceRoleDataPolicyCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
    pub default_policy: RoleDataPolicyDefaultsInput,
    pub model_policies: Vec<RoleDataModelPolicyInput>,
}

pub struct ReplaceRoleFrontstageRoutesCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
    pub page_ids: Vec<Uuid>,
    pub tab_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct ReplaceRoleConsolePolicyCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
    pub groups: Vec<ConsolePolicyGroupInput>,
}

#[derive(Debug, Clone)]
pub struct ConsolePolicyGroupInput {
    pub kind: String,
    pub group_id: String,
    pub mode: String,
    pub operations: Vec<ConsolePolicyOperationInput>,
}

#[derive(Debug, Clone)]
pub enum ConsolePolicyOperationInput {
    Simple { operation_id: String, enabled: bool },
    Row { operation_id: String, scope: String },
}

const CONSOLE_POLICY_TRANSLATIONS: &[(&str, &str, &str)] = &[
    ("auto.api_documentation", "API documentation", "API 文档"),
    (
        "auto.api_key_authentication",
        "API key authentication",
        "API Key 认证",
    ),
    ("auto.system_runtime", "System runtime", "系统运行"),
    (
        "auto.application_management",
        "Application management",
        "应用管理",
    ),
    ("auto.auth_center", "Authentication center", "认证中心"),
    ("auto.data_source", "Data source", "数据源"),
    ("auto.file_management", "File management", "文件管理"),
    ("auto.infrastructure", "Infrastructure", "基础设施"),
    ("auto.memory_observation", "Memory observation", "内存观测"),
    ("auto.user_management", "User management", "用户管理"),
    ("auto.model_providers", "Model providers", "模型提供商"),
    ("auto.mcp_management", "MCP management", "MCP 管理"),
    (
        "auto.permission_management",
        "Permission management",
        "权限管理",
    ),
    (
        "console.operations.applications.create.label",
        "Create application",
        "创建应用",
    ),
    (
        "console.operations.applications.create.description",
        "Create applications in the current workspace",
        "在当前工作区创建应用",
    ),
    (
        "console.operations.applications.view.label",
        "View applications",
        "查看应用",
    ),
    (
        "console.operations.applications.view.description",
        "Read applications within the permitted row scope",
        "按允许的行范围读取应用",
    ),
    (
        "console.operations.applications.update.label",
        "Update applications",
        "修改应用",
    ),
    (
        "console.operations.applications.update.description",
        "Update applications within the permitted row scope",
        "按允许的行范围修改应用",
    ),
    (
        "console.operations.applications.delete.label",
        "Delete applications",
        "删除应用",
    ),
    (
        "console.operations.applications.delete.description",
        "Delete applications within the permitted row scope",
        "按允许的行范围删除应用",
    ),
    (
        "console.operations.applications.publish.label",
        "Publish applications",
        "发布应用",
    ),
    (
        "console.operations.applications.publish.description",
        "Publish applications after domain validation",
        "通过领域校验后发布应用",
    ),
    (
        "console.operations.applications.api.set_enabled.label",
        "Enable application API",
        "启用应用 API",
    ),
    (
        "console.operations.applications.api.set_enabled.description",
        "Enable or disable the application API",
        "启用或停用应用 API",
    ),
    (
        "console.operations.applications.orchestration.template.export.label",
        "Export orchestration template",
        "导出编排模板",
    ),
    (
        "console.operations.applications.orchestration.template.export.description",
        "Export an application orchestration template",
        "导出应用编排模板",
    ),
    (
        "console.operations.applications.orchestration.template.import.label",
        "Import orchestration template",
        "导入编排模板",
    ),
    (
        "console.operations.applications.orchestration.template.import.description",
        "Import an application orchestration template",
        "导入应用编排模板",
    ),
    (
        "console.operations.applications.orchestration.version.restore.label",
        "Restore orchestration version",
        "恢复编排版本",
    ),
    (
        "console.operations.applications.orchestration.version.restore.description",
        "Restore an application orchestration version",
        "恢复应用编排版本",
    ),
    (
        "console.operations.applications.run.label",
        "Run applications",
        "运行应用",
    ),
    (
        "console.operations.applications.run.description",
        "Run an application after domain admission checks",
        "通过领域准入校验后运行应用",
    ),
    (
        "console.operations.applications.logs.export.label",
        "Export application logs",
        "导出应用日志",
    ),
    (
        "console.operations.applications.logs.export.description",
        "Export application runtime logs",
        "导出应用运行日志",
    ),
    (
        "console.operations.applications.logs.import.label",
        "Import application logs",
        "导入应用日志",
    ),
    (
        "console.operations.applications.logs.import.description",
        "Import application runtime logs",
        "导入应用运行日志",
    ),
    (
        "console.operations.data_sources.list.label",
        "List data sources",
        "查看数据源列表",
    ),
    (
        "console.operations.data_sources.list.description",
        "List data source definitions",
        "查看数据源定义列表",
    ),
    (
        "console.operations.data_sources.create.label",
        "Create data source",
        "创建数据源",
    ),
    (
        "console.operations.data_sources.create.description",
        "Create a data source definition",
        "创建数据源定义",
    ),
    (
        "console.operations.data_sources.defaults.update.label",
        "Update data source defaults",
        "修改数据源默认配置",
    ),
    (
        "console.operations.data_sources.defaults.update.description",
        "Update data source default configuration",
        "修改数据源默认配置",
    ),
    (
        "console.operations.data_sources.validate.label",
        "Validate data source",
        "校验数据源",
    ),
    (
        "console.operations.data_sources.validate.description",
        "Validate a data source connection",
        "校验数据源连接",
    ),
    (
        "console.operations.data_sources.discover.label",
        "Discover data source schema",
        "发现数据源结构",
    ),
    (
        "console.operations.data_sources.discover.description",
        "Discover the available data source schema",
        "发现可用的数据源结构",
    ),
    (
        "console.operations.data_sources.preview.label",
        "Preview data source",
        "预览数据源",
    ),
    (
        "console.operations.data_sources.preview.description",
        "Preview records from a data source",
        "预览数据源记录",
    ),
    (
        "console.operations.data_sources.map_to_model.label",
        "Map data source to model",
        "映射数据源到数据模型",
    ),
    (
        "console.operations.data_sources.map_to_model.description",
        "Map a data source schema to a data model",
        "将数据源结构映射到数据模型",
    ),
    (
        "console.operations.data_sources.view.label",
        "View data source instances",
        "查看数据源实例",
    ),
    (
        "console.operations.data_sources.view.description",
        "Read data source instances within the permitted row scope",
        "按允许的行范围读取数据源实例",
    ),
    (
        "console.operations.data_sources.secret.rotate.label",
        "Rotate data source secret",
        "轮换数据源密钥",
    ),
    (
        "console.operations.data_sources.secret.rotate.description",
        "Rotate a data source secret through the control plane",
        "通过控制面轮换数据源密钥",
    ),
    (
        "console.operations.model_definitions.list.label",
        "List data models",
        "查看数据模型列表",
    ),
    (
        "console.operations.model_definitions.list.description",
        "List data model definitions",
        "查看数据模型定义列表",
    ),
    (
        "console.operations.model_definitions.create.label",
        "Create data model",
        "创建数据模型",
    ),
    (
        "console.operations.model_definitions.create.description",
        "Create a data model definition",
        "创建数据模型定义",
    ),
    (
        "console.operations.model_definitions.update.label",
        "Update data model",
        "修改数据模型",
    ),
    (
        "console.operations.model_definitions.update.description",
        "Update a data model definition",
        "修改数据模型定义",
    ),
    (
        "console.operations.model_definitions.delete.label",
        "Delete data model",
        "删除数据模型",
    ),
    (
        "console.operations.model_definitions.delete.description",
        "Delete a data model definition",
        "删除数据模型定义",
    ),
    (
        "console.operations.model_definitions.advisor.view.label",
        "View model advisor",
        "查看模型顾问",
    ),
    (
        "console.operations.model_definitions.advisor.view.description",
        "View data model protection advice",
        "查看数据模型保护建议",
    ),
    (
        "console.operations.model_definitions.openapi.view.label",
        "View model OpenAPI",
        "查看数据模型 OpenAPI",
    ),
    (
        "console.operations.model_definitions.openapi.view.description",
        "View the data model OpenAPI contract",
        "查看数据模型 OpenAPI 契约",
    ),
    (
        "console.operations.model_fields.create.label",
        "Create model field",
        "创建模型字段",
    ),
    (
        "console.operations.model_fields.create.description",
        "Create a data model field",
        "创建数据模型字段",
    ),
    (
        "console.operations.model_fields.update.label",
        "Update model field",
        "修改模型字段",
    ),
    (
        "console.operations.model_fields.update.description",
        "Update a data model field",
        "修改数据模型字段",
    ),
    (
        "console.operations.model_fields.delete.label",
        "Delete model field",
        "删除模型字段",
    ),
    (
        "console.operations.model_fields.delete.description",
        "Delete a data model field",
        "删除数据模型字段",
    ),
    (
        "console.operations.model_scope_grants.list.label",
        "List model scope grants",
        "查看模型范围授权列表",
    ),
    (
        "console.operations.model_scope_grants.list.description",
        "List data model scope grants",
        "查看数据模型范围授权列表",
    ),
    (
        "console.operations.model_scope_grants.create.label",
        "Create model scope grant",
        "创建模型范围授权",
    ),
    (
        "console.operations.model_scope_grants.create.description",
        "Create a data model scope grant",
        "创建数据模型范围授权",
    ),
    (
        "console.operations.model_scope_grants.update.label",
        "Update model scope grant",
        "修改模型范围授权",
    ),
    (
        "console.operations.model_scope_grants.update.description",
        "Update a data model scope grant",
        "修改数据模型范围授权",
    ),
    (
        "console.operations.file_storages.list.label",
        "List file storages",
        "查看文件存储列表",
    ),
    (
        "console.operations.file_storages.list.description",
        "List configured file storages",
        "查看已配置的文件存储列表",
    ),
    (
        "console.operations.file_storages.create.label",
        "Create file storage",
        "创建文件存储",
    ),
    (
        "console.operations.file_storages.create.description",
        "Create a file storage configuration",
        "创建文件存储配置",
    ),
    (
        "console.operations.file_storages.update.label",
        "Update file storage",
        "修改文件存储",
    ),
    (
        "console.operations.file_storages.update.description",
        "Update a file storage configuration",
        "修改文件存储配置",
    ),
    (
        "console.operations.file_storages.delete.label",
        "Delete file storage",
        "删除文件存储",
    ),
    (
        "console.operations.file_storages.delete.description",
        "Delete a file storage configuration",
        "删除文件存储配置",
    ),
    (
        "console.operations.file_tables.list.label",
        "List file tables",
        "查看文件表列表",
    ),
    (
        "console.operations.file_tables.list.description",
        "List file tables",
        "查看文件表列表",
    ),
    (
        "console.operations.file_tables.create.label",
        "Create file table",
        "创建文件表",
    ),
    (
        "console.operations.file_tables.create.description",
        "Create a file table",
        "创建文件表",
    ),
    (
        "console.operations.file_tables.storage.bind.label",
        "Bind file table storage",
        "绑定文件表存储",
    ),
    (
        "console.operations.file_tables.storage.bind.description",
        "Bind a file table to file storage",
        "将文件表绑定到文件存储",
    ),
    (
        "console.operations.file_tables.delete.label",
        "Delete file table",
        "删除文件表",
    ),
    (
        "console.operations.file_tables.delete.description",
        "Delete a file table",
        "删除文件表",
    ),
    (
        "console.operations.files.upload.label",
        "Upload files",
        "上传文件",
    ),
    (
        "console.operations.files.upload.description",
        "Upload files to the current workspace",
        "向当前工作区上传文件",
    ),
    (
        "console.operations.files.content.download.label",
        "Download file content",
        "下载文件内容",
    ),
    (
        "console.operations.files.content.download.description",
        "Download file content within the permitted scope",
        "在允许的范围内下载文件内容",
    ),
    (
        "console.resources.applications.label",
        "Applications",
        "应用",
    ),
    (
        "console.resources.applications.description",
        "Application records in the current workspace",
        "当前工作区中的应用记录",
    ),
    (
        "console.resources.applications.actions.create.label",
        "Create",
        "创建",
    ),
    (
        "console.resources.applications.actions.create.description",
        "Create an application",
        "创建应用",
    ),
    (
        "console.resources.applications.actions.view.label",
        "View",
        "查看",
    ),
    (
        "console.resources.applications.actions.view.description",
        "View an application",
        "查看应用",
    ),
    (
        "console.resources.applications.actions.update.label",
        "Update",
        "修改",
    ),
    (
        "console.resources.applications.actions.update.description",
        "Update an application",
        "修改应用",
    ),
    (
        "console.resources.applications.actions.delete.label",
        "Delete",
        "删除",
    ),
    (
        "console.resources.applications.actions.delete.description",
        "Delete an application",
        "删除应用",
    ),
    (
        "console.resources.data_source_instances.label",
        "Data source instances",
        "数据源实例",
    ),
    (
        "console.resources.data_source_instances.description",
        "Configured data source instances in the current workspace",
        "当前工作区中配置的数据源实例",
    ),
    (
        "console.resources.data_source_instances.actions.view.label",
        "View",
        "查看",
    ),
    (
        "console.resources.data_source_instances.actions.view.description",
        "View a data source instance",
        "查看数据源实例",
    ),
];

fn localized_reference(reference: &str, locale: &str) -> Result<String, ControlPlaneError> {
    let value = CONSOLE_POLICY_TRANSLATIONS
        .iter()
        .find(|(candidate, _, _)| *candidate == reference)
        .map(|(_, english, simplified_chinese)| match locale {
            "en_US" => *english,
            "zh_Hans" => *simplified_chinese,
            _ => "",
        })
        .filter(|value| !value.is_empty())
        .ok_or(ControlPlaneError::InvalidInput(
            "console_policy_translation",
        ))?;
    Ok(value.to_string())
}

fn localized_pair(locale: &str, english: &str, simplified_chinese: &str) -> String {
    match locale {
        "zh_Hans" => simplified_chinese.to_string(),
        _ => english.to_string(),
    }
}

fn domain_console_policy_group(
    registered: &RegisteredConsolePolicyGroup,
) -> Result<domain::ConsolePolicyGroup, ControlPlaneError> {
    match registered {
        RegisteredConsolePolicyGroup::SettingsFeature(group_id) => {
            domain::ConsolePolicyGroup::settings_feature(group_id)
                .map_err(|_| ControlPlaneError::InvalidInput("console_policy_group"))
        }
        RegisteredConsolePolicyGroup::Other(group_id) => {
            domain::ConsolePolicyGroup::other(group_id)
                .map_err(|_| ControlPlaneError::InvalidInput("console_policy_group"))
        }
    }
}

fn console_policy_group_key(group: &domain::ConsolePolicyGroup) -> ConsolePolicyGroupKey {
    (
        group.kind().as_str().to_string(),
        group.group_id().as_str().to_string(),
    )
}

fn console_policy_group_text(
    group: &domain::ConsolePolicyGroup,
    locale: &str,
) -> Result<(String, String), ControlPlaneError> {
    let (label_ref, english_description, simplified_chinese_description) =
        match (group.kind().as_str(), group.group_id().as_str()) {
            ("settings_feature", "system.docs") => (
                "auto.api_documentation",
                "API documentation operations",
                "API 文档操作",
            ),
            ("settings_feature", "system.api-key-authentication") => (
                "auto.api_key_authentication",
                "API key authentication operations",
                "API Key 认证操作",
            ),
            ("settings_feature", "system.system-runtime") => (
                "auto.system_runtime",
                "System runtime operations",
                "系统运行操作",
            ),
            ("settings_feature", "system.applications") => (
                "auto.application_management",
                "Application management operations",
                "应用管理操作",
            ),
            ("settings_feature", "system.auth-center") => (
                "auto.auth_center",
                "Authentication center operations",
                "认证中心操作",
            ),
            ("settings_feature", "system.data-models") => (
                "auto.data_source",
                "Data model and data source operations",
                "数据模型与数据源操作",
            ),
            ("settings_feature", "system.files") => (
                "auto.file_management",
                "File management operations",
                "文件管理操作",
            ),
            ("settings_feature", "system.host-infrastructure") => (
                "auto.infrastructure",
                "Host infrastructure operations",
                "主机基础设施操作",
            ),
            ("settings_feature", "system.memory-observation") => (
                "auto.memory_observation",
                "Memory observation operations",
                "内存观测操作",
            ),
            ("settings_feature", "system.members") => (
                "auto.user_management",
                "Member management operations",
                "成员管理操作",
            ),
            ("settings_feature", "system.model-providers") => (
                "auto.model_providers",
                "Model provider operations",
                "模型提供商操作",
            ),
            ("settings_feature", "system.mcp-management") => (
                "auto.mcp_management",
                "MCP management operations",
                "MCP 管理操作",
            ),
            ("settings_feature", "system.roles") => (
                "auto.permission_management",
                "Role and permission operations",
                "角色与权限操作",
            ),
            ("other", "other.data-sources") => (
                "auto.data_source",
                "Other data source operations",
                "其他数据源操作",
            ),
            ("other", "other.files") => (
                "auto.file_management",
                "Other file operations",
                "其他文件操作",
            ),
            _ => {
                return Err(ControlPlaneError::InvalidInput(
                    "console_policy_group_translation",
                ))
            }
        };
    Ok((
        localized_reference(label_ref, locale)?,
        localized_pair(locale, english_description, simplified_chinese_description),
    ))
}

fn compiled_console_policy_operations(
    inventory: &ConsoleOperationCompiledInventory,
) -> Result<CompiledConsolePolicyOperationIndex, ControlPlaneError> {
    let mut resources = BTreeMap::<String, &ResourceAccessRegistration>::new();
    for resource in &inventory.resources {
        if resources
            .insert(resource.resource_code.clone(), resource)
            .is_some()
        {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_resource_duplicate",
            ));
        }
        let mut actions = BTreeSet::new();
        for action in &resource.actions {
            if !actions.insert(action.action_code.as_str()) {
                return Err(ControlPlaneError::InvalidInput(
                    "console_policy_action_duplicate",
                ));
            }
        }
    }

    let mut groups = BTreeMap::new();
    for operation in &inventory.operations {
        let group = domain_console_policy_group(&operation.policy_group)?;
        let group_key = console_policy_group_key(&group);
        let Some(operation_kind) = (match &operation.authorization {
            ConsoleAuthorization::Authenticated => None,
            ConsoleAuthorization::Simple => Some(CompiledConsolePolicyOperationKind::Simple),
            ConsoleAuthorization::ResourceAction {
                resource_code,
                action_code,
            } => {
                let resource = resources
                    .get(resource_code)
                    .ok_or(ControlPlaneError::InvalidInput("console_policy_resource"))?;
                if resource.scope_kind != ResourceAccessScopeKind::Workspace
                    || resource.scope_field.as_deref() != Some("scope_id")
                    || resource.owner_field.as_deref() != Some("created_by")
                {
                    return Err(ControlPlaneError::InvalidInput(
                        "console_policy_resource_scope",
                    ));
                }
                if !resource
                    .actions
                    .iter()
                    .any(|action| action.action_code == *action_code)
                {
                    return Err(ControlPlaneError::InvalidInput("console_policy_action"));
                }
                Some(CompiledConsolePolicyOperationKind::Row)
            }
        }) else {
            continue;
        };
        if domain::ConsoleOperationId::try_from(operation.operation_id.as_str()).is_err() {
            return Err(ControlPlaneError::InvalidInput("console_policy_operation"));
        }
        let operations = groups.entry(group_key).or_insert_with(BTreeMap::new);
        if operations
            .insert(operation.operation_id.clone(), operation_kind)
            .is_some()
        {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_operation_duplicate",
            ));
        }
    }
    Ok(groups)
}

fn operation_text(
    operation: &ConsoleOperationInventoryEntry,
    group: &domain::ConsolePolicyGroup,
    locale: &str,
) -> Result<(String, String), ControlPlaneError> {
    let label = localized_reference(&operation.label_ref, locale)?;
    let description = operation
        .description_ref
        .as_deref()
        .map(|reference| localized_reference(reference, locale))
        .transpose()?
        .or_else(|| {
            console_policy_group_text(group, locale)
                .ok()
                .map(|(_, description)| description)
        })
        .ok_or(ControlPlaneError::InvalidInput(
            "console_policy_description",
        ))?;
    Ok((label, description))
}

fn build_console_policy_catalog_for_locale(
    inventory: &ConsoleOperationCompiledInventory,
    locale: &str,
) -> Result<ConsolePolicyCatalog, ControlPlaneError> {
    let operation_index = compiled_console_policy_operations(inventory)?;
    let mut groups = Vec::with_capacity(operation_index.len());
    for ((kind, group_id), operations) in operation_index {
        let group = if kind == "settings_feature" {
            domain::ConsolePolicyGroup::settings_feature(&group_id)
        } else {
            domain::ConsolePolicyGroup::other(&group_id)
        }
        .map_err(|_| ControlPlaneError::InvalidInput("console_policy_group"))?;
        let (label, description) = console_policy_group_text(&group, locale)?;
        let mut operation_views = inventory
            .operations
            .iter()
            .filter(|operation| {
                domain_console_policy_group(&operation.policy_group)
                    .map(|candidate| {
                        console_policy_group_key(&candidate) == (kind.clone(), group_id.clone())
                    })
                    .unwrap_or(false)
                    && !matches!(operation.authorization, ConsoleAuthorization::Authenticated)
            })
            .map(|operation| {
                let (label, description) = operation_text(operation, &group, locale)?;
                let operation_kind = operations
                    .get(&operation.operation_id)
                    .ok_or(ControlPlaneError::InvalidInput("console_policy_operation"))?;
                let authorization = match &operation.authorization {
                    ConsoleAuthorization::Simple => ConsolePolicyAuthorization::Simple,
                    ConsoleAuthorization::ResourceAction {
                        resource_code,
                        action_code,
                    } => ConsolePolicyAuthorization::ResourceAction {
                        resource_code: resource_code.clone(),
                        action_code: action_code.clone(),
                    },
                    ConsoleAuthorization::Authenticated => {
                        return Err(ControlPlaneError::InvalidInput("console_policy_type"));
                    }
                };
                let allowed_row_scopes = match operation_kind {
                    CompiledConsolePolicyOperationKind::Simple => Vec::new(),
                    CompiledConsolePolicyOperationKind::Row => vec![
                        domain::ConsoleOperationRowScope::Disabled,
                        domain::ConsoleOperationRowScope::Own,
                        domain::ConsoleOperationRowScope::ScopeAll,
                    ],
                };
                Ok((
                    operation.order,
                    operation.operation_id.clone(),
                    ConsolePolicyCatalogOperation {
                        operation_id: operation.operation_id.clone(),
                        label,
                        description,
                        order: operation.order,
                        allowed_row_scopes,
                        authorization,
                    },
                ))
            })
            .collect::<Result<Vec<_>, ControlPlaneError>>()?;
        operation_views.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
        groups.push(ConsolePolicyCatalogGroup {
            kind: group.kind(),
            group_id,
            label,
            description,
            operations: operation_views
                .into_iter()
                .map(|(_, _, operation)| operation)
                .collect(),
        });
    }
    groups.sort_by(|left, right| {
        let left_kind_order = (left.kind == domain::ConsolePolicyGroupKind::Other) as u8;
        let right_kind_order = (right.kind == domain::ConsolePolicyGroupKind::Other) as u8;
        left_kind_order
            .cmp(&right_kind_order)
            .then(left.group_id.cmp(&right.group_id))
    });

    let resources = inventory
        .resources
        .iter()
        .map(|resource| {
            let label = localized_reference(&resource.label_ref, locale)?;
            let description = resource
                .description_ref
                .as_deref()
                .map(|reference| localized_reference(reference, locale))
                .transpose()?
                .ok_or(ControlPlaneError::InvalidInput(
                    "console_policy_description",
                ))?;
            let mut actions = resource
                .actions
                .iter()
                .map(|action| {
                    let label = localized_reference(&action.label_ref, locale)?;
                    let description = action
                        .description_ref
                        .as_deref()
                        .map(|reference| localized_reference(reference, locale))
                        .transpose()?
                        .ok_or(ControlPlaneError::InvalidInput(
                            "console_policy_description",
                        ))?;
                    Ok(ConsolePolicyCatalogAction {
                        action_code: action.action_code.clone(),
                        label,
                        description,
                    })
                })
                .collect::<Result<Vec<_>, ControlPlaneError>>()?;
            actions.sort_by(|left, right| left.action_code.cmp(&right.action_code));
            Ok(ConsolePolicyCatalogResource {
                resource_code: resource.resource_code.clone(),
                label,
                description,
                actions,
            })
        })
        .collect::<Result<Vec<_>, ControlPlaneError>>()?;

    Ok(ConsolePolicyCatalog {
        schema_version: inventory.schema_version.to_string(),
        locale: locale.to_string(),
        groups,
        resources,
    })
}

fn validate_complete_console_policy_catalog(
    inventory: &ConsoleOperationCompiledInventory,
) -> Result<CompiledConsolePolicyOperationIndex, ControlPlaneError> {
    let operation_index = compiled_console_policy_operations(inventory)?;
    build_console_policy_catalog_for_locale(inventory, "en_US")?;
    build_console_policy_catalog_for_locale(inventory, "zh_Hans")?;
    Ok(operation_index)
}

pub struct RoleFrontstageRoutesView {
    pub pages: Vec<domain::FrontstagePageRecord>,
    pub tabs: Vec<domain::frontstage::FrontstagePageTabRecord>,
    pub rules: Vec<domain::frontstage::FrontstagePageVisibilityRuleRecord>,
}

pub struct RoleService<R> {
    repository: R,
}

fn ensure_workspace_role_data_policy_scope(
    scope: domain::RoleDataPolicyScope,
) -> Result<(), ControlPlaneError> {
    if scope == domain::RoleDataPolicyScope::SystemAll {
        return Err(ControlPlaneError::InvalidInput(
            "system_all_requires_system_role",
        ));
    }

    Ok(())
}

fn ensure_workspace_role_data_policy_allowed(
    default_policy: &RoleDataPolicyDefaultsInput,
    model_policies: &[RoleDataModelPolicyInput],
) -> Result<(), ControlPlaneError> {
    ensure_workspace_role_data_policy_scope(default_policy.default_view_scope)?;
    ensure_workspace_role_data_policy_scope(default_policy.default_update_scope)?;
    ensure_workspace_role_data_policy_scope(default_policy.default_delete_scope)?;

    for policy in model_policies {
        for scope in [
            policy.view_scope_override,
            policy.update_scope_override,
            policy.delete_scope_override,
        ]
        .into_iter()
        .flatten()
        {
            ensure_workspace_role_data_policy_scope(scope)?;
        }
    }

    Ok(())
}

impl<R> RoleService<R>
where
    R: RoleRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn get_console_policy_catalog(
        &self,
        actor_user_id: Uuid,
        inventory: &ConsoleOperationCompiledInventory,
        locale: &str,
    ) -> Result<ConsolePolicyCatalog> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        validate_complete_console_policy_catalog(inventory)?;
        build_console_policy_catalog_for_locale(inventory, locale).map_err(Into::into)
    }

    pub async fn get_console_policy(
        &self,
        actor_user_id: Uuid,
        role_code: &str,
        inventory: &ConsoleOperationCompiledInventory,
    ) -> Result<domain::RoleConsolePolicy> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        let operation_index = validate_complete_console_policy_catalog(inventory)?;
        let policy = self
            .repository
            .get_role_console_policy(actor.current_workspace_id, role_code)
            .await?;
        complete_stored_console_policy(policy, &operation_index).map_err(Into::into)
    }

    pub async fn replace_console_policy(
        &self,
        command: ReplaceRoleConsolePolicyCommand,
        inventory: &ConsoleOperationCompiledInventory,
    ) -> Result<domain::RoleConsolePolicy> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;

        if self
            .repository
            .list_roles(actor.current_workspace_id)
            .await?
            .into_iter()
            .find(|role| role.code == command.role_code)
            .is_some_and(|role| role.is_builtin || !role.is_editable)
        {
            return Err(ControlPlaneError::PermissionDenied("builtin_role_immutable").into());
        }

        let operation_index = validate_complete_console_policy_catalog(inventory)?;
        let groups = role_console_policy_groups_from_input(&command.groups, &operation_index)?;
        let role_code = command.role_code.clone();
        let policy = self
            .repository
            .replace_role_console_policy(&crate::ports::ReplaceRoleConsolePolicyInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                role_code: role_code.clone(),
                groups,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.console_policy_replaced",
                serde_json::json!({
                    "code": role_code,
                    "schema_version": inventory.schema_version,
                }),
            ))
            .await?;
        Ok(policy)
    }

    pub async fn list_roles(&self, actor_user_id: Uuid) -> Result<Vec<domain::RoleTemplate>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository.list_roles(actor.current_workspace_id).await
    }

    pub async fn get_role_permissions(
        &self,
        actor_user_id: Uuid,
        role_code: &str,
    ) -> Result<Vec<String>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .list_role_permissions(actor.current_workspace_id, role_code)
            .await
    }

    pub async fn get_role_data_policy(
        &self,
        actor_user_id: Uuid,
        role_code: &str,
    ) -> Result<RoleDataPolicyView> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .get_role_data_policy(actor.current_workspace_id, role_code)
            .await
    }

    pub async fn create_role(&self, command: CreateRoleCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .create_team_role(&CreateWorkspaceRoleInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                code: command.code.clone(),
                name: command.name.clone(),
                introduction: command.introduction.clone(),
                auto_grant_new_permissions: command.auto_grant_new_permissions,
                is_default_member_role: command.is_default_member_role,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.created",
                serde_json::json!({ "code": command.code }),
            ))
            .await?;
        Ok(())
    }

    pub async fn update_role(&self, command: UpdateRoleCommand) -> Result<()> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .update_team_role(&UpdateWorkspaceRoleInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                role_code: command.role_code.clone(),
                name: command.name.clone(),
                introduction: command.introduction.clone(),
                auto_grant_new_permissions: command.auto_grant_new_permissions,
                is_default_member_role: command.is_default_member_role,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.updated",
                serde_json::json!({ "code": command.role_code }),
            ))
            .await?;
        Ok(())
    }

    pub async fn delete_role(&self, command: DeleteRoleCommand) -> Result<()> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .delete_team_role(
                command.actor_user_id,
                actor.current_workspace_id,
                &command.role_code,
            )
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.deleted",
                serde_json::json!({ "code": command.role_code }),
            ))
            .await?;
        Ok(())
    }

    pub async fn replace_permissions(&self, command: ReplaceRolePermissionsCommand) -> Result<()> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;

        self.repository
            .replace_role_permissions(
                command.actor_user_id,
                actor.current_workspace_id,
                &command.role_code,
                &command.permission_codes,
            )
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.permissions_replaced",
                serde_json::json!({
                    "code": command.role_code,
                    "permission_codes": command.permission_codes,
                }),
            ))
            .await?;
        Ok(())
    }

    pub async fn replace_data_policy(
        &self,
        command: ReplaceRoleDataPolicyCommand,
    ) -> Result<RoleDataPolicyView> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        ensure_workspace_role_data_policy_allowed(
            &command.default_policy,
            &command.model_policies,
        )?;

        let policy = self
            .repository
            .replace_role_data_policy(&ReplaceRoleDataPolicyInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                role_code: command.role_code.clone(),
                default_policy: command.default_policy,
                model_policies: command.model_policies,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.data_policy_replaced",
                serde_json::json!({
                    "code": command.role_code,
                }),
            ))
            .await?;
        Ok(policy)
    }
}

impl<R> RoleService<R>
where
    R: RoleRepository + AuthRepository,
{
    pub async fn list_permission_options(
        &self,
        actor_user_id: Uuid,
    ) -> Result<Vec<domain::PermissionDefinition>> {
        let actor =
            RoleRepository::load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        AuthRepository::list_permissions(&self.repository).await
    }
}

impl<R> RoleService<R>
where
    R: RoleRepository + FrontstagePageRepository,
{
    pub async fn get_frontstage_routes(
        &self,
        actor_user_id: Uuid,
        role_code: &str,
    ) -> Result<RoleFrontstageRoutesView> {
        let actor =
            RoleRepository::load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        RoleRepository::list_role_permissions(
            &self.repository,
            actor.current_workspace_id,
            role_code,
        )
        .await?;

        let pages = FrontstagePageRepository::list_frontstage_pages(
            &self.repository,
            actor.current_workspace_id,
        )
        .await?;
        let mut tabs = Vec::new();
        for page in pages
            .iter()
            .filter(|page| page.kind == domain::FrontstagePageKind::Page)
        {
            tabs.extend(
                FrontstagePageRepository::list_frontstage_page_tabs(
                    &self.repository,
                    actor.current_workspace_id,
                    page.id,
                )
                .await?,
            );
        }
        let rules = FrontstagePageRepository::list_frontstage_page_visibility_rules_for_role(
            &self.repository,
            actor.current_workspace_id,
            role_code,
        )
        .await?;

        Ok(RoleFrontstageRoutesView { pages, tabs, rules })
    }

    pub async fn replace_frontstage_routes(
        &self,
        command: ReplaceRoleFrontstageRoutesCommand,
    ) -> Result<()> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor =
            RoleRepository::load_actor_context_for_user(&self.repository, command.actor_user_id)
                .await?;
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        RoleRepository::list_role_permissions(
            &self.repository,
            actor.current_workspace_id,
            &command.role_code,
        )
        .await?;

        FrontstagePageRepository::replace_frontstage_page_visibility_rules_for_role(
            &self.repository,
            actor.current_workspace_id,
            &command.role_code,
            &command.page_ids,
            &command.tab_ids,
            command.actor_user_id,
        )
        .await?;
        RoleRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.frontstage_routes_replaced",
                serde_json::json!({
                    "code": command.role_code,
                    "page_ids": command.page_ids,
                    "tab_ids": command.tab_ids,
                }),
            ),
        )
        .await?;
        Ok(())
    }
}
