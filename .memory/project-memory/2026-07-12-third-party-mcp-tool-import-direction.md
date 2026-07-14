---
memory_type: project
topic: 第三方 MCP Tool 导入与本地封装方向
summary: 第三方 MCP Tool 通过 HTTPS Streamable HTTP 接入；本地 Tool 以 interface_wrapper / mcp_proxy 区分执行目标，MCP 代理支持双向字段映射并统一复用实例 Binding。
keywords:
  - mcp-management
  - third-party-mcp
  - remote-mcp-tool
  - schema-mapping
  - streamable-http
  - execution-target
  - field-mapping
match_when:
  - 实现或调整第三方 MCP 连接、授权、tools/list、tools/call 或 Tool 导入
  - 调整本地 Tool 与远程 Tool 的 Schema、描述、映射或来源关系
created_at: 2026-07-12 00
updated_at: 2026-07-14 20
last_verified_at: 2026-07-14 20
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

`2026-07-14 20`

## 谁在做什么

用户批准并已实现 MCP 管理文案精确为“第三方MCP”的 Tab：用户先连接第三方 MCP，后端获取远程完整 Tool 清单，用户选择后导入现有本地 Tool 管理体系。GitHub Issue #1246 已完成开发与独立 QA blocker 修复，进入 QA / 用户验收阶段；最终账本为 17 green、0 red，桌面/移动运行态、PostgreSQL migration harness 和代理 Binding 端到端仍需后续证据。

## 为什么这样做

第三方 MCP Tool 本身是远程执行接口，不应只复制 Schema 后绑定无关接口，也不应绕过本地工具治理直接暴露。后端负责授权和远程协议，本地 Tool 负责稳定的模型与运行时 contract。

## 为什么要做

需要同时获得第三方 MCP 生态接入能力，以及现有 Tool 的描述、Schema、映射、权限、风险和挂载治理能力。

## 截止日期

无固定截止日期；Issue #1246 实现和后续扩展持续遵守。

## 决策背后动机

- 远程输入输出 Schema 与本地输入输出 Schema 分离，通过 `input_mapping` / `output_mapping` 连接。
- Tool 执行目标显式区分 `interface_wrapper` 与 `mcp_proxy`；前者映射 HTTP path/query/body，后者映射本地 arguments 到远端 arguments，并把远端 `structuredContent` 映射为本地结构化结果。
- MCP 代理保留第三方 `CallToolResult.content` / `isError` 协议语义；无结构化结果时不猜测文本 JSON。
- 首版生产 transport 为 HTTPS Streamable HTTP，认证只支持 `none`、`bearer`、`custom_header`；OAuth、SSE、stdio 不在首版范围。
- 首版字段映射支持嵌套路径、重命名、筛选和必填校验，不引入脚本、表达式或通用转换语言。
- 第三方描述初始化本地 `short_description`，同时保留远程原始描述用于变化识别。
- 导入创建本地 `draft + high` Tool，用户在 Tool 配置完成字段映射和风险确认后，通过现有 Binding 统一挂载到 MCP 实例。
- 远程刷新只提示变化，不自动覆盖本地修改。
- 远程连接、凭据、`tools/list` 和 `tools/call` 由后端作为唯一数据来源管理。

## 关联文档

- GitHub Issue #1246：https://github.com/taichuy/1flowbase/issues/1246
