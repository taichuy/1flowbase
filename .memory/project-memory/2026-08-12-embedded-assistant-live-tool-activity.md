---
memory_type: project
topic: 嵌入式助手生成态实时工具活动
summary: 已在 dev 合入 runtime internal tool started/finished 实时事件和生成态“最新工具 + 累计数量”UI，等待用户人工验收。
keywords:
  - embedded assistant
  - assistant_tool_call_started
  - assistant_tool_call_finished
  - durable runtime event
  - live tool activity
created_at: 2026-08-12 22
updated_at: 2026-08-12 22
decision_policy: verify_before_decision
status: implemented_pending_user_acceptance
scope:
  - api/crates/orchestration-runtime/src/execution_engine.rs
  - api/crates/orchestration-runtime/src/execution_engine/llm_executor.rs
  - api/crates/control-plane/src/orchestration_runtime/live_debug_run/mod.rs
  - web/app/src/features/agent-flow/components/debug-console/conversation
integration_commit: d9545e08a
---

# 当前状态

Root agent 已在隔离 worktree 完成跨前后端实现、集中 Dev Acceptance QA，并于 `2026-08-12 22` 合并到 `dev`；下一步由用户进行人工界面验收。

# 为什么这样做

生成期间此前没有逐次发出 runtime internal tool 的 canonical started/finished 事件，前端只能在 terminal 后从 durable snapshot 看到工具详情。目标是在不新增内部轮次 contract 的前提下，让当前回答及时反馈工具执行进度。

# 已确认边界

- runtime 在工具真正开始和完成时，通过已有 `ExecutionLifecycle` 将 canonical 事件写入 durable runtime events，并同步到 live stream。
- 前端只基于事件里的 `tool_call_id` 去重统计，不从文本、DOM 或工具名猜测。
- 生成期间全局只显示一个工具区域，正文只显示最新工具，Badge 显示当前回答累计数量。
- terminal 后移除临时 Badge，继续展示原有完整工具详情。
- 不增加 `round_id`、`round_index`，本期范围是 runtime internal tools。

# 验收动机与停止条件

动机是消除“思考时工具区域为空、结束后才突然出现”的信息延迟。用户人工确认生成态时序、视觉位置和终态切换后，本阶段可关闭；若需要把外部 callback 统一纳入，应重新确认范围。
