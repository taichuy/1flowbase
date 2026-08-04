---
memory_type: feedback
feedback_category: repository
topic: Agent 操作层沿用 API Key 与后端权限且不依赖 GUI
summary: 系统 Agent 操作层由 Agent 独立完成，不设计 GUI 共用入口，也不另建 Agent principal、delegation grant 或审批权限域；身份与授权继续以 API Key、角色和后端 operation/resource 校验为唯一真值。
keywords:
  - agent operation layer
  - MCP
  - API key
  - permission
  - headless
  - interface catalog
match_when:
  - 设计系统 Agent 操作层或 MCP 管理能力
  - 讨论 Agent 与人类权限、托管、审批或身份模型
  - 把后端领域操作暴露给 MCP
created_at: 2026-08-03 12
updated_at: 2026-08-03 12
last_verified_at: 2026-08-03 12
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/routes/mcp_protocol.rs
  - api/apps/api-server/src/routes/settings/mcp_management
  - api/crates/control-plane/src/mcp_bundle.rs
  - MCP Virtual UI
---

# Agent 操作层沿用现有授权边界

## 规则

- Agent 操作层是无 GUI 的独立自主调用面，不要求 GUI 与 Agent 共用操作流程。
- MCP 请求已经由 API Key 解析到用户、角色和工作区；后端 operation、资源范围与领域状态校验继续作为唯一授权真值。
- 不为 Agent 操作层另建 `Agent principal`、`delegation grant` 或人工审批中心，除非用户后续明确改变授权模型。
- 新能力应由后端领域接口承载并注册进 interface catalog，再配置为 MCP Tool；MCP Binding 只负责发现，不承担授权。

## 原因

用户明确系统 Agent 在该层独立完成操作，现有 API Key、角色和后端权限已经绑定完整。新增 GUI 或平行授权域会重复建模并扩散权限复杂度。

## 适用场景

远程制品拉取与导入、MCP Bundle 更新、插件管理、数据建模发布，以及其他需要 Agent 自主完成的控制面操作。
