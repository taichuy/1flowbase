use super::CoreConsoleLocaleText;

pub(super) const TEXTS: &[CoreConsoleLocaleText] = &[
    text!(
        "console.operations.host_infrastructure.cache.domain.clear.label",
        "Clear cache domain",
        "清除缓存域"
    ),
    text!(
        "console.operations.host_infrastructure.cache.domain.clear.description",
        "Allow users to clear cache domain",
        "允许清除缓存域"
    ),
    text!(
        "console.operations.host_infrastructure.cache.entry.clear.label",
        "Clear cache entry",
        "清除缓存条目"
    ),
    text!(
        "console.operations.host_infrastructure.cache.entry.clear.description",
        "Allow users to clear cache entry",
        "允许清除缓存条目"
    ),
    text!(
        "console.operations.host_infrastructure.cache.reveal.label",
        "Reveal cache",
        "查看缓存"
    ),
    text!(
        "console.operations.host_infrastructure.cache.reveal.description",
        "Allow users to reveal cache",
        "允许查看缓存"
    ),
    text!(
        "console.operations.host_infrastructure.cache.view.label",
        "View cache",
        "查看缓存"
    ),
    text!(
        "console.operations.host_infrastructure.cache.view.description",
        "Allow users to view cache",
        "允许查看缓存"
    ),
    text!(
        "console.operations.host_infrastructure.memory.reveal.label",
        "Reveal memory observation",
        "查看内存观测"
    ),
    text!(
        "console.operations.host_infrastructure.memory.reveal.description",
        "Allow users to reveal memory observation",
        "允许查看内存观测"
    ),
    text!(
        "console.operations.host_infrastructure.memory.view.label",
        "View memory observation",
        "查看内存观测"
    ),
    text!(
        "console.operations.host_infrastructure.memory.view.description",
        "Allow users to view memory observation",
        "允许查看内存观测"
    ),
    text!(
        "console.operations.host_infrastructure.providers.configure.label",
        "Configure infrastructure providers",
        "配置基础设施提供商"
    ),
    text!(
        "console.operations.host_infrastructure.providers.configure.description",
        "Allow users to configure infrastructure providers",
        "允许配置基础设施提供商"
    ),
    text!(
        "console.operations.host_infrastructure.providers.view.label",
        "View infrastructure providers",
        "查看基础设施提供商"
    ),
    text!(
        "console.operations.host_infrastructure.providers.view.description",
        "Allow users to view infrastructure providers",
        "允许查看基础设施提供商"
    ),
    text!(
        "console.operations.mcp.bundles.export.label",
        "Export MCP bundles",
        "导出MCP 套件"
    ),
    text!(
        "console.operations.mcp.bundles.export.description",
        "Allow users to export MCP bundles",
        "允许导出MCP 套件"
    ),
    text!(
        "console.operations.mcp.bundles.import.label",
        "Import MCP bundles",
        "导入MCP 套件"
    ),
    text!(
        "console.operations.mcp.bundles.import.description",
        "Allow users to import MCP bundles",
        "允许导入MCP 套件"
    ),
    text!(
        "console.operations.mcp.bundles.official.list.label",
        "List official MCP bundles",
        "查看官方 MCP 套件"
    ),
    text!(
        "console.operations.mcp.bundles.official.list.description",
        "Allow users to list official MCP bundles",
        "允许查看官方 MCP 套件"
    ),
    text!(
        "console.operations.mcp.bundles.preview.label",
        "Preview MCP bundles",
        "预览MCP 套件"
    ),
    text!(
        "console.operations.mcp.bundles.preview.description",
        "Allow users to preview MCP bundles",
        "允许预览MCP 套件"
    ),
    text!(
        "console.operations.mcp.catalog.export.label",
        "Export MCP catalog",
        "导出MCP 目录"
    ),
    text!(
        "console.operations.mcp.catalog.export.description",
        "Allow users to export MCP catalog",
        "允许导出MCP 目录"
    ),
    text!(
        "console.operations.mcp.catalog.view.label",
        "View MCP catalog",
        "查看MCP 目录"
    ),
    text!(
        "console.operations.mcp.catalog.view.description",
        "Allow users to view MCP catalog",
        "允许查看MCP 目录"
    ),
    text!(
        "console.operations.mcp.client_credential.delete.label",
        "Delete MCP client credential",
        "删除MCP 客户端凭据"
    ),
    text!(
        "console.operations.mcp.client_credential.delete.description",
        "Allow users to delete MCP client credential",
        "允许删除MCP 客户端凭据"
    ),
    text!(
        "console.operations.mcp.client_credential.reveal.label",
        "Reveal MCP client credential",
        "查看MCP 客户端凭据"
    ),
    text!(
        "console.operations.mcp.client_credential.reveal.description",
        "Allow users to reveal MCP client credential",
        "允许查看MCP 客户端凭据"
    ),
    text!(
        "console.operations.mcp.client_credential.save.label",
        "Save MCP client credential",
        "保存MCP 客户端凭据"
    ),
    text!(
        "console.operations.mcp.client_credential.save.description",
        "Allow users to save MCP client credential",
        "允许保存MCP 客户端凭据"
    ),
    text!(
        "console.operations.mcp.debug.execute.label",
        "Execute MCP request",
        "执行MCP 请求"
    ),
    text!(
        "console.operations.mcp.debug.execute.description",
        "Allow users to execute MCP request",
        "允许执行MCP 请求"
    ),
    text!(
        "console.operations.mcp.discovery_policy.update.label",
        "Update MCP discovery policy",
        "更新MCP 发现策略"
    ),
    text!(
        "console.operations.mcp.discovery_policy.update.description",
        "Allow users to update MCP discovery policy",
        "允许更新MCP 发现策略"
    ),
    text!(
        "console.operations.mcp.discovery_policy.view.label",
        "View MCP discovery policy",
        "查看MCP 发现策略"
    ),
    text!(
        "console.operations.mcp.discovery_policy.view.description",
        "Allow users to view MCP discovery policy",
        "允许查看MCP 发现策略"
    ),
    text!(
        "console.operations.mcp.groups.delete.label",
        "Delete MCP groups",
        "删除MCP 分组"
    ),
    text!(
        "console.operations.mcp.groups.delete.description",
        "Allow users to delete MCP groups",
        "允许删除MCP 分组"
    ),
    text!(
        "console.operations.mcp.groups.move.label",
        "Move MCP groups",
        "移动MCP 分组"
    ),
    text!(
        "console.operations.mcp.groups.move.description",
        "Allow users to move MCP groups",
        "允许移动MCP 分组"
    ),
    text!(
        "console.operations.mcp.groups.upsert.label",
        "Save MCP groups",
        "保存MCP 分组"
    ),
    text!(
        "console.operations.mcp.groups.upsert.description",
        "Allow users to save MCP groups",
        "允许保存MCP 分组"
    ),
    text!(
        "console.operations.mcp.instances.copy.label",
        "Copy MCP instances",
        "复制MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.copy.description",
        "Allow users to copy MCP instances",
        "允许复制MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.create.label",
        "Create MCP instances",
        "创建MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.create.description",
        "Allow users to create MCP instances",
        "允许创建MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.delete.label",
        "Delete MCP instances",
        "删除MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.delete.description",
        "Allow users to delete MCP instances",
        "允许删除MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.export.label",
        "Export MCP instances",
        "导出MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.export.description",
        "Allow users to export MCP instances",
        "允许导出MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.update.label",
        "Update MCP instances",
        "更新MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.update.description",
        "Allow users to update MCP instances",
        "允许更新MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.view.label",
        "View MCP instances",
        "查看MCP 实例"
    ),
    text!(
        "console.operations.mcp.instances.view.description",
        "Allow users to view MCP instances",
        "允许查看MCP 实例"
    ),
    text!(
        "console.operations.mcp.tool_bindings.create.label",
        "Create MCP tool bindings",
        "创建MCP 工具绑定"
    ),
    text!(
        "console.operations.mcp.tool_bindings.create.description",
        "Allow users to create MCP tool bindings",
        "允许创建MCP 工具绑定"
    ),
    text!(
        "console.operations.mcp.tool_bindings.delete.label",
        "Delete MCP tool bindings",
        "删除MCP 工具绑定"
    ),
    text!(
        "console.operations.mcp.tool_bindings.delete.description",
        "Allow users to delete MCP tool bindings",
        "允许删除MCP 工具绑定"
    ),
    text!(
        "console.operations.mcp.tool_bindings.update.label",
        "Update MCP tool bindings",
        "更新MCP 工具绑定"
    ),
    text!(
        "console.operations.mcp.tool_bindings.update.description",
        "Allow users to update MCP tool bindings",
        "允许更新MCP 工具绑定"
    ),
    text!(
        "console.operations.mcp.tools.create.label",
        "Create MCP tools",
        "创建MCP 工具"
    ),
    text!(
        "console.operations.mcp.tools.create.description",
        "Allow users to create MCP tools",
        "允许创建MCP 工具"
    ),
    text!(
        "console.operations.mcp.tools.delete.label",
        "Delete MCP tools",
        "删除MCP 工具"
    ),
    text!(
        "console.operations.mcp.tools.delete.description",
        "Allow users to delete MCP tools",
        "允许删除MCP 工具"
    ),
    text!(
        "console.operations.mcp.tools.description.check.label",
        "Check MCP tool descriptions",
        "检查MCP 工具描述"
    ),
    text!(
        "console.operations.mcp.tools.description.check.description",
        "Allow users to check MCP tool descriptions",
        "允许检查MCP 工具描述"
    ),
    text!(
        "console.operations.mcp.tools.description.refresh.label",
        "Refresh MCP tool descriptions",
        "刷新MCP 工具描述"
    ),
    text!(
        "console.operations.mcp.tools.description.refresh.description",
        "Allow users to refresh MCP tool descriptions",
        "允许刷新MCP 工具描述"
    ),
    text!(
        "console.operations.mcp.tools.update.label",
        "Update MCP tools",
        "更新MCP 工具"
    ),
    text!(
        "console.operations.mcp.tools.update.description",
        "Allow users to update MCP tools",
        "允许更新MCP 工具"
    ),
    text!(
        "console.operations.mcp.tools.view.label",
        "View MCP tools",
        "查看MCP 工具"
    ),
    text!(
        "console.operations.mcp.tools.view.description",
        "Allow users to view MCP tools",
        "允许查看MCP 工具"
    ),
    text!(
        "console.operations.mcp.upstream_connections.create.label",
        "Create MCP upstream connections",
        "创建MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.create.description",
        "Allow users to create MCP upstream connections",
        "允许创建MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.delete.label",
        "Delete MCP upstream connections",
        "删除MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.delete.description",
        "Allow users to delete MCP upstream connections",
        "允许删除MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.discover.label",
        "Discover MCP upstream connections",
        "发现MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.discover.description",
        "Allow users to discover MCP upstream connections",
        "允许发现MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.test.label",
        "Test MCP upstream connections",
        "测试MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.test.description",
        "Allow users to test MCP upstream connections",
        "允许测试MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.update.label",
        "Update MCP upstream connections",
        "更新MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.update.description",
        "Allow users to update MCP upstream connections",
        "允许更新MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.view.label",
        "View MCP upstream connections",
        "查看MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_connections.view.description",
        "Allow users to view MCP upstream connections",
        "允许查看MCP 上游连接"
    ),
    text!(
        "console.operations.mcp.upstream_credentials.delete.label",
        "Delete MCP upstream credentials",
        "删除MCP 上游凭据"
    ),
    text!(
        "console.operations.mcp.upstream_credentials.delete.description",
        "Allow users to delete MCP upstream credentials",
        "允许删除MCP 上游凭据"
    ),
    text!(
        "console.operations.mcp.upstream_credentials.update.label",
        "Update MCP upstream credentials",
        "更新MCP 上游凭据"
    ),
    text!(
        "console.operations.mcp.upstream_credentials.update.description",
        "Allow users to update MCP upstream credentials",
        "允许更新MCP 上游凭据"
    ),
    text!(
        "console.operations.mcp.upstream_tools.debug.label",
        "Debug MCP upstream tools",
        "调试MCP 上游工具"
    ),
    text!(
        "console.operations.mcp.upstream_tools.debug.description",
        "Allow users to debug MCP upstream tools",
        "允许调试MCP 上游工具"
    ),
    text!(
        "console.operations.mcp.upstream_tools.import.label",
        "Import MCP upstream tools",
        "导入MCP 上游工具"
    ),
    text!(
        "console.operations.mcp.upstream_tools.import.description",
        "Allow users to import MCP upstream tools",
        "允许导入MCP 上游工具"
    ),
];
