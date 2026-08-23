---
memory_type: project
topic: Clash Proxy provider packaging and Network Center catalog repair
summary: 用户确认以平衡方案修复 Clash/Mihomo Proxy 的发布流水线与目录分类；0.2.2 已作为 network_egress_provider 发布并可被 Network Center 官方目录筛选，且不再向严格 Manifest schema 写入目录专用字段。
keywords:
  - clash-proxy
  - mihomo
  - runtime extension
  - network egress provider
  - official plugin registry
match_when:
  - 排查 Clash/Mihomo Proxy 未打包、未发布或未在网络中心展示
  - 修改官方 runtime extension 发布工作流或目录分类
created_at: 2026-08-23 11
updated_at: 2026-08-23 17
last_verified_at: 2026-08-23 17
decision_policy: verify_before_decision
status: active
scope:
  - /home/taichuy/git/1flowbase-official-plugins/.github/workflows/provider-release.yml
  - /home/taichuy/git/1flowbase-official-plugins/runtime-extensions/@taichuy/clash-proxy/manifest.yaml
  - api/apps/api-server/src/routes/network_center/plugins.rs
  - api/apps/api-server/src/config.rs
---

# Clash Proxy 发布与目录修复

- 谁在做什么：官方插件仓库维护 Clash/Mihomo Proxy 的跨平台构建和官方目录；主仓 Network Center 只消费 `network_egress_provider` 类型的目录条目。
- 为什么这样做：构建源码原先放在会被打包步骤整体清空的 `dist/` 下，导致所有平台在编译前失去 Mihomo 源码；即使发布完成，默认 `model_provider` 分类也会被 Network Center 过滤掉。
- 为什么要做：用户确认采用将构建源码移出可清理输出目录、保留现有发布修复流程，并把扩展明确声明为 `network_egress_provider` 的平衡方案。
- 截止日期：已于 2026-08-23 发布兼容包 `clash-proxy-v0.2.2`；目录提交固定为 `b8aeba0`。后续如 UI 未立即展示，先按正常目录刷新 TTL 或重启服务刷新官方目录缓存，再查 API 读取结果。
- 后续上传边界：用户确认插件包大小限制由全局 `API_PLUGIN_UPLOAD_MAX_BYTES` 统一配置，默认 8 MiB；网络中心、扩展中心、模型提供者与通用插件上传不得分别设置上限。
- 运行态注意：API 最终使用启动时编译的 console route assembly；全局上传配置必须传入 `compile_console_boot_plan_with_interface_operations_and_plugin_upload_max_bytes`，仅在后续 router factory 传值不会影响已编译的启动路由。
- Manifest 边界：`plugin_type` 是官方目录投影字段，不是 `PluginManifestV1` 字段。目录生成器必须由标准 `slot_codes` 推导 `network_egress_provider`，包内 Manifest 不得包含该未知字段。
- 真实订阅兼容：用户确认并在 `clash-proxy-v0.2.3` 落地“平衡”范围：以 `clash.meta` 请求标识获取原始 Clash/Mihomo YAML，仅投影 `proxies` 为独立回环 egress，保留允许的远程节点字段（含 `trojan`、`vless`、`hysteria2`）；规则、分组、DNS、监听器与递归 provider 一律不导入。发布完成时目录记录为 `0.2.3`、`network_egress_provider`、6 个平台资产。
