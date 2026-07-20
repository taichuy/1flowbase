---
memory_type: project
topic: MCP 创建 Workflow 并发布 API 后缺少安全的公开 operation 转 Tool 边界
summary: 已通过 MCP 创建并发布 MCP Workflow Echo；动态注册正常，但 MCP 发现、认证与输入契约暴露出 Workflow 仍承袭过多 AgentFlow 语义。用户已确认长期演进采用 Application 共享壳、AgentFlow / Workflow bounded product contract、共享执行节点默认开放、专属边界节点显式归属的方向；完整 Root → Delivery 方案待最终批准。
keywords:
  - MCP
  - workflow
  - application public API
  - interface catalog
  - unsupported_mcp_interface_scope
  - application_operation
  - mcp-workflow-echo-20260720
match_when:
  - 继续把已发布 Workflow API 转换为 MCP Tool
  - 设计应用公开 API 进入 MCP interface catalog 的认证与所有权
created_at: 2026-07-20 12
updated_at: 2026-07-20 15
last_verified_at: 2026-07-20 15
decision_policy: verify_before_decision
scope:
  - application 019f7def-ef16-7610-a553-dfd87fe1e8ed
  - /api/ex/mcp-workflow-echo-20260720
  - api/apps/api-server/src/routes/settings/mcp_management.rs
  - api/crates/control-plane/src/application_public_api
---

# MCP Workflow Echo Public API Gap

## 当前状态

- 已通过 MCP 创建 Workflow 应用 `MCP Workflow Echo`，application id 为 `019f7def-ef16-7610-a553-dfd87fe1e8ed`。
- Workflow 使用 `workflow_start -> template_transform -> workflow_end`，必填 body 参数 `message` 原样返回。
- 草稿已保存并发布；publication id 为 `019f7df1-3cbe-7352-bd16-3a5965cf703e`，公开 URL 为 `/api/ex/mcp-workflow-echo-20260720`，`active=true`、`api_enabled=true`。
- 已通过 MCP 创建并挂载 Workflow 创建、编排读取/保存、API mapping、发布和 API docs 查询 Tool。

## 阻塞证据与纠正

- 纠正旧结论：发布后的 `/api/ex/{slug}` 已由 `dynamic_openapi` 注册进全局 `/openapi.json`，不是“没有注册接口”。
- MCP interface catalog 只装配 static API docs 与 runtime data model CRUD，因此没有消费已经存在于动态 OpenAPI 的 `/api/ex/{slug}` operation。
- 静态 Native 路由 `POST /api/agent/v1/runs` 可被发现，但 `bindable=false`，`disabled_reason=unsupported_mcp_interface_scope`。
- 当前 `/api/ex/{slug}` route 和 service 仍强制解析并认证 Application API Key，并校验 Key 绑定的 application 与 slug publication 相同；这与用户记忆中的“只有 AgentFlow 才需要每应用 Key”不一致，需要产品决策。
- MCP interface wrapper 会原样转发 MCP User API Key 的 Authorization header；即使补齐动态 catalog，也会被 `/api/ex` 当成错误的 Application API Key 拒绝。
- 已批准的 Workflow contract 要求 HTTP source 直接来自 `workflow_start.config.input_fields`、删除 target selector；当前运行链仍从 `publication.mapping_snapshot.extension.parameters` 读取 `target` 并写 selector。仓库已有 `workflow_start_http_inputs` 解析器且明确拒绝 target，但尚未接入 `WorkflowExtensionRunService`。在转 MCP Tool 前应先统一这份参数真值，不能继续固化第二套 mapping。

## 当前需求边界（2026-07-20 15）

- 用户明确把 Workflow 视为仍在开发中的新产品能力，当前覆盖 `/api/ex/*` 扩展接口触发与定时触发。
- 用户判断初期实现从 AgentFlow 复制过多，已经造成产品 contract、认证、发布和运行语义边界模糊；后续应作为长期架构演进处理，而不是只修 MCP catalog 或 bearer 转发。
- 本轮只做 `problem-framing`。在用户确认架构方向前，不修改产品代码、数据库、schema 或运行时行为。
- 当前建议尚未获批；确认前只把本节作为需求范围与动机，不作为实现决策。
- 用户已确认节点注册规则：通用执行节点默认共享，产品专属节点显式绑定 application type，trigger-specific 只作为产品专属节点中的更小例外；不为每个共享节点重复声明产品适用范围。
- 完整架构方案正在收敛为新的两层 Issue Tree；在 Root 正文获批前不执行源码、schema、migration 或运行时修改。

## 待确认方向

优先确认 `/api/ex/{slug}` 的认证真值：若发布即公开且无需 Application API Key，则修复 route/service 认证并让 MCP catalog 复用动态 OpenAPI operation；若仍需鉴权，则应把 Workflow Extension 的认证策略显式建模，不能继续借 AgentFlow Application API Key 隐式承担路由与授权两种职责。无论选择哪种认证，HTTP 参数 Schema、运行映射和 MCP Tool Schema 都应统一从 `workflow_start.input_fields` 派生。
