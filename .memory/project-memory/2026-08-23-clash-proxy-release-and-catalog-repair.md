---
memory_type: project
topic: Clash Proxy provider packaging and Network Center catalog repair
summary: 用户确认以平衡方案修复 Clash/Mihomo Proxy 的发布流水线与目录分类；0.2.1 已作为 network_egress_provider 发布并可被 Network Center 官方目录筛选。
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
updated_at: 2026-08-23 12
last_verified_at: 2026-08-23 12
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
- 截止日期：已于 2026-08-23 完成并发布 `clash-proxy-v0.2.1`；目录提交固定为 `0bca56a`。后续如 UI 未立即展示，先按正常目录刷新 TTL 或重启服务刷新官方目录缓存，再查 API 读取结果。
- 后续上传边界：用户确认插件包大小限制由全局 `API_PLUGIN_UPLOAD_MAX_BYTES` 统一配置，默认 8 MiB；网络中心、扩展中心、模型提供者与通用插件上传不得分别设置上限。
