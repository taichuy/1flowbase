---
memory_type: project
topic: provider reasoning effort request-log projection
summary: 用户于 2026-08-04 20 确认只修复 Provider attempt 的推理强度日志投影；运行时从最终 Provider model_parameters 记录 reasoning_effort，不改 Provider 翻译、不由前端补偿、不回填缺少可靠来源的历史日志，也不保存原始请求体。
keywords:
  - model-provider-request-logs
  - reasoning-effort
  - attempt-metrics
  - runtime-projection
  - no-history-backfill
match_when:
  - 调整模型供应商请求日志的推理参数投影
  - 判断 reasoning_effort 应来自入站请求、运行时还是 Provider wire
created_at: 2026-08-04 20
updated_at: 2026-08-04 20
last_verified_at: 2026-08-04 20
decision_policy: verify_before_decision
scope:
  - api/crates/orchestration-runtime/src/execution_engine
  - api/crates/control-plane/src/orchestration_runtime/persistence
  - model_provider_request_logs.reasoning_effort
---

# Provider 推理强度日志投影

## 谁在做什么

用户确认由 1flowbase runtime 在 Provider attempt 形成时，把最终生效 `model_parameters` 中的推理强度写入请求日志投影；AI 已按 Single Issue 完成局部实现与定向验证。

## 为什么这样做

入站 Anthropic `adaptive/high` 已正确进入 AI Native 并由 OpenAI-Compatible Provider 转成 `reasoning_effort: high`，但 attempt metric 没有携带该字段，导致请求日志表和页面始终显示空值。修复目标是恢复可观测性，不改变上游协议行为。

## 当前已确认决策

- 日志值来自最终 Provider invocation `model_parameters`；显式 `reasoning_effort` 优先，否则读取 typed `reasoning.effort`。
- 未构造 Provider invocation 或未设置推理强度时保持空值。
- 不修改 Provider 插件翻译，不让前端推断或兼容输出。
- 不保存原始 Provider HTTP 请求体，不对无法可靠恢复的历史日志做回填。

## 截止日期

本决策持续有效；若未来需要记录 mode、budget、Provider 翻译决策或精确 wire audit，必须重新进入需求对齐，而不是扩大当前字符串列语义。
