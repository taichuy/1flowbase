---
memory_type: feedback
feedback_category: repository
topic: runtime must not impose tool round limits
summary: 大语言模型工具调用的继续与中断由模型、客户端和用户控制；1flowbase 网关/运行时不得用固定轮数对长任务做用户无感知阻断。
keywords:
  - tool rounds
  - runtime internal tools
  - visible internal llm tool
  - gateway boundary
  - user cancellation
created_at: 2026-08-12 21
updated_at: 2026-08-12 21
last_verified_at: 2026-08-12 21
decision_policy: direct_reference
scope:
  - api/crates/orchestration-runtime/src/execution_engine/llm_executor.rs
  - api/crates/orchestration-runtime/src/execution_engine/visible_internal_llm_tools.rs
  - api/apps/api-server/src/routes/assistant/websocket.rs
---

# Runtime Must Not Impose Tool Round Limits

## 规则

大语言模型重复或连续调用工具属于客户端 / 模型执行语义。1flowbase 网关和编排运行时不得用固定调用轮数、相同调用检测或其他用户无感知策略中断任务。用户需要停止时使用现有 run cancellation 生命周期。

## 原因

长任务正常需要多轮 discovery 与工具调用，重复调用也不足以证明任务失控；宿主无法仅凭轮数区分合法探索和异常循环。固定上限会把正常任务错误结算为失败，并丢掉最后一次成功工具结果。

## 适用场景

- MCP、Assistant client tools、runtime internal tools 的 inline callback 循环。
- `visible_internal_llm_tool` 的多轮主模型 recall。
- 设计网关层 timeout、budget、loop detection 或隐式 circuit breaker 时。
