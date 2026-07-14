use super::CoreConsoleLocaleText;

pub(super) const TEXTS: &[CoreConsoleLocaleText] = &[
    text!(
        "console.operations.auth_center.authenticators.copy.label",
        "Copy authenticators",
        "复制认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.copy.description",
        "Allow users to copy authenticators",
        "允许复制认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.create.label",
        "Create authenticators",
        "创建认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.create.description",
        "Allow users to create authenticators",
        "允许创建认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.delete.label",
        "Delete authenticators",
        "删除认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.delete.description",
        "Allow users to delete authenticators",
        "允许删除认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.enable.label",
        "Enable authenticators",
        "启用认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.enable.description",
        "Allow users to enable authenticators",
        "允许启用认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.order.label",
        "Reorder authenticators",
        "排序认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.order.description",
        "Allow users to reorder authenticators",
        "允许排序认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.update.label",
        "Update authenticators",
        "更新认证器"
    ),
    text!(
        "console.operations.auth_center.authenticators.update.description",
        "Allow users to update authenticators",
        "允许更新认证器"
    ),
    text!(
        "console.operations.auth_center.overview.view.label",
        "View authentication center overview",
        "查看认证中心概览"
    ),
    text!(
        "console.operations.auth_center.overview.view.description",
        "Allow users to view authentication center overview",
        "允许查看认证中心概览"
    ),
    text!(
        "console.operations.core_authenticated.label",
        "Use signed-in console",
        "使用已登录后台"
    ),
    text!(
        "console.operations.core_authenticated.description",
        "Access console routes available to signed-in users",
        "访问已登录用户可用的后台路由"
    ),
    text!(
        "console.operations.members.create.label",
        "Create members",
        "创建成员"
    ),
    text!(
        "console.operations.members.create.description",
        "Allow users to create members",
        "允许创建成员"
    ),
    text!(
        "console.operations.members.delete.label",
        "Delete members",
        "删除成员"
    ),
    text!(
        "console.operations.members.delete.description",
        "Allow users to delete members",
        "允许删除成员"
    ),
    text!(
        "console.operations.members.disable.label",
        "Disable members",
        "禁用成员"
    ),
    text!(
        "console.operations.members.disable.description",
        "Allow users to disable members",
        "允许禁用成员"
    ),
    text!(
        "console.operations.members.enable.label",
        "Enable members",
        "启用成员"
    ),
    text!(
        "console.operations.members.enable.description",
        "Allow users to enable members",
        "允许启用成员"
    ),
    text!(
        "console.operations.members.list.label",
        "List members",
        "查看成员"
    ),
    text!(
        "console.operations.members.list.description",
        "Allow users to list members",
        "允许查看成员"
    ),
    text!(
        "console.operations.members.password.reset.label",
        "Reset member passwords",
        "重置成员密码"
    ),
    text!(
        "console.operations.members.password.reset.description",
        "Allow users to reset member passwords",
        "允许重置成员密码"
    ),
    text!(
        "console.operations.members.role_options.list.label",
        "List member role options",
        "查看成员角色选项"
    ),
    text!(
        "console.operations.members.role_options.list.description",
        "Allow users to list member role options",
        "允许查看成员角色选项"
    ),
    text!(
        "console.operations.members.roles.replace.label",
        "Replace member roles",
        "替换成员角色"
    ),
    text!(
        "console.operations.members.roles.replace.description",
        "Allow users to replace member roles",
        "允许替换成员角色"
    ),
    text!(
        "console.operations.members.update.label",
        "Update members",
        "更新成员"
    ),
    text!(
        "console.operations.members.update.description",
        "Allow users to update members",
        "允许更新成员"
    ),
    text!(
        "console.operations.roles.console_policy.replace.label",
        "Replace role console policy",
        "替换角色后台策略"
    ),
    text!(
        "console.operations.roles.console_policy.replace.description",
        "Allow users to replace role console policy",
        "允许替换角色后台策略"
    ),
    text!(
        "console.operations.roles.console_policy.view.label",
        "View role console policy",
        "查看角色后台策略"
    ),
    text!(
        "console.operations.roles.console_policy.view.description",
        "Allow users to view role console policy",
        "允许查看角色后台策略"
    ),
    text!(
        "console.operations.roles.console_policy_catalog.view.label",
        "View console policy catalog",
        "查看后台策略目录"
    ),
    text!(
        "console.operations.roles.console_policy_catalog.view.description",
        "Allow users to view console policy catalog",
        "允许查看后台策略目录"
    ),
    text!(
        "console.operations.roles.create.label",
        "Create roles",
        "创建角色"
    ),
    text!(
        "console.operations.roles.create.description",
        "Allow users to create roles",
        "允许创建角色"
    ),
    text!(
        "console.operations.roles.data_model_options.list.label",
        "List role data model options",
        "查看角色数据模型选项"
    ),
    text!(
        "console.operations.roles.data_model_options.list.description",
        "Allow users to list role data model options",
        "允许查看角色数据模型选项"
    ),
    text!(
        "console.operations.roles.data_policy.replace.label",
        "Replace role data policy",
        "替换角色数据策略"
    ),
    text!(
        "console.operations.roles.data_policy.replace.description",
        "Allow users to replace role data policy",
        "允许替换角色数据策略"
    ),
    text!(
        "console.operations.roles.data_policy.view.label",
        "View role data policy",
        "查看角色数据策略"
    ),
    text!(
        "console.operations.roles.data_policy.view.description",
        "Allow users to view role data policy",
        "允许查看角色数据策略"
    ),
    text!(
        "console.operations.roles.delete.label",
        "Delete roles",
        "删除角色"
    ),
    text!(
        "console.operations.roles.delete.description",
        "Allow users to delete roles",
        "允许删除角色"
    ),
    text!(
        "console.operations.roles.frontstage_routes.replace.label",
        "Replace role frontstage routes",
        "替换角色前台路由"
    ),
    text!(
        "console.operations.roles.frontstage_routes.replace.description",
        "Allow users to replace role frontstage routes",
        "允许替换角色前台路由"
    ),
    text!(
        "console.operations.roles.frontstage_routes.view.label",
        "View role frontstage routes",
        "查看角色前台路由"
    ),
    text!(
        "console.operations.roles.frontstage_routes.view.description",
        "Allow users to view role frontstage routes",
        "允许查看角色前台路由"
    ),
    text!(
        "console.operations.roles.list.label",
        "List roles",
        "查看角色"
    ),
    text!(
        "console.operations.roles.list.description",
        "Allow users to list roles",
        "允许查看角色"
    ),
    text!(
        "console.operations.roles.permission_options.list.label",
        "List role permission options",
        "查看角色权限选项"
    ),
    text!(
        "console.operations.roles.permission_options.list.description",
        "Allow users to list role permission options",
        "允许查看角色权限选项"
    ),
    text!(
        "console.operations.roles.permissions.replace.label",
        "Replace role permissions",
        "替换角色权限"
    ),
    text!(
        "console.operations.roles.permissions.replace.description",
        "Allow users to replace role permissions",
        "允许替换角色权限"
    ),
    text!(
        "console.operations.roles.permissions.view.label",
        "View role permissions",
        "查看角色权限"
    ),
    text!(
        "console.operations.roles.permissions.view.description",
        "Allow users to view role permissions",
        "允许查看角色权限"
    ),
    text!(
        "console.operations.roles.update.label",
        "Update roles",
        "更新角色"
    ),
    text!(
        "console.operations.roles.update.description",
        "Allow users to update roles",
        "允许更新角色"
    ),
    text!(
        "console.operations.settings_feature.access.system.applications.label",
        "Access application management",
        "访问应用管理"
    ),
    text!(
        "console.operations.settings_feature.access.system.applications.description",
        "Allow access to application management",
        "允许访问应用管理"
    ),
    text!(
        "console.operations.settings_feature.access.system.data-models.label",
        "Access data model settings",
        "访问数据模型设置"
    ),
    text!(
        "console.operations.settings_feature.access.system.data-models.description",
        "Allow access to data model settings",
        "允许访问数据模型设置"
    ),
    text!(
        "console.operations.settings_feature.access.system.docs.label",
        "Access API documentation",
        "访问API 文档"
    ),
    text!(
        "console.operations.settings_feature.access.system.docs.description",
        "Allow access to API documentation",
        "允许访问API 文档"
    ),
    text!(
        "console.operations.system.release_status.view.label",
        "View release status",
        "查看发布状态"
    ),
    text!(
        "console.operations.system.release_status.view.description",
        "Allow users to view release status",
        "允许查看发布状态"
    ),
    text!(
        "console.operations.system.runtime_profile.view.label",
        "View runtime profile",
        "查看运行配置"
    ),
    text!(
        "console.operations.system.runtime_profile.view.description",
        "Allow users to view runtime profile",
        "允许查看运行配置"
    ),
    text!(
        "console.operations.workspace.update.label",
        "Update current workspace",
        "更新当前工作区"
    ),
    text!(
        "console.operations.workspace.update.description",
        "Allow users to update current workspace",
        "允许更新当前工作区"
    ),
    text!(
        "console.operations.workspace.view.label",
        "View current workspace",
        "查看当前工作区"
    ),
    text!(
        "console.operations.workspace.view.description",
        "Allow users to view current workspace",
        "允许查看当前工作区"
    ),
    text!(
        "console.operations.workspaces.list.label",
        "List workspaces",
        "查看工作区"
    ),
    text!(
        "console.operations.workspaces.list.description",
        "Allow users to list workspaces",
        "允许查看工作区"
    ),
];
