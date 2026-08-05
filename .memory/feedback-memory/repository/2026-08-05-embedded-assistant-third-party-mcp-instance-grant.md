---
memory_type: feedback
feedback_category: repository
topic: 内置助手挂载第三方 MCP 时以实例接入作为默认调用授权
summary: 用户确认当前阶段不为第三方 MCP 做按角色或按工具的默认区分；已启用且被内置助手挂载的第三方 MCP 实例对该助手可用。当前用户角色仍约束 1flowbase 后端接口；第三方 MCP 的调用资格由实例接入和挂载配置决定。
keywords:
  - embedded assistant
  - third-party MCP
  - MCP proxy
  - instance mount
  - authorization
match_when:
  - 设计内置 AI 助手的 MCP 挂载与执行链路
  - 判断第三方 MCP Proxy 是否需要逐工具或逐角色授权
  - 区分 1flowbase 后端接口权限与第三方 MCP 实例接入权限
created_at: 2026-08-05 09
updated_at: 2026-08-05 09
last_verified_at: 2026-08-05 09
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/routes/mcp_protocol.rs
  - api/crates/control-plane/src/mcp_management.rs
---

# Embedded Assistant Third-Party MCP Instance Grant

## 规则

内置助手挂载第三方 MCP 时，不默认增加按角色、按工具的拒绝或授权层。只要第三方 MCP 实例已接入、启用并被该助手配置挂载，即允许助手调用其已暴露的工具。

当前用户角色仍用于 1flowbase 自身后端接口的授权；不得把这一规则误读为放宽本地接口的权限检查。第三方 MCP 的上游凭据与实例可用性仍由 MCP 实例配置负责，终端用户不需要创建或持有 API key。

## 原因

用户明确说明当前阶段不打算对第三方 MCP 做默认权限区分：能够接入并挂载该实例即表示可以调用。

## 适用场景

内置聊天助手、第三方 MCP Proxy、MCP 实例挂载、用户角色授权边界。
