---
memory_type: project
topic: system-runtime-live-monitoring-approved
summary: 用户确认并已实现 system runtime 平衡方案：2 秒 HTTP polling、前端 120 秒易失窗口、API Server / Plugin Runner 独立采集；target snapshot 使用 HostInfrastructure CacheStore 做 1 秒新鲜度、10 秒保留，并通过 DistributedLock 合并并发刷新及接入内存观察。
keywords:
  - system-runtime
  - runtime-profile
  - live-monitoring
  - cgroup
  - polling
  - echarts
  - cache-store
  - distributed-lock
  - memory-observation
match_when:
  - 继续调整 /settings/system-runtime 页面
  - 讨论宿主或容器资源实时采集、持久化边界或外部监控依赖
  - 修改 runtime-profile 的 runtime_targets 或资源指标合同
created_at: 2026-07-17 13
updated_at: 2026-07-17 15
last_verified_at: 2026-07-17 15
decision_policy: verify_before_decision
scope:
  - api/crates/runtime-profile
  - api/apps/api-server
  - api/apps/plugin-runner
  - web/app/src/features/settings
  - web/packages/api-client
---

# System Runtime 实时资源监控方案已确认

## 谁在做什么

用户已确认平衡方案并授权直接实现；AI 已完成 `/settings/system-runtime` 紧凑重构、API Server / Plugin Runner 独立运行目标、前端实时曲线，以及基于 HostInfrastructure ephemeral contract 的可观察 target snapshot 缓存。定向自动化与浏览器视口验证已完成，当前尚未提交或 push。

## 为什么这样做

旧页面存在较多中间空白，运行信息分散；用户希望页面打开时直接看到当前进程或容器可触达环境的 CPU、内存、存储、网络流量和磁盘 I/O，同时不建设长期指标存储。

## 为什么要做

该能力用于本地开发与容器内的即时诊断，应保持默认部署轻量，避免为了短窗口排障引入 Prometheus、Docker API、Kubernetes API 或数据库时序表。

## 已确认决策

- 复用 `GET /api/console/system/runtime-profile` 与既有 `system.runtime_profile.view` 权限，不新增路由、权限点或数据库。
- 前端每 2 秒轮询，内存中保留最近 120 秒且最多 60 点；数据不持久化。
- API Server 与 Plugin Runner 以独立 `runtime_target` 返回，即使同一 `host_fingerprint` 也不合并监控序列。
- 速率由后端计算，前端只负责窗口和绘图；200 ms 内的并发请求复用最近样本。
- API Server 聚合 target snapshot 前先读取 `CacheStore`；快照 1 秒内可直接返回、在 ephemeral 中保留 10 秒，通过 `DistributedLock` 和 double-check 合并并发刷新。
- Cache key 使用 `system-runtime:v1:snapshot:{api_node_id}:{runtime_instance_id}:{target_id}`，因此自动进入内存观察的 Cache 树；API Server 与 Plugin Runner 各一条，Runner 不可达也作为短期 observation 缓存。
- 缓存只保存与用户无关的 target profile；鉴权、locale 和最终 `SystemRuntimeProfileResponse` 仍按请求生成。无效缓存条目先驱逐再重新采集，缓存不成为运行环境真值。
- 页面隐藏、组件卸载或连续 3 次请求失败时停止采集；成功采样会清空连续失败计数。
- Linux 真容器或受限 cgroup 优先使用 cgroup v2；本地无资源限制的 systemd session 使用 host，cgroup CPU 不可读时回退 host。
- 不可用、预热或过期指标通过 `availability` 明确表达，不用数值 0 冒充。
- 图表使用 ECharts 原生宿主，不增加 wrapper；画布初始化一次，轮询时只更新 option。
- 页面沿用 SettingsRouteShell 的固定 viewport 高度，并由 `SettingsSectionSurface fill` 的 body 接管纵向滚动；左侧导航保持固定。
- ECharts 宿主显式保持可收缩；从 2048px 缩到 1400px / 1200px 时，chart、canvas 和滚动 body 同步收窄，不产生横向衍生。
- 页面信息顺序固定为“运行概览 → 运行环境 → 资源监控”；运行目标下拉归属运行环境，监控标题只显示采集状态和最近 2 分钟窗口。
- 运行环境移除当前语言、回退语言、支持语言，仅保留进程内存、插件安装路径与宿主扩展路径；使用 Ant Design Descriptions 纵向标签布局，桌面三列、窄屏单列，长路径在容器内换行。

## 截止日期

无固定截止日期；实现与自动化验证已于 2026-07-17 完成，提交等待用户视觉确认。

## 决策背后动机

优先解决“打开页面即可诊断”的产品问题，把实时性和复杂度控制在当前进程可承担的范围内；易失短窗口足以覆盖开发与容器排障，同时保留未来接入专门可观测平台的清晰边界。
