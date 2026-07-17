---
memory_type: project
topic: system-runtime-live-monitoring-approved
summary: 用户确认并已实现 system runtime 平衡方案：复用 runtime-profile 权限与接口，以 2 秒 HTTP polling 在前端保留 120 秒易失窗口，可切换 API Server / Plugin Runner，采集 CPU、内存、存储、网络和磁盘 I/O，不持久化且不引入外部监控依赖。
keywords:
  - system-runtime
  - runtime-profile
  - live-monitoring
  - cgroup
  - polling
  - echarts
match_when:
  - 继续调整 /settings/system-runtime 页面
  - 讨论宿主或容器资源实时采集、持久化边界或外部监控依赖
  - 修改 runtime-profile 的 runtime_targets 或资源指标合同
created_at: 2026-07-17 13
updated_at: 2026-07-17 13
last_verified_at: 2026-07-17 13
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

用户已确认平衡方案并授权直接实现；AI 已完成 `/settings/system-runtime` 紧凑重构、后端按请求采样、API Server / Plugin Runner 独立运行目标和前端实时曲线，并完成定向自动化与浏览器视口验证。当前等待用户确认最终视觉效果，尚未提交或 push。

## 为什么这样做

旧页面存在较多中间空白，运行信息分散；用户希望页面打开时直接看到当前进程或容器可触达环境的 CPU、内存、存储、网络流量和磁盘 I/O，同时不建设长期指标存储。

## 为什么要做

该能力用于本地开发与容器内的即时诊断，应保持默认部署轻量，避免为了短窗口排障引入 Prometheus、Docker API、Kubernetes API 或数据库时序表。

## 已确认决策

- 复用 `GET /api/console/system/runtime-profile` 与既有 `system.runtime_profile.view` 权限，不新增路由、权限点或数据库。
- 前端每 2 秒轮询，内存中保留最近 120 秒且最多 60 点；数据不持久化。
- API Server 与 Plugin Runner 以独立 `runtime_target` 返回，即使同一 `host_fingerprint` 也不合并监控序列。
- 速率由后端计算，前端只负责窗口和绘图；200 ms 内的并发请求复用最近样本。
- 页面隐藏、组件卸载或连续 3 次请求失败时停止采集；成功采样会清空连续失败计数。
- Linux 真容器或受限 cgroup 优先使用 cgroup v2；本地无资源限制的 systemd session 使用 host，cgroup CPU 不可读时回退 host。
- 不可用、预热或过期指标通过 `availability` 明确表达，不用数值 0 冒充。
- 图表使用 ECharts 原生宿主，不增加 wrapper；画布初始化一次，轮询时只更新 option。

## 截止日期

无固定截止日期；实现与自动化验证已于 2026-07-17 完成，提交等待用户视觉确认。

## 决策背后动机

优先解决“打开页面即可诊断”的产品问题，把实时性和复杂度控制在当前进程可承担的范围内；易失短窗口足以覆盖开发与容器排障，同时保留未来接入专门可观测平台的清晰边界。
