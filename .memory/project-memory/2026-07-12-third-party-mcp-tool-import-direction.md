---
memory_type: project
topic: 第三方 MCP Tool 导入与本地封装方向
summary: 第三方 MCP Tool 作为可执行远程接口来源接入；本地 MCP Tool 是最终 contract，独立维护描述、Schema、输入输出映射、权限、风险和挂载配置。
keywords:
  - mcp-management
  - third-party-mcp
  - remote-mcp-tool
  - schema-mapping
  - oauth
match_when:
  - 实现或调整第三方 MCP 连接、授权、tools/list、tools/call 或 Tool 导入
  - 调整本地 Tool 与远程 Tool 的 Schema、描述、映射或来源关系
created_at: 2026-07-12 00
updated_at: 2026-07-12 00
last_verified_at: 2026-07-12 00
decision_policy: verify_before_decision
scope:
  - web/app/src/features/settings/components/mcp-management
  - web/packages/api-client/src/console-mcp-management.ts
  - api/apps/api-server/src/routes/settings/mcp_management.rs
  - api/crates/control-plane/src/mcp_management.rs
  - api/crates/domain/src/mcp_management.rs
  - api/crates/storage-durable/postgres/src/mcp_management_repository.rs
---

# 第三方 MCP Tool 导入与本地封装方向

## 时间

`2026-07-12 00`

## 谁在做什么

用户批准在 MCP 管理增加“第三方 MCP”Tab：用户先连接并授权第三方 MCP，后端获取远程完整 Tool 清单，用户选择后导入现有本地 Tool 管理体系。已创建 GitHub Issue #1246。

## 为什么这样做

第三方 MCP Tool 本身是远程执行接口，不应只复制 Schema 后绑定无关接口，也不应绕过本地工具治理直接暴露。后端负责授权和远程协议，本地 Tool 负责稳定的模型与运行时 contract。

## 为什么要做

需要同时获得第三方 MCP 生态接入能力，以及现有 Tool 的描述、Schema、映射、权限、风险和挂载治理能力。

## 截止日期

无固定截止日期；Issue #1246 实现和后续扩展持续遵守。

## 决策背后动机

- 远程输入输出 Schema 与本地输入输出 Schema 分离，通过 `input_mapping` / `output_mapping` 连接。
- 第三方描述初始化本地 `short_description`，同时保留远程原始描述用于变化识别。
- 导入创建本地未启用 Tool 草稿，用户在 Tools Tab 完成配置后再挂载。
- 远程刷新只提示变化，不自动覆盖本地修改。
- 远程连接、凭据、`tools/list` 和 `tools/call` 由后端作为唯一数据来源管理。

## 关联文档

- GitHub Issue #1246
