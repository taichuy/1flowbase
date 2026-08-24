---
memory_type: project
topic: Network egress provider current-version activation
summary: Network Egress 代理实例绑定稳定插件家族，不绑定带版本的 installation；运行节点必须把家族解析为本节点 current artifact。
keywords:
  - network-egress
  - provider family
  - is_current
  - runtime activation
  - plugin update
match_when:
  - 修改网络代理插件安装、版本选择、实例绑定或 worker 生命周期
  - 排查界面版本与实际进程版本不一致
created_at: 2026-08-23 17
updated_at: 2026-08-24 18
last_verified_at: 2026-08-24 18
decision_policy: verify_before_decision
status: active
scope:
  - api/crates/control-plane/src/network_egress.rs
  - api/crates/storage-durable/postgres/src/plugin_repository.rs
  - api/apps/api-server/src/routes/network_center/plugins.rs
  - api/apps/api-server/src/provider_runtime/mod.rs
  - api/apps/plugin-runner/src/network_egress_host.rs
---

# Network Egress 当前版本必须闭合到运行态

- 谁在做什么：用户要求 Network Center 中安装或切换为 current 的代理插件版本，必须成为既有代理实例后续实际启动的版本；AI 已确认当前实现只切换 node artifact 的 `is_current`，既有实例仍固定旧 `installation_id`，旧 worker 也不会因家族切换自动退出。
- 为什么这样做：界面只展示一个 current 版本时，它就是用户可观察的运行版本承诺；目录真值与实际进程真值分裂会让安全修复、资源限制和行为修复无法生效。
- 为什么要做：2026-08-24 运行态已复现界面 current 为 `0.2.8`，数据库供应方绑定 `0.2.5`，实际进程也从 `0.2.5/bin` 启动；这不是插件协议故障，而是版本激活 contract 缺失。
- 截止日期：2026-08-24 完成问题对齐与取证；实现方向尚待用户确认，进入开发时必须同时覆盖版本真值、历史实例、worker drain/unload 和失败回滚。

2026-08-24 用户进一步确认：不能通过升级时批量改写实例 `installation_id` 修复，因为这仍然把长期代理配置与短期版本制品耦合。目标边界改为：实例持久化稳定家族身份，运行时按节点解析 current artifact，worker 按 provider instance 与 artifact generation 隔离。

旧阶段语义“安装新版本只影响新建代理，既有实例继续固定旧版本”自 `2026-08-24 17` 起 superseded，不再作为有效设计依据。

2026-08-24 18 实现与 Dev Acceptance 完成：provider 持久化稳定 `category + organization + artifact_id` 家族，运行节点定向解析 current artifact；worker 以 `provider_id + artifact generation` 隔离，host lease 精确映射到 generation，旧 generation 在最后 lease 释放后退出；版本激活先按每个 provider 的独立 secret 做 target preflight，失败不改变旧 current。历史 `installation_id` 列仅作为 staged migration 的 legacy pointer 保留，运行时和新写入均不再使用。

定向证据：稳定家族持久化/current 解析、preflight 失败保持旧 current、跨 publisher current 隔离、provider worker/secret 隔离、generation drain、plugin-runner 6 条运行时回归、API client 244 条测试与构建、Rust backend check 均通过。开发主库尚未手工执行新 migration；部署新版 API 重启时由 migration runner 执行，随后仍需用实际 PID/命令行确认进程来自 current version 的 `bin` 路径。
