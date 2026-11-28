---
title: "负载监控（/settings/compute-monitor）打开与采集性能平衡优化方案已完成验收与线上打包"
memory_type: project
created_at: "2026-11-22 18"
updated_at: "2026-11-22 20"
decision_policy: verify_before_decision
status: settled
tags: [compute-monitor, ssh, shared-stream, record-permissions, performance]
---

# 负载监控（`/settings/compute-monitor`）打开与采集性能平衡优化方案

- **谁做了什么**：主控 Agent 组织并指挥远端 `taichuy-dev` 上的 Codex（`gpt-6.2-sol` / `xhigh` / `danger-full-access`），协同完成 `1flowbase` 底座（`dev` 提交 `0325e237ae4df29b4d7f533c7ea30e7c8983a8ba`）与商业插件仓 `1flowbase-plugins-business`（`codex/plugin-resource-business` 提交 `31ca85fc40d451dfd59ded44aef412441cc5f60c`，含 `@taichuy/ssh@0.1.26` 与 `@IQLater/compute-monitor@0.2.9`）的打开与采集性能优化，通过两轮独立集中 QA（`Central QA #2` `VERDICT: QA_PASS`）并在 GitHub Actions `provider-ci`（Run `45023597992`）完成线上打包。
- **为什么这样做（核心根因与优化决策）**：
  1. **底座通用能力收敛**：`read_resource_authority` 按 `SYSTEM_SCOPE_ID` 缩小为单目标安装 + `active_requested` 最小依赖投影，且只读执行准入（`execute_snapshot_duplex` / `execute_hook` / `authorize_snapshot_binding` / `managed_ephemeral`）改走 `SharedRead` 共享锁与单安装查询，消除每次调用扫描 `243` 行 `3.78 MB metadata_json` 与并发串行锁竞争；`roles.record_capabilities.get` 批量合并记录存在性与策略查询；`shared_stream` 预编译事件 Schema、`buffer_unordered(16)` 渐进并发准入并无损保留冷启动 `shared-latest` 种子快照回放；`PluginSettingsPage.tsx` 增加 16 项有界 LRU 编译缓存。
  2. **插件职责内提速**：`@taichuy/ssh@0.1.26` 在入口跳认证完成后立即释放 `ExecutionGate` 并提供单通道受控行流；`@IQLater/compute-monitor@0.2.9` 采用单连接常驻 `python3 -u` 流式采样 + `250ms` 双点引导差分速率计算，`hosts.list` 开启 `execution_concurrency: parallel`，`ui/settings.tsx` 引用计数合并在途请求消除 `<React.StrictMode>` 重复请求与 `ERR_ABORTED`，`ui/streams.ts` 保持热回放在线状态徽标不回退闪烁。
- **实测效果（11 台真实 SSH 主机，含 6 台共用跳板节点）**：
  - 首屏 `GET /hosts`：从重复 2 次、`593–1147 ms` 降至 **1 次、0 abort、`66.5 ms`**；
  - `POST /record-capabilities`：从 `2207–2590 ms` 降至 **`106–115 ms`**；
  - 首屏建流 `WS 101`：从 `5636–6156 ms` 降至 **`2105.5 ms`**；
  - 11 台机器冷开首个完整速率样本（`warming_up: false`）：从 `38.3–40.5 s` 降至 **`3.68 s`**（`250 ms` 双点差分生效，直连节点不再被慢跳板队头阻塞）；
  - 同会话 SPA 切页重开：挂载 **`109 ms`**（0 Worker、无 `<Spin />`），`WS 101` 后热快照回放 **`633–813 ms`**，11 台主机与 17 条凭据数据摘要逐字节无损。
- **发布边界提醒**：当前 `@taichuy/ssh@0.1.26` 与 `@IQLater/compute-monitor@0.2.9` 基于 `#2896` 资源导入契约推送在 `codex/plugin-resource-business` 并完成 `provider-ci` 线上打包产物留存；待底座 `#2896` 合入生产主干后再合入商业插件仓 `main` 发布正式目录。
- **截止日期**：`2026-12-22 23`（随 `#2896` 合入生产主线后复核归档）。
