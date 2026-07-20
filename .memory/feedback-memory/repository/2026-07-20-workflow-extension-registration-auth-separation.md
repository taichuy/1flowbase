---
memory_type: feedback
feedback_category: repository
topic: Workflow Extension 的动态注册、MCP 发现与调用认证必须分开判断
summary: 排查 /api/ex/* 时必须分开判断动态注册、MCP 发现与调用认证；同时区分作为入站入口的 Workflow HTTP trigger 和作为共享执行能力的 http_request 节点。Workflow 只专有 Start/End 等产品边界节点，HTTP、Code、数据查询等通用执行节点继续跨应用类型共享。
keywords:
  - workflow extension
  - api/ex
  - dynamic OpenAPI
  - MCP interface catalog
  - Application API Key
  - registration
  - authentication
  - http_request
  - shared execution nodes
  - product boundary nodes
match_when:
  - 诊断 /api/ex/{slug} 发布后是否已注册
  - 将 Workflow Extension operation 转为 MCP Tool
  - 判断 Workflow 与 AgentFlow 是否共用 Application API Key
  - 判断 Workflow 节点与 AgentFlow 节点的专属或共享边界
created_at: 2026-07-20 12
updated_at: 2026-07-20 15
last_verified_at: 2026-07-20 15
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/openapi/mod.rs
  - api/apps/api-server/src/routes/application_public_api/ex.rs
  - api/apps/api-server/src/routes/settings/mcp_management.rs
  - api/crates/control-plane/src/application_public_api/workflow_extension.rs
---

# Workflow Extension Registration And Authentication Separation

## 规则

判断 Workflow Extension 能否转 MCP Tool 时，固定拆成四层：

1. publication 是否激活 slug；
2. dynamic `/openapi.json` 是否注册具体 `/api/ex/{slug}` operation；
3. MCP interface catalog 是否消费该动态 operation；
4. MCP 调用身份是否符合 `/api/ex` 的认证 contract。

术语和节点边界同时固定为：`Workflow HTTP trigger` 是从 `/api/ex/*` 进入 Workflow 的外部触发入口，不是画布里的 `http_request` 节点；`workflow_start/workflow_end` 等产品边界节点归 Workflow，`http_request`、`code`、数据查询等只依赖显式输入输出的执行节点归共享编排能力层，不复制为 Workflow 专属节点。

不得因为第 3 层缺失就否定第 2 层，也不得因为 slug 已定位 application 就自动推导“无需鉴权”。AgentFlow 的共享 path 依赖 Application API Key 定位应用；Workflow Extension 的 slug 已能定位 publication，认证是否继续使用应用 Key必须单独由产品语义决定。

## 原因

用户纠正了“发布后的 Workflow Extension 没有注册成接口”的推断。源码和 route test 证明 `dynamic_openapi` 会把启用 publication 的具体 slug 注册进全局 OpenAPI；真正缺口是 MCP catalog 仍使用静态 registry。同时当前 route/service 仍强制 Application API Key，因此“注册”与“认证”是两个独立问题。

## 适用场景

Workflow API 发布、动态 OpenAPI、MCP Tool 转换、Application API Key、Webhook/API trigger 或 `/api/ex/*` 故障诊断。
