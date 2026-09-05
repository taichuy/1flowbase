---
memory_type: project
topic: Frontstage BlockContext 后端静态契约与 MCP 查询
summary: 用户确认并完成平衡方案：API Server 内嵌 resources/ctx 版本化契约，通过认证只读接口和 frontstage_assistant MCP Tool 查询，并用测试约束前端 BlockContext keys 与 SDK version 漂移。
keywords:
  - BlockContext
  - ctx
  - Frontstage
  - MCP
  - static contract
created_at: 2026-09-05 00
updated_at: 2026-09-05 00
last_verified_at: 2026-09-05 00
decision_policy: verify_before_decision
status: dev-acceptance
scope:
  - api/apps/api-server/resources/ctx
  - api/apps/api-server/src/routes/frontstage/block_context_contract.rs
  - api/apps/api-server/resources/mcp/frontstage-assistant.json
---

# Frontstage BlockContext 后端静态契约与 MCP 查询

- 谁在做什么：Root agent 已将 17 项 `ctx.*` 作者契约写入 API Server 静态资源，注册认证只读接口，并把同一接口绑定到内置 Frontstage MCP Bundle。
- 为什么这样做：此前完整 `BlockContext` 只存在于前端 TypeScript，后端和 MCP 无法稳定查询字段、类型与成员说明。
- 为什么要做：让 GUI、Agent 和后端 interface catalog 共享可发现契约，同时避免 MCP 再硬编码第三份 ctx 清单。
- 截止日期：2026-09-05 已完成 Dev Acceptance；等待用户验收。
- 决策动机：静态资源编译时嵌入、接口注册时强校验，请求路径不读取磁盘；`non_context_symbols` 只表示不由 `ctx` 注入，不把允许直接使用的浏览器全局误报为禁止能力。
