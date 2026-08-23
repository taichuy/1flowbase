---
memory_type: project
topic: 失败会话轮次保留用户输入并闭合中断工具调用
summary: Issue #1849 已实现：Native history 按消息事实回放失败轮 user query，并在 Provider-bound projection 中闭合完整但无 output 的 tool call。
keywords:
  - assistant conversation
  - failed run
  - native history
  - tool interruption
  - replay policy
  - issue 1849
match_when:
  - 实现或验收 GitHub Issue #1849
  - 修改 assistant conversation native history 的失败轮准入
  - 处理完整 tool call 缺少 output 的下一轮回放
created_at: 2026-08-23 00
updated_at: 2026-08-23 09
last_verified_at: 2026-08-23 09
decision_policy: verify_before_decision
source_issue: "#1849"
status: implemented_local
scope:
  - api/crates/storage-durable/postgres/src/orchestration_runtime_repository/application_run_logs
  - api/crates/control-plane/src/application_public_api
---

# 失败会话轮次保留用户输入并闭合中断工具调用

## 谁在做什么

- 用户确认把该问题作为一个小缺陷挂到线上，不扩成 Issue Tree。
- AI 已完成本地实现和定向验证，尚未提交或推送。

## 为什么这样做

当前 native history 用整轮 `run.status = succeeded` 作为消息准入条件，导致已经接受、持久化且用户可见的 user message 在 Provider 超时或失败后从下一轮上下文消失。工具调用若在回调中断后留下完整 call 而没有 output，还可能造成协议拒绝或重复副作用。

## 已确认方向

- run status 只描述 execution；会话消息是否存在按消息持久化事实判断。
- 失败、超时或取消后的 persisted user query 仍可回放。
- 没有首 token 时不创建 assistant message，也不把错误文案伪装成 assistant content。
- 已持久化完整 tool call 缺 output 时，在 Provider-bound history 中做 interrupted/aborted 最小闭合；不自动重试可能有副作用的工具。
- 不建设 per-delta journal、断点续传、全局 conversation registry 或 Provider continuation。

## 截止日期与停止条件

- 截止日期：用户未指定。
- 若修复需要 migration、历史重写、per-delta durable journal，或改变 #1743 reasoning contract，则退回 `problem-framing`，不得扩大 #1849。

## 实现与证据

- `list_assistant_conversation_native_history` 不再按整轮 `succeeded` 过滤 user query；failed run 只有已持久化 Native assistant message 才回放 assistant，避免把 error payload 伪造成 assistant content。
- API server 统一使用 `assistant_conversation_native_history_to_values`：完整 `id/name/arguments` tool call 获得同 `tool_call_id`、`is_error=true` 的 interrupted output；残缺 call 与孤儿 output 不进入 Provider history，不触发自动重试。
- 定向证据：storage-postgres 两个 repository/projection 集成测试通过；control-plane tool closure 单测通过；`cargo check -p api-server` 通过。
