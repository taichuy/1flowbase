---
memory_type: feedback
feedback_category: repository
topic: sse-streaming-is-event-driven-not-polling
summary: 1flowbase 流式输出默认沿用状态机与 RuntimeEventStream 订阅，不把定时产生 delta 的测试 fixture 或 SSE 实时转发表述为轮询；发现批量返回时先检查协议适配层是否绕过订阅并等待终态。
keywords:
  - SSE
  - RuntimeEventStream
  - state machine
  - polling
  - callback resume
  - delta
created_at: 2026-07-23 17
updated_at: 2026-07-23 17
last_verified_at: 2026-07-23 17
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/routes/application_public_api/compat_sse
  - api/apps/api-server/src/routes/application_public_api/anthropic.rs
  - api/crates/control-plane/src/orchestration_runtime
---

# SSE Streaming Is Event Driven, Not Polling

## 规则

诊断 SSE、首 token、工具回调恢复或 delta 批量返回时，默认以 `orchestration-runtime` 状态机和 `RuntimeEventStream` 事件订阅为既定架构。受控测试中主动 append 多个 delta 只是验证客户端能在终态前逐条收到，不代表生产代码增加轮询。

## 原因

用户纠正过：项目已从数据库轮询迁移到状态机驱动的运行事件流。把“按间隔产生 delta”描述为轮询，会混淆事件生产节奏与消费者查询机制。当前 Anthropic 工具恢复问题的证据是适配层等待 callback 完成后才包装 SSE，不是状态机或 RuntimeEventStream 退回轮询。

## 适用场景

- 设计或验收 Anthropic/OpenAI compatible SSE。
- 修复 callback resume、首 token 延迟或内容一次性出现。
- 编写流式测试 fixture、时间线断言或运行态诊断。
