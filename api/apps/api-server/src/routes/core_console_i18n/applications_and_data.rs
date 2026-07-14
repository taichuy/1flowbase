use super::CoreConsoleLocaleText;

pub(super) const TEXTS: &[CoreConsoleLocaleText] = &[
    text!(
        "console.operations.agent_flow.data_model_options.list.label",
        "View Agent Flow data model options",
        "查看智能体流程数据模型选项"
    ),
    text!(
        "console.operations.agent_flow.data_model_options.list.description",
        "Allow users to list data model options used by Agent Flow",
        "允许查看智能体流程使用的数据模型选项"
    ),
    text!(
        "console.operations.applications.api.set_enabled.label",
        "Enable application API",
        "启用应用 API"
    ),
    text!(
        "console.operations.applications.api.set_enabled.description",
        "Enable or disable the application API",
        "启用或停用应用 API"
    ),
    text!(
        "console.operations.applications.create.label",
        "Create application",
        "创建应用"
    ),
    text!(
        "console.operations.applications.create.description",
        "Create applications in the current workspace",
        "在当前工作区创建应用"
    ),
    text!(
        "console.operations.applications.delete.label",
        "Delete applications",
        "删除应用"
    ),
    text!(
        "console.operations.applications.delete.description",
        "Delete applications within the permitted row scope",
        "按允许的行范围删除应用"
    ),
    text!(
        "console.operations.applications.logs.export.label",
        "Export application logs",
        "导出应用日志"
    ),
    text!(
        "console.operations.applications.logs.export.description",
        "Export application runtime logs",
        "导出应用运行日志"
    ),
    text!(
        "console.operations.applications.logs.import.label",
        "Import application logs",
        "导入应用日志"
    ),
    text!(
        "console.operations.applications.logs.import.description",
        "Import application runtime logs",
        "导入应用运行日志"
    ),
    text!(
        "console.operations.applications.orchestration.template.export.label",
        "Export orchestration template",
        "导出编排模板"
    ),
    text!(
        "console.operations.applications.orchestration.template.export.description",
        "Export an application orchestration template",
        "导出应用编排模板"
    ),
    text!(
        "console.operations.applications.orchestration.template.import.label",
        "Import orchestration template",
        "导入编排模板"
    ),
    text!(
        "console.operations.applications.orchestration.template.import.description",
        "Import an application orchestration template",
        "导入应用编排模板"
    ),
    text!(
        "console.operations.applications.orchestration.version.restore.label",
        "Restore orchestration version",
        "恢复编排版本"
    ),
    text!(
        "console.operations.applications.orchestration.version.restore.description",
        "Restore an application orchestration version",
        "恢复应用编排版本"
    ),
    text!(
        "console.operations.applications.publish.label",
        "Publish applications",
        "发布应用"
    ),
    text!(
        "console.operations.applications.publish.description",
        "Publish applications after domain validation",
        "通过领域校验后发布应用"
    ),
    text!(
        "console.operations.applications.run.label",
        "Run applications",
        "运行应用"
    ),
    text!(
        "console.operations.applications.run.description",
        "Run an application after domain admission checks",
        "通过领域准入校验后运行应用"
    ),
    text!(
        "console.operations.applications.update.label",
        "Update applications",
        "修改应用"
    ),
    text!(
        "console.operations.applications.update.description",
        "Update applications within the permitted row scope",
        "按允许的行范围修改应用"
    ),
    text!(
        "console.operations.applications.view.label",
        "View applications",
        "查看应用"
    ),
    text!(
        "console.operations.applications.view.description",
        "Read applications within the permitted row scope",
        "按允许的行范围读取应用"
    ),
    text!(
        "console.operations.data_sources.create.label",
        "Create data source",
        "创建数据源"
    ),
    text!(
        "console.operations.data_sources.create.description",
        "Create a data source definition",
        "创建数据源定义"
    ),
    text!(
        "console.operations.data_sources.defaults.update.label",
        "Update data source defaults",
        "修改数据源默认配置"
    ),
    text!(
        "console.operations.data_sources.defaults.update.description",
        "Update data source default configuration",
        "修改数据源默认配置"
    ),
    text!(
        "console.operations.data_sources.discover.label",
        "Discover data source schema",
        "发现数据源结构"
    ),
    text!(
        "console.operations.data_sources.discover.description",
        "Discover the available data source schema",
        "发现可用的数据源结构"
    ),
    text!(
        "console.operations.data_sources.list.label",
        "List data sources",
        "查看数据源列表"
    ),
    text!(
        "console.operations.data_sources.list.description",
        "List data source definitions",
        "查看数据源定义列表"
    ),
    text!(
        "console.operations.data_sources.map_to_model.label",
        "Map data source to model",
        "映射数据源到数据模型"
    ),
    text!(
        "console.operations.data_sources.map_to_model.description",
        "Map a data source schema to a data model",
        "将数据源结构映射到数据模型"
    ),
    text!(
        "console.operations.data_sources.preview.label",
        "Preview data source",
        "预览数据源"
    ),
    text!(
        "console.operations.data_sources.preview.description",
        "Preview records from a data source",
        "预览数据源记录"
    ),
    text!(
        "console.operations.data_sources.secret.rotate.label",
        "Rotate data source secret",
        "轮换数据源密钥"
    ),
    text!(
        "console.operations.data_sources.secret.rotate.description",
        "Rotate a data source secret through the control plane",
        "通过控制面轮换数据源密钥"
    ),
    text!(
        "console.operations.data_sources.validate.label",
        "Validate data source",
        "校验数据源"
    ),
    text!(
        "console.operations.data_sources.validate.description",
        "Validate a data source connection",
        "校验数据源连接"
    ),
    text!(
        "console.operations.data_sources.view.label",
        "View data source instances",
        "查看数据源实例"
    ),
    text!(
        "console.operations.data_sources.view.description",
        "Read data source instances within the permitted row scope",
        "按允许的行范围读取数据源实例"
    ),
    text!(
        "console.operations.file_storages.create.label",
        "Create file storage",
        "创建文件存储"
    ),
    text!(
        "console.operations.file_storages.create.description",
        "Create a file storage configuration",
        "创建文件存储配置"
    ),
    text!(
        "console.operations.file_storages.delete.label",
        "Delete file storage",
        "删除文件存储"
    ),
    text!(
        "console.operations.file_storages.delete.description",
        "Delete a file storage configuration",
        "删除文件存储配置"
    ),
    text!(
        "console.operations.file_storages.list.label",
        "List file storages",
        "查看文件存储列表"
    ),
    text!(
        "console.operations.file_storages.list.description",
        "List configured file storages",
        "查看已配置的文件存储列表"
    ),
    text!(
        "console.operations.file_storages.update.label",
        "Update file storage",
        "修改文件存储"
    ),
    text!(
        "console.operations.file_storages.update.description",
        "Update a file storage configuration",
        "修改文件存储配置"
    ),
    text!(
        "console.operations.file_tables.create.label",
        "Create file table",
        "创建文件表"
    ),
    text!(
        "console.operations.file_tables.create.description",
        "Create a file table",
        "创建文件表"
    ),
    text!(
        "console.operations.file_tables.delete.label",
        "Delete file table",
        "删除文件表"
    ),
    text!(
        "console.operations.file_tables.delete.description",
        "Delete a file table",
        "删除文件表"
    ),
    text!(
        "console.operations.file_tables.list.label",
        "List file tables",
        "查看文件表列表"
    ),
    text!(
        "console.operations.file_tables.list.description",
        "List file tables",
        "查看文件表列表"
    ),
    text!(
        "console.operations.file_tables.storage.bind.label",
        "Bind file table storage",
        "绑定文件表存储"
    ),
    text!(
        "console.operations.file_tables.storage.bind.description",
        "Bind a file table to file storage",
        "将文件表绑定到文件存储"
    ),
    text!(
        "console.operations.model_definitions.advisor.view.label",
        "View model advisor",
        "查看模型顾问"
    ),
    text!(
        "console.operations.model_definitions.advisor.view.description",
        "View data model protection advice",
        "查看数据模型保护建议"
    ),
    text!(
        "console.operations.model_definitions.create.label",
        "Create data model",
        "创建数据模型"
    ),
    text!(
        "console.operations.model_definitions.create.description",
        "Create a data model definition",
        "创建数据模型定义"
    ),
    text!(
        "console.operations.model_definitions.delete.label",
        "Delete data model",
        "删除数据模型"
    ),
    text!(
        "console.operations.model_definitions.delete.description",
        "Delete a data model definition",
        "删除数据模型定义"
    ),
    text!(
        "console.operations.model_definitions.list.label",
        "List data models",
        "查看数据模型列表"
    ),
    text!(
        "console.operations.model_definitions.list.description",
        "List data model definitions",
        "查看数据模型定义列表"
    ),
    text!(
        "console.operations.model_definitions.openapi.view.label",
        "View model OpenAPI",
        "查看数据模型 OpenAPI"
    ),
    text!(
        "console.operations.model_definitions.openapi.view.description",
        "View the data model OpenAPI contract",
        "查看数据模型 OpenAPI 契约"
    ),
    text!(
        "console.operations.model_definitions.update.label",
        "Update data model",
        "修改数据模型"
    ),
    text!(
        "console.operations.model_definitions.update.description",
        "Update a data model definition",
        "修改数据模型定义"
    ),
    text!(
        "console.operations.model_fields.create.label",
        "Create model field",
        "创建模型字段"
    ),
    text!(
        "console.operations.model_fields.create.description",
        "Create a data model field",
        "创建数据模型字段"
    ),
    text!(
        "console.operations.model_fields.delete.label",
        "Delete model field",
        "删除模型字段"
    ),
    text!(
        "console.operations.model_fields.delete.description",
        "Delete a data model field",
        "删除数据模型字段"
    ),
    text!(
        "console.operations.model_fields.update.label",
        "Update model field",
        "修改模型字段"
    ),
    text!(
        "console.operations.model_fields.update.description",
        "Update a data model field",
        "修改数据模型字段"
    ),
    text!(
        "console.operations.model_scope_grants.create.label",
        "Create model scope grant",
        "创建模型范围授权"
    ),
    text!(
        "console.operations.model_scope_grants.create.description",
        "Create a data model scope grant",
        "创建数据模型范围授权"
    ),
    text!(
        "console.operations.model_scope_grants.list.label",
        "List model scope grants",
        "查看模型范围授权列表"
    ),
    text!(
        "console.operations.model_scope_grants.list.description",
        "List data model scope grants",
        "查看数据模型范围授权列表"
    ),
    text!(
        "console.operations.model_scope_grants.update.label",
        "Update model scope grant",
        "修改模型范围授权"
    ),
    text!(
        "console.operations.model_scope_grants.update.description",
        "Update a data model scope grant",
        "修改数据模型范围授权"
    ),
];
