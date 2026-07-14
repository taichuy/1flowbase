use super::CoreConsoleLocaleText;

pub(super) const TEXTS: &[CoreConsoleLocaleText] = &[
    text!("auto.api_documentation", "API documentation", "API 文档"),
    text!(
        "auto.api_key_authentication",
        "API key authentication",
        "API Key 认证"
    ),
    text!("auto.system_runtime", "System runtime", "系统运行"),
    text!(
        "auto.application_management",
        "Application management",
        "应用管理"
    ),
    text!("auto.auth_center", "Authentication center", "认证中心"),
    text!("auto.data_source", "Data source", "数据源"),
    text!("auto.file_management", "File management", "文件管理"),
    text!("auto.infrastructure", "Infrastructure", "基础设施"),
    text!("auto.memory_observation", "Memory observation", "内存观测"),
    text!("auto.user_management", "User management", "用户管理"),
    text!("auto.model_providers", "Model providers", "模型提供商"),
    text!("auto.mcp_management", "MCP management", "MCP 管理"),
    text!(
        "auto.permission_management",
        "Permission management",
        "权限管理"
    ),
    text!(
        "console.policy_groups.settings.system.docs.description",
        "API documentation operations",
        "API 文档操作",
    ),
    text!(
        "console.policy_groups.settings.system.api-key-authentication.description",
        "API key authentication operations",
        "API Key 认证操作",
    ),
    text!(
        "console.policy_groups.settings.system.system-runtime.description",
        "System runtime operations",
        "系统运行操作",
    ),
    text!(
        "console.policy_groups.settings.system.applications.description",
        "Application management operations",
        "应用管理操作",
    ),
    text!(
        "console.policy_groups.settings.system.auth-center.description",
        "Authentication center operations",
        "认证中心操作",
    ),
    text!(
        "console.policy_groups.settings.system.data-models.description",
        "Data model and data source operations",
        "数据模型与数据源操作",
    ),
    text!(
        "console.policy_groups.settings.system.files.description",
        "File management operations",
        "文件管理操作",
    ),
    text!(
        "console.policy_groups.settings.system.host-infrastructure.description",
        "Host infrastructure operations",
        "主机基础设施操作",
    ),
    text!(
        "console.policy_groups.settings.system.memory-observation.description",
        "Memory observation operations",
        "内存观测操作",
    ),
    text!(
        "console.policy_groups.settings.system.members.description",
        "Member management operations",
        "成员管理操作",
    ),
    text!(
        "console.policy_groups.settings.system.model-providers.description",
        "Model provider operations",
        "模型提供商操作",
    ),
    text!(
        "console.policy_groups.settings.system.mcp-management.description",
        "MCP management operations",
        "MCP 管理操作",
    ),
    text!(
        "console.policy_groups.settings.system.roles.description",
        "Role and permission operations",
        "角色与权限操作",
    ),
    text!(
        "console.policy_groups.other.core.authenticated.label",
        "Signed-in console",
        "已登录后台",
    ),
    text!(
        "console.policy_groups.other.core.authenticated.description",
        "Console routes available to every signed-in user",
        "所有已登录用户可访问的后台路由",
    ),
    text!(
        "console.policy_groups.other.other.agent-flow.label",
        "Agent Flow",
        "智能体流程",
    ),
    text!(
        "console.policy_groups.other.other.agent-flow.description",
        "Registered Agent Flow operations outside system settings",
        "系统设置之外已注册的智能体流程操作",
    ),
    text!(
        "console.policy_groups.other.other.data-sources.label",
        "Data source utilities",
        "数据源工具",
    ),
    text!(
        "console.policy_groups.other.other.data-sources.description",
        "Registered data source operations outside system settings",
        "系统设置之外已注册的数据源操作",
    ),
    text!(
        "console.policy_groups.other.other.frontend-blocks.label",
        "Frontend blocks",
        "前端区块",
    ),
    text!(
        "console.policy_groups.other.other.frontend-blocks.description",
        "Registered frontend block catalog operations",
        "已注册的前端区块目录操作",
    ),
    text!(
        "console.policy_groups.other.other.js-dependencies.label",
        "JavaScript dependencies",
        "JavaScript 依赖",
    ),
    text!(
        "console.policy_groups.other.other.js-dependencies.description",
        "Registered JavaScript dependency operations",
        "已注册的 JavaScript 依赖操作",
    ),
    text!(
        "console.policy_groups.other.other.model-providers.label",
        "Model provider utilities",
        "模型提供商工具",
    ),
    text!(
        "console.policy_groups.other.other.model-providers.description",
        "Registered model provider operations outside system settings",
        "系统设置之外已注册的模型提供商操作",
    ),
    text!(
        "console.policy_groups.other.other.node-contributions.label",
        "Node contributions",
        "节点贡献",
    ),
    text!(
        "console.policy_groups.other.other.node-contributions.description",
        "Registered node contribution catalog operations",
        "已注册的节点贡献目录操作",
    ),
    text!(
        "console.policy_groups.other.other.plugins.label",
        "Plugins",
        "插件",
    ),
    text!(
        "console.policy_groups.other.other.plugins.description",
        "Registered plugin catalog and lifecycle operations",
        "已注册的插件目录与生命周期操作",
    ),
    text!(
        "console.policy_groups.other.other.workspace.label",
        "Current workspace",
        "当前工作区",
    ),
    text!(
        "console.policy_groups.other.other.workspace.description",
        "Registered operations for the current workspace",
        "当前工作区的已注册操作",
    ),
    text!(
        "console.policy.group_modes.disabled.label",
        "Disabled",
        "关闭"
    ),
    text!(
        "console.policy.group_modes.disabled.description",
        "Do not grant operations in this group",
        "不授予此组中的操作",
    ),
    text!(
        "console.policy.group_modes.full.label",
        "Full access",
        "完全开放"
    ),
    text!(
        "console.policy.group_modes.full.description",
        "Grant every operation in this group",
        "授予此组中的全部操作",
    ),
    text!(
        "console.policy.group_modes.custom.label",
        "Custom access",
        "自定义"
    ),
    text!(
        "console.policy.group_modes.custom.description",
        "Choose operations and row scopes individually",
        "逐项选择操作和行范围",
    ),
    text!(
        "console.policy.row_scopes.disabled.label",
        "Disabled",
        "关闭"
    ),
    text!(
        "console.policy.row_scopes.disabled.description",
        "Do not grant this operation",
        "不授予此操作",
    ),
    text!(
        "console.policy.row_scopes.own.label",
        "Own records",
        "仅自己"
    ),
    text!(
        "console.policy.row_scopes.own.description",
        "Allow records created by the current user",
        "允许当前用户创建的记录",
    ),
    text!(
        "console.policy.row_scopes.scope_all.label",
        "Current workspace",
        "当前空间",
    ),
    text!(
        "console.policy.row_scopes.scope_all.description",
        "Allow records in the current workspace",
        "允许当前工作区中的记录",
    ),
    text!(
        "console.resources.applications.label",
        "Applications",
        "应用"
    ),
    text!(
        "console.resources.applications.description",
        "Applications in the current workspace",
        "当前工作区中的应用",
    ),
    text!(
        "console.resources.applications.actions.create.label",
        "Create",
        "创建",
    ),
    text!(
        "console.resources.applications.actions.create.description",
        "Create an application",
        "创建应用",
    ),
    text!(
        "console.resources.applications.actions.view.label",
        "View",
        "查看"
    ),
    text!(
        "console.resources.applications.actions.view.description",
        "View an application",
        "查看应用",
    ),
    text!(
        "console.resources.applications.actions.update.label",
        "Update",
        "修改",
    ),
    text!(
        "console.resources.applications.actions.update.description",
        "Update an application",
        "修改应用",
    ),
    text!(
        "console.resources.applications.actions.delete.label",
        "Delete",
        "删除",
    ),
    text!(
        "console.resources.applications.actions.delete.description",
        "Delete an application",
        "删除应用",
    ),
    text!(
        "console.resources.data_source_instances.label",
        "Data source instances",
        "数据源实例",
    ),
    text!(
        "console.resources.data_source_instances.description",
        "Configured data source instances in the current workspace",
        "当前工作区中配置的数据源实例",
    ),
    text!(
        "console.resources.data_source_instances.actions.view.label",
        "View",
        "查看",
    ),
    text!(
        "console.resources.data_source_instances.actions.view.description",
        "View a data source instance",
        "查看数据源实例",
    ),
];
