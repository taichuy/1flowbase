---
memory_type: project
topic: Clash Proxy provider packaging and Network Center catalog repair
summary: Clash/Mihomo Proxy 已完成发布/目录修复与按 egress 键控的有界运行时热池；插件 main 和主仓 dev 已合入，Root #1858 等待用户验收。
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
updated_at: 2026-08-24 11
last_verified_at: 2026-08-24 11
decision_policy: verify_before_decision
status: active
scope:
  - /home/taichuy/git/1flowbase-official-plugins/.github/workflows/provider-release.yml
  - /home/taichuy/git/1flowbase-official-plugins/runtime-extensions/@taichuy/clash-proxy/manifest.yaml
  - api/apps/api-server/src/routes/network_center/plugins.rs
  - api/apps/api-server/src/config.rs
  - /home/taichuy/git/1flowbase-official-plugins/runtime-extensions/@taichuy/clash-proxy/src/runtime_pool.rs
  - api/apps/plugin-runner/src/network_egress_host.rs
  - api/apps/api-server/src/network_egress_probe.rs
---

# Clash Proxy 发布与目录修复

- 谁在做什么：官方插件仓库维护 Clash/Mihomo Proxy 的跨平台构建和官方目录；主仓 Network Center 只消费 `network_egress_provider` 类型的目录条目。
- 为什么这样做：构建源码原先放在会被打包步骤整体清空的 `dist/` 下，导致所有平台在编译前失去 Mihomo 源码；即使发布完成，默认 `model_provider` 分类也会被 Network Center 过滤掉。
- 为什么要做：用户确认采用将构建源码移出可清理输出目录、保留现有发布修复流程，并把扩展明确声明为 `network_egress_provider` 的平衡方案。

## 有界运行时热池

- 用户在 2026-08-24 09 确认：Clash Provider 不应继续把 Mihomo 进程生命周期绑定到每次业务调用；采用按 `provider_egress_key` 键控、容量有界、引用计数、Singleflight、LRU / Idle TTL 与失败退避的热池，现有 acquire / release ABI 保持不变。
- 根因证据：插件 `memory_bytes=256 MiB` 被 runner 设置为 `RLIMIT_AS` 并由 Mihomo 子进程继承；Mihomo 在 256 MiB 与 512 MiB 下无法启动，1 GiB 可启动。地址空间限制不等于实际 RSS。
- 交付真值：Root https://github.com/taichuy/1flowbase/issues/1858；插件 Delivery https://github.com/taichuy/1flowbase-official-plugins/issues/3；Host Delivery https://github.com/taichuy/1flowbase/issues/1859。2026-08-24 11 已合入插件 `main@f8cab47` 与主仓 `dev@06deb7814`，三个 Issue 均为 `phase:user-acceptance`。
- 设计边界：前端不拥有进程调度；不实现单 Mihomo 全局切换出口；池容量依据实际 `M_peak` 与资源预算 `C ≤ floor((B - B₀) / M_peak)` 推导；正式跨平台产物证据由 release CI 留存。
- 资源与 QA 证据：真实 Mihomo 在 1 GiB `RLIMIT_AS` 下 `VmRSS=30,940 KiB`、`VmHWM=32,016 KiB`；容量按 `(2 GiB - 256 MiB) / 384 MiB = 4` 固化。插件 21 tests、真实 core benchmark、Runner 6 tests、cancellation exact-release 和错误分类均通过；Worker spawn 时保存不可变 PGID，leader 异常退出后由 Host 回收整个进程组。
- 截止日期：已于 2026-08-23 发布兼容包 `clash-proxy-v0.2.2`；目录提交固定为 `b8aeba0`。后续如 UI 未立即展示，先按正常目录刷新 TTL 或重启服务刷新官方目录缓存，再查 API 读取结果。
- 后续上传边界：用户确认插件包大小限制由全局 `API_PLUGIN_UPLOAD_MAX_BYTES` 统一配置，默认 8 MiB；网络中心、扩展中心、模型提供者与通用插件上传不得分别设置上限。
- 运行态注意：API 最终使用启动时编译的 console route assembly；全局上传配置必须传入 `compile_console_boot_plan_with_interface_operations_and_plugin_upload_max_bytes`，仅在后续 router factory 传值不会影响已编译的启动路由。
- Manifest 边界：`plugin_type` 是官方目录投影字段，不是 `PluginManifestV1` 字段。目录生成器必须由标准 `slot_codes` 推导 `network_egress_provider`，包内 Manifest 不得包含该未知字段。
- 真实订阅兼容：用户确认并在 `clash-proxy-v0.2.3` 落地“平衡”范围：以 `clash.meta` 请求标识获取原始 Clash/Mihomo YAML，仅投影 `proxies` 为独立回环 egress，保留允许的远程节点字段（含 `trojan`、`vless`、`hysteria2`）；规则、分组、DNS、监听器与递归 provider 一律不导入。发布完成时目录记录为 `0.2.3`、`network_egress_provider`、6 个平台资产。
- Ambient proxy 修复：`ureq 3.x` 默认继承宿主 `ALL_PROXY / HTTPS_PROXY / HTTP_PROXY`；开发运行态的 `127.0.0.1:7897` 使订阅请求 `ConnectionRefused`。`clash-proxy-v0.2.5` 的订阅专用 Agent 显式使用 `proxy(None)`，避免网络出口 Provider 递归依赖宿主代理。
- 运行态验收：`0.2.5` 六平台签名资产已发布，Linux amd64 包 SHA-256 为 `905305ed9bc58ec27b15164cc2068bb0fca3db3f269478c7fd439b98b92fb341`；开发环境安装后为 `verified_official / signature_status=verified / is_current=true`。真实订阅通过 `/api/console/network-center/pools/proxies` 创建返回 HTTP 201，Provider 为 `healthy`，投影 113 个全部可用 egress，且 `last_sync_error=null`。
- 主仓目录修复：`PluginManagementService::list_catalog` 不再只投影 `model_provider`，同时按当前 contract 与 metadata 投影已安装的 `network_egress_provider`；开发环境 families 与 official catalog 均正确识别当前 `0.2.5`。
