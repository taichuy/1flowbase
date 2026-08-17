---
memory_type: project
topic: 扩展中心成为插件生命周期唯一管理入口
summary: 用户确认并已实施平衡方案：可执行插件安装/升级后自动激活，历史 current/assigned 版本按启用迁移、非当前历史版本 dormant；Runtime/Capability 热启停，HostExtension 重启生效；MCP 安装升级只自动选择本地当前 Bundle，workspace 导入仍需预览确认；分类目录页先读取本地安装库存，远程目录与更新检查失败仅降级在线目录/更新标记；扩展中心统一承担启停，模型供应商页保留该领域的官方安装、本地上传、升级与版本切换入口。
keywords:
  - extension-center
  - plugin-lifecycle
  - desired-state
  - host-extension
  - mcp-bundle
  - model-provider
  - immutable-package
match_when:
  - 调整插件安装、升级、启停或历史迁移
  - 调整扩展中心、模型供应商页或 MCP Bundle 入口职责
  - 修改官方插件发版与 version bump 门禁
created_at: 2026-08-15 10
updated_at: 2026-08-17 23
last_verified_at: 2026-08-17 23
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane/src/plugin_management
  - api/apps/api-server/src/routes/plugins_and_models/plugins/extension_center.rs
  - web/app/src/features/settings/pages/settings-page/SettingsExtensionCenterSection.tsx
  - web/app/src/features/settings/pages/settings-page/SettingsModelProvidersSection.tsx
  - /home/taichuy/git/1flowbase-official-plugins/scripts/detect-version-releases.mjs
---

# 扩展中心生命周期 Owner

2026-08-17 23：谁在做什么：`SettingsExtensionCenterSection` 在每个分类页先消费后端本地安装库存，再请求在线目录；更新检查只提交本地已安装版本集合。为什么这样做：本地安装状态不依赖网络，远程源只拥有目录与最新版本真值。为什么要做：断网、冷启动无目录缓存或远程超时时，已安装扩展仍须可查看和操作，失败只显示在在线目录提示和更新按钮状态。截止日期：已随 `99c445a59` 合入 `dev`。决策动机：将网络不确定性限制在它实际拥有的语义范围，避免在线目录阻塞本地生命周期管理。

谁在做什么：扩展中心统一管理可执行插件的 desired state（启用/关闭）；模型供应商页保留模型供应商插件的官方安装、本地上传、升级与版本切换，同时管理实例、密钥、模型目录与路由；MCP Management 只管理 workspace MCP 实例导入和配置。

为什么这样做：worker 启停、Bundle 本地版本选择和 workspace 配置导入是三种不同副作用，必须统一入口但不能压成同一个状态语义。

关键边界：模型供应商页不得再承担启停；HostExtension 不热卸载；MCP 新版本不静默覆盖 workspace；源码、manifest 或实际包输入变化必须 bump version、重新打包并发布新 checksum/签名，readme/demo/tests/target 变化不触发发版门禁。
