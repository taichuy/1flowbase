---
memory_type: project
topic: Root #2079 AI Native 工具交付提交边界
summary: 三层契约修复已按“写出回执决定交付状态 + 单 attempt 停泊/再获取”方案完成并 fast-forward 进 dev（547aed156）；本地 7800 已用新构建重启，Codex 工具往返验证通过；长会话 beta 测试在 tmux codexbeta 中运行。
keywords:
  - issue-2079
  - tool-delivery
  - delivery-receipt
  - delivery-uncertain
  - callback-inbox
  - responses
created_at: 2026-09-17 18
updated_at: 2026-09-18 02
last_verified_at: 2026-09-18 02
decision_policy: verify_before_decision
scope:
  - Root Issue 2079
  - Delivery 2080
  - Delivery 2081
  - Delivery 2082
---

# Root #2079 当前状态

- 谁在做什么：用户于 2026-09-17 要求“做到底、不开 subagent”，我在 `codex/issue-2079` 上补齐两处 contract 缺口后 fast-forward 合入 `dev`（`9bfd8d830` 写出回执、`547aed156` 终态 run 审计事件容错）。线上 Issue #2079/#2080/#2081 状态尚未同步更新，仍显示 `phase:discussion`。
- 采用的语义（用户在草稿中待确认项，本轮按平衡方向直接实现）：SSE/WS writer 的真实写出结果决定 durable delivery 状态；`projected → acked`，写前丢弃 → `pending` 可重放，写中丢失 → `uncertain`，不自动重放、只走显式恢复（`runtime_events.delivery_status='uncertain'`，migration `20260917120000`）。
- 工具轮次 admission：一个 callback_task 只有一个 durable attempt；`processing` 由唯一在途投递持有，`received` 是 partial 之间的停泊态；并发投递只有一个 CAS winner（`claim_published_callback_resume_attempt`），winner 的 payload 覆盖 attempt payload。
- 真实运行发现并修复：最后一个工具结果完成 run 后，`public_run_resume_succeeded` 审计事件因 run 已 sealed 返回 `flow_run_terminal`，把已成功的 resume 变成失败流；现改为终态 run 上 best-effort。
- 证据：`tmp/test-governance/issue-2079-fix2/` 01–09；PG 集成 5/5，api-server 目标测试 40/40 + 3/3，control-plane resume 15/15；Codex `gpt-5.6-luna` 4 次 exec 往返 4 acked / 4 succeeded / 无 WARN。
- 已知未做：control-plane `service::callback_tasks` 中 3 个 fixture 在本分支之前（dev 上 9 个）就已失败，与本次改动无关，未处理；`uncertain` 行的显式恢复入口（API/控制台）尚未实现，仅有存储语义与日志。
- 截止日期：无硬日期。下一步是 beta 长会话结果 + 更新线上 Issue 状态。
