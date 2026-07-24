---
memory_type: feedback
feedback_category: repository
topic: Gateway 工具兼容矩阵不等于工具执行所有权
summary: 在 AI gateway 兼容工作中，客户端与 Provider 专属工具默认是传输、流式、关联和终态测试向量；除非产品明确授权托管执行，否则不要把完整工具 inventory 扩张成 gateway execution domain、策略 UI 或本地 MCP/SSRF owner。
keywords:
  - gateway
  - tool calling
  - transport conformance
  - passthrough
  - execution owner
  - hosted tools
  - MCP
match_when:
  - 设计或修复 OpenAI Responses、Anthropic Messages、Chat Completions 工具兼容
  - 使用 Claude Code、Codex、OpenCode 的工具场景验收 gateway
  - 决定 hosted tool、client tool、MCP 的执行 owner
created_at: 2026-07-24 08
updated_at: 2026-07-24 08
last_verified_at: 2026-07-24 08
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/routes/application_public_api
  - api/crates/control-plane/src/application_public_api
  - api/crates/orchestration-runtime
  - api/crates/plugin-framework
  - scripts/node/ai-gateway-concurrency
---

# Gateway Tools Are Transport Fixtures, Not Execution Owners

## 时间

`2026-07-24 08`

## 规则

- 完整 Tool/Item/Event inventory 默认用于验证 gateway 是否无损接收、路由、实时转发、关联 call/result、保持 terminal 和正确处理 provider continuation。
- caller tool 由 Claude Code、Codex、OpenCode 等调用方执行；hosted/MCP list/call 由上游 Provider 执行；gateway 不因兼容这些 wire types 获得执行所有权。
- 同协议 provider 使用 transparent passthrough，允许 opaque future extensions；跨协议 semantic mapping 只覆盖证明等价的 subset，其他类型明确拒绝。
- 除非用户单独批准“托管工具执行平台”，不要把兼容缺陷扩张成 Hosted Policy UI、完整工具业务模型、local MCP/SSRF executor、OAuth/managed credential、费用或 approval ledger。

## 原因

用户指出，Codex、Claude Code、OpenCode 中大量工具本身是客户端或上游 Provider 的职责。用这些工具进行 tmux/mock 验收，是为了证明网关传输没有丢字段、乱映射、缓冲 SSE 或恢复错误 callback；不能据此推导 gateway 应执行这些工具。复杂度应留在能观察并控制工具生命周期的实际 owner。

## 适用场景

- 兼容 OpenAI Responses tools/tool choices/items/events。
- 设计 Chat/Anthropic/Responses 跨协议工具映射。
- 规划 provider-hosted web/file/code/image 或 MCP 支持。
- 使用真实 AI 编程客户端作为 gateway E2E consumer。
