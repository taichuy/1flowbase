use super::CoreConsoleLocaleText;

pub(super) const TEXTS: &[CoreConsoleLocaleText] = &[
    text!(
        "console.operations.frontend_blocks.view.label",
        "View frontend block catalog",
        "查看前端区块目录"
    ),
    text!(
        "console.operations.frontend_blocks.view.description",
        "Allow users to view frontend block catalog",
        "允许查看前端区块目录"
    ),
    text!(
        "console.operations.js_dependencies.view.label",
        "View JavaScript dependencies",
        "查看JavaScript 依赖"
    ),
    text!(
        "console.operations.js_dependencies.view.description",
        "Allow users to view JavaScript dependencies",
        "允许查看JavaScript 依赖"
    ),
    text!(
        "console.operations.model_provider_plugins.artifact.install.label",
        "Install model provider plugin artifact",
        "安装模型提供商插件制品"
    ),
    text!(
        "console.operations.model_provider_plugins.artifact.install.description",
        "Allow users to install model provider plugin artifact",
        "允许安装模型提供商插件制品"
    ),
    text!(
        "console.operations.model_provider_plugins.artifact.refresh.label",
        "Refresh model provider plugin artifact",
        "刷新模型提供商插件制品"
    ),
    text!(
        "console.operations.model_provider_plugins.artifact.refresh.description",
        "Allow users to refresh model provider plugin artifact",
        "允许刷新模型提供商插件制品"
    ),
    text!(
        "console.operations.model_provider_plugins.families.delete.label",
        "Delete model provider plugin families",
        "删除模型提供商插件系列"
    ),
    text!(
        "console.operations.model_provider_plugins.families.delete.description",
        "Allow users to delete model provider plugin families",
        "允许删除模型提供商插件系列"
    ),
    text!(
        "console.operations.model_provider_plugins.families.switch.label",
        "Switch model provider plugin families",
        "切换模型提供商插件系列"
    ),
    text!(
        "console.operations.model_provider_plugins.families.switch.description",
        "Allow users to switch model provider plugin families",
        "允许切换模型提供商插件系列"
    ),
    text!(
        "console.operations.model_provider_plugins.families.upgrade.label",
        "Upgrade model provider plugin families",
        "升级模型提供商插件系列"
    ),
    text!(
        "console.operations.model_provider_plugins.families.upgrade.description",
        "Allow users to upgrade model provider plugin families",
        "允许升级模型提供商插件系列"
    ),
    text!(
        "console.operations.model_provider_plugins.families.view.label",
        "View model provider plugin families",
        "查看模型提供商插件系列"
    ),
    text!(
        "console.operations.model_provider_plugins.families.view.description",
        "Allow users to view model provider plugin families",
        "允许查看模型提供商插件系列"
    ),
    text!(
        "console.operations.model_provider_plugins.install.official.label",
        "Install official model provider plugin",
        "安装官方模型提供商插件"
    ),
    text!(
        "console.operations.model_provider_plugins.install.official.description",
        "Allow users to install an official model provider plugin",
        "允许安装官方模型提供商插件"
    ),
    text!(
        "console.operations.model_provider_plugins.install.upload.label",
        "Install uploaded model provider plugin",
        "安装上传的模型提供商插件"
    ),
    text!(
        "console.operations.model_provider_plugins.install.upload.description",
        "Allow users to install an uploaded model provider plugin",
        "允许安装上传的模型提供商插件"
    ),
    text!(
        "console.operations.model_provider_plugins.official_catalog.view.label",
        "View official model provider plugin catalog",
        "查看官方模型提供商插件目录"
    ),
    text!(
        "console.operations.model_provider_plugins.official_catalog.view.description",
        "Allow users to view official model provider plugin catalog",
        "允许查看官方模型提供商插件目录"
    ),
    text!(
        "console.operations.model_provider_plugins.tasks.view.label",
        "View model provider plugin tasks",
        "查看模型提供商插件任务"
    ),
    text!(
        "console.operations.model_provider_plugins.tasks.view.description",
        "Allow users to view model provider plugin tasks",
        "允许查看模型提供商插件任务"
    ),
    text!(
        "console.operations.model_providers.balance.view.label",
        "View model provider balance",
        "查看模型提供商余额"
    ),
    text!(
        "console.operations.model_providers.balance.view.description",
        "Allow users to view model provider balance",
        "允许查看模型提供商余额"
    ),
    text!(
        "console.operations.model_providers.catalog.view.label",
        "View model provider catalog",
        "查看模型提供商目录"
    ),
    text!(
        "console.operations.model_providers.catalog.view.description",
        "Allow users to view model provider catalog",
        "允许查看模型提供商目录"
    ),
    text!(
        "console.operations.model_providers.icons.view.label",
        "View model provider icons",
        "查看模型提供商图标"
    ),
    text!(
        "console.operations.model_providers.icons.view.description",
        "Allow users to view model provider icons",
        "允许查看模型提供商图标"
    ),
    text!(
        "console.operations.model_providers.instances.create.label",
        "Create model provider instances",
        "创建模型提供商实例"
    ),
    text!(
        "console.operations.model_providers.instances.create.description",
        "Allow users to create model provider instances",
        "允许创建模型提供商实例"
    ),
    text!(
        "console.operations.model_providers.instances.delete.label",
        "Delete model provider instances",
        "删除模型提供商实例"
    ),
    text!(
        "console.operations.model_providers.instances.delete.description",
        "Allow users to delete model provider instances",
        "允许删除模型提供商实例"
    ),
    text!(
        "console.operations.model_providers.instances.models.refresh.label",
        "Refresh model provider instance models",
        "刷新模型提供商实例模型"
    ),
    text!(
        "console.operations.model_providers.instances.models.refresh.description",
        "Allow users to refresh model provider instance models",
        "允许刷新模型提供商实例模型"
    ),
    text!(
        "console.operations.model_providers.instances.models.view.label",
        "View model provider instance models",
        "查看模型提供商实例模型"
    ),
    text!(
        "console.operations.model_providers.instances.models.view.description",
        "Allow users to view model provider instance models",
        "允许查看模型提供商实例模型"
    ),
    text!(
        "console.operations.model_providers.instances.secrets.reveal.label",
        "Reveal model provider instance secrets",
        "查看模型提供商实例密钥"
    ),
    text!(
        "console.operations.model_providers.instances.secrets.reveal.description",
        "Allow users to reveal model provider instance secrets",
        "允许查看模型提供商实例密钥"
    ),
    text!(
        "console.operations.model_providers.instances.update.label",
        "Update model provider instances",
        "更新模型提供商实例"
    ),
    text!(
        "console.operations.model_providers.instances.update.description",
        "Allow users to update model provider instances",
        "允许更新模型提供商实例"
    ),
    text!(
        "console.operations.model_providers.instances.validate.label",
        "Validate model provider instances",
        "校验模型提供商实例"
    ),
    text!(
        "console.operations.model_providers.instances.validate.description",
        "Allow users to validate model provider instances",
        "允许校验模型提供商实例"
    ),
    text!(
        "console.operations.model_providers.instances.view.label",
        "View model provider instances",
        "查看模型提供商实例"
    ),
    text!(
        "console.operations.model_providers.instances.view.description",
        "Allow users to view model provider instances",
        "允许查看模型提供商实例"
    ),
    text!(
        "console.operations.model_providers.main_instance.update.label",
        "Update model provider main instance",
        "更新模型提供商主实例"
    ),
    text!(
        "console.operations.model_providers.main_instance.update.description",
        "Allow users to update model provider main instance",
        "允许更新模型提供商主实例"
    ),
    text!(
        "console.operations.model_providers.main_instance.view.label",
        "View model provider main instance",
        "查看模型提供商主实例"
    ),
    text!(
        "console.operations.model_providers.main_instance.view.description",
        "Allow users to view model provider main instance",
        "允许查看模型提供商主实例"
    ),
    text!(
        "console.operations.model_providers.options.view.label",
        "View application model provider options",
        "查看应用可用的模型提供商选项"
    ),
    text!(
        "console.operations.model_providers.options.view.description",
        "Allow users to view model provider options used by applications",
        "允许查看应用使用的模型提供商选项"
    ),
    text!(
        "console.operations.model_providers.settings_options.view.label",
        "View model provider settings options",
        "查看模型提供商设置选项"
    ),
    text!(
        "console.operations.model_providers.settings_options.view.description",
        "Allow users to view options used to configure model providers",
        "允许查看配置模型提供商使用的选项"
    ),
    text!(
        "console.operations.model_providers.preview.view.label",
        "View model provider preview",
        "查看模型提供商预览"
    ),
    text!(
        "console.operations.model_providers.preview.view.description",
        "Allow users to view model provider preview",
        "允许查看模型提供商预览"
    ),
    text!(
        "console.operations.model_providers.request_logs.clear.label",
        "Clear model provider request logs",
        "清除模型提供商请求日志"
    ),
    text!(
        "console.operations.model_providers.request_logs.clear.description",
        "Allow users to clear model provider request logs",
        "允许清除模型提供商请求日志"
    ),
    text!(
        "console.operations.model_providers.request_logs.delete.label",
        "Delete model provider request logs",
        "删除模型提供商请求日志"
    ),
    text!(
        "console.operations.model_providers.request_logs.delete.description",
        "Allow users to delete model provider request logs",
        "允许删除模型提供商请求日志"
    ),
    text!(
        "console.operations.model_providers.request_logs.view.label",
        "View model provider request logs",
        "查看模型提供商请求日志"
    ),
    text!(
        "console.operations.model_providers.request_logs.view.description",
        "Allow users to view model provider request logs",
        "允许查看模型提供商请求日志"
    ),
    text!(
        "console.operations.node_contributions.view.label",
        "View node contribution catalog",
        "查看节点贡献目录"
    ),
    text!(
        "console.operations.node_contributions.view.description",
        "Allow users to view node contribution catalog",
        "允许查看节点贡献目录"
    ),
    text!(
        "console.operations.plugins.artifact.install.label",
        "Install plugin artifact",
        "安装插件制品"
    ),
    text!(
        "console.operations.plugins.artifact.install.description",
        "Allow users to install plugin artifact",
        "允许安装插件制品"
    ),
    text!(
        "console.operations.plugins.artifact.refresh.label",
        "Refresh plugin artifact",
        "刷新插件制品"
    ),
    text!(
        "console.operations.plugins.artifact.refresh.description",
        "Allow users to refresh plugin artifact",
        "允许刷新插件制品"
    ),
    text!(
        "console.operations.plugins.assign.label",
        "Assign plugins",
        "分配插件"
    ),
    text!(
        "console.operations.plugins.assign.description",
        "Allow users to assign plugins",
        "允许分配插件"
    ),
    text!(
        "console.operations.plugins.catalog.view.label",
        "View plugin catalog",
        "查看插件目录"
    ),
    text!(
        "console.operations.plugins.catalog.view.description",
        "Allow users to view plugin catalog",
        "允许查看插件目录"
    ),
    text!(
        "console.operations.plugins.catalog_projection.refresh.label",
        "Refresh plugin catalog projection",
        "刷新插件目录投影"
    ),
    text!(
        "console.operations.plugins.catalog_projection.refresh.description",
        "Allow users to refresh plugin catalog projection",
        "允许刷新插件目录投影"
    ),
    text!(
        "console.operations.plugins.enable.label",
        "Enable plugins",
        "启用插件"
    ),
    text!(
        "console.operations.plugins.enable.description",
        "Allow users to enable plugins",
        "允许启用插件"
    ),
    text!(
        "console.operations.plugins.families.delete.label",
        "Delete plugin families",
        "删除插件系列"
    ),
    text!(
        "console.operations.plugins.families.delete.description",
        "Allow users to delete plugin families",
        "允许删除插件系列"
    ),
    text!(
        "console.operations.plugins.families.switch.label",
        "Switch plugin families",
        "切换插件系列"
    ),
    text!(
        "console.operations.plugins.families.switch.description",
        "Allow users to switch plugin families",
        "允许切换插件系列"
    ),
    text!(
        "console.operations.plugins.families.upgrade.label",
        "Upgrade plugin families",
        "升级插件系列"
    ),
    text!(
        "console.operations.plugins.families.upgrade.description",
        "Allow users to upgrade plugin families",
        "允许升级插件系列"
    ),
    text!(
        "console.operations.plugins.families.view.label",
        "View plugin families",
        "查看插件系列"
    ),
    text!(
        "console.operations.plugins.families.view.description",
        "Allow users to view plugin families",
        "允许查看插件系列"
    ),
    text!(
        "console.operations.plugins.install.label",
        "Install plugins",
        "安装插件"
    ),
    text!(
        "console.operations.plugins.install.description",
        "Allow users to install plugins",
        "允许安装插件"
    ),
    text!(
        "console.operations.plugins.install.official.label",
        "Install official plugin",
        "安装官方插件"
    ),
    text!(
        "console.operations.plugins.install.official.description",
        "Allow users to install an official plugin",
        "允许安装官方插件"
    ),
    text!(
        "console.operations.plugins.install.upload.label",
        "Install uploaded plugin",
        "安装上传的插件"
    ),
    text!(
        "console.operations.plugins.install.upload.description",
        "Allow users to install an uploaded plugin",
        "允许安装上传的插件"
    ),
    text!(
        "console.operations.plugins.official_catalog.view.label",
        "View official plugin catalog",
        "查看官方插件目录"
    ),
    text!(
        "console.operations.plugins.official_catalog.view.description",
        "Allow users to view official plugin catalog",
        "允许查看官方插件目录"
    ),
    text!(
        "console.operations.plugins.tasks.view.label",
        "View plugin tasks",
        "查看插件任务"
    ),
    text!(
        "console.operations.plugins.tasks.view.description",
        "Allow users to view plugin tasks",
        "允许查看插件任务"
    ),
];
