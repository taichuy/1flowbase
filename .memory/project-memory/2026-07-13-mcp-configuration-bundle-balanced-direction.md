---
memory_type: project
topic: MCP 配置包组织、整套导入与版本提示平衡方案
summary: 用户批准在 official plugins 仓库按组织维护 MCP 配置包，Tool 先导入、实例目录后组装；单项接口缺失标记不可用并继续，同时记录配置包版本与导出系统版本，旧系统导出的包导入新系统时仅警告不阻断。
keywords:
  - mcp-management
  - mcp-bundle
  - official-plugins
  - import-export
  - system-version
match_when:
  - 设计或实现 MCP 配置包目录、manifest、catalog、导入导出或版本兼容提示
  - 调整 MCP Tool 批量导入、实例路径组合、部分成功或不可用状态
created_at: 2026-07-13 10
updated_at: 2026-07-13 10
last_verified_at: 2026-07-13 10
decision_policy: verify_before_decision
scope:
  - /home/taichuy/git/1flowbase-official-plugins/mcp
  - web/app/src/features/settings/components/mcp-management
  - web/packages/api-client/src/console-mcp-management.ts
  - api/apps/api-server/src/routes/settings/mcp_management.rs
  - api/crates/control-plane/src/mcp_management.rs
  - api/crates/domain/src/mcp_management.rs
---

# MCP 配置包平衡方案

## 谁在做什么

用户批准 MCP 配置包平衡方案：在 `1flowbase-official-plugins/mcp/<organization>/<bundle_id>/` 下按 `tools/` 与 `instances/` 维护配置源码，通过 manifest、catalog 和单一归档包完成整套下载导入。

## 为什么这样做

MCP Tool 是独立能力定义，实例负责路径、分组和 Tool Binding。导入必须先处理 Tool，再组装实例目录；目标环境缺少 Tool 的 `interface_id` 时，该 Tool 标记为不可用并继续后续项，不能破坏整套导入。

## 为什么要做

需要让不同组织维护可移植的 MCP 配置套件，并支持一键分发、部分成功、稳定语义引用和跨系统版本风险提示。

## 截止日期

无固定截止日期；进入 issue 或实现后持续遵守。

## 决策背后动机

- 配置包使用稳定 `tool_id` / `instance_id` 引用，不携带 workspace UUID、记录 UUID 或审计字段作为导入 contract。
- 配置包维护独立包版本和格式 schema 版本。
- 导出由后端自动记录当前导出系统版本。
- 当配置包记录的导出系统版本低于当前导入系统版本时，导入前弹窗警告，但用户确认后仍可继续，不作为阻断条件。
- manifest、checksum 或不支持的 schema version 仍属于结构性错误，可在写入前拒绝。
- 同名本地资源默认不静默覆盖；具体冲突策略在 implementation handoff 中保持显式。

## 线上 Issue

- GitHub Issue #1253：`[待实现] MCP 配置包按组织分发、整套导入与版本兼容提示`
- 地址：https://github.com/taichuy/1flowbase/issues/1253
- 当前为 `level:standalone`、`grade:g3`、`phase:ready`。
