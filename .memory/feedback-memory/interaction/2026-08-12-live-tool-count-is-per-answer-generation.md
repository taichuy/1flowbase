---
memory_type: feedback
feedback_category: product_contract
topic: 生成态工具数量按整次回答统计
summary: 嵌入式助手生成期间只显示一个最新工具区域，数量是当前回答生成内去重后的工具调用累计数；不要引入大语言模型内部 round_id 或 round_index。
keywords:
  - embedded assistant
  - live tool activity
  - tool count
  - answer generation
  - round identity
match_when:
  - 设计或实现助手生成期间的工具调用展示
  - 决定工具数量统计边界或是否新增轮次 contract
created_at: 2026-08-12 22
updated_at: 2026-08-12 22
decision_policy: direct_reference
status: confirmed
scope:
  - web/app/src/features/agent-flow/components/debug-console/conversation
  - api/crates/orchestration-runtime
  - api/crates/control-plane/src/orchestration_runtime
---

# 生成态工具统计边界

- 规则：一次用户消息触发的当前回答生成只展示一个工具区域；显示最近一次工具调用，并由前端按 `tool_call_id` 去重计算累计数量。
- 原因：用户关注的是当前回答整体进度，不是大语言模型内部 tool round；新增 `round_id / round_index` 会扩大 contract，且不服务当前交互。
- 适用场景：嵌入式助手、Debug Assistant Message、生成态工具提示、实时工具事件消费。
- 终态：回答结束、失败或取消后，临时数量标记消失，恢复 durable trace 的完整工具详情。
