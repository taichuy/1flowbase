---
memory_type: project
topic: 嵌入式助手生成态实时工具活动
summary: 已统一嵌入式助手实时与 durable activity 的 stream sequence，并让工具 WebSocket 事件携带轮次 call_usage、终态和历史侧栏从运行快照恢复完整 token 指标；等待用户人工验收。
keywords:
  - embedded assistant
  - assistant_tool_call_started
  - assistant_tool_call_finished
  - durable runtime event
  - live tool activity
created_at: 2026-08-12 22
updated_at: 2026-08-16 18
decision_policy: verify_before_decision
status: implemented_pending_user_acceptance
scope:
  - api/crates/orchestration-runtime/src/execution_engine.rs
  - api/crates/orchestration-runtime/src/execution_engine/llm_executor.rs
  - api/crates/control-plane/src/orchestration_runtime/live_debug_run/mod.rs
  - web/app/src/features/agent-flow/components/debug-console/conversation
integration_commit: f714f9bcd
---

# 当前状态

Root agent 已在隔离 worktree 完成跨前后端实现和集中 Dev Acceptance QA。`cf9a92f2c` 统一活动时序与思考生命周期，`f714f9bcd` 恢复工具轮次 token 的实时、终态和历史投影；下一步由用户进行人工界面验收。

# 为什么这样做

生成期间此前没有逐次发出 runtime internal tool 的 canonical started/finished 事件，前端只能在 terminal 后从 durable snapshot 看到工具详情。目标是在不新增内部轮次 contract 的前提下，让当前回答及时反馈工具执行进度。

# 已确认边界

- runtime 在工具真正开始和完成时，通过已有 `ExecutionLifecycle` 将 canonical 事件写入 durable runtime events，并同步到 live stream。
- 前端只基于事件里的 `tool_call_id` 去重统计，不从文本、DOM 或工具名猜测。
- 生成期间全局只显示一个工具区域，正文只显示最新工具，Badge 显示当前回答累计数量。
- terminal 后移除临时 Badge，继续展示原有完整工具详情。
- 不增加 `round_id`、`round_index`，本期范围是 runtime internal tools。
- durable activity 使用 `sequence_start` 定位活动、`sequence_end` 推进游标；可见工具或节点事件必须切断 reasoning/text 聚合，不能跨事件合并。
- 生命周期事件先取得 runtime stream sequence，再携带同一序号持久化；终态和历史回放不得改写实时顺序。
- 当前 reasoning 段自动展开；后续工具、输出或下一段 reasoning 到来时自动折叠。用户手动折叠后，同一段后续 delta 不得重新展开；终态默认全部收起。
- runtime internal tool 的 started / finished WebSocket 事件都携带与 `tool_call_id` 同轮次的 `call_usage`；前端不按事件邻近关系猜测 usage 归属。
- 生成态节点侧栏消费 WebSocket 增量；终态和历史侧栏打开时使用现有运行快照 `node_runs[]` 恢复完整节点 metrics、工具回调摘要与 token。

# 验收动机与停止条件

动机是消除“思考时工具区域为空、结束后才突然出现”的信息延迟。用户人工确认生成态时序、工具行 token、视觉位置、终态切换和历史恢复后，本阶段可关闭；若需要把外部 callback 统一纳入，应重新确认范围。
