---
memory_type: project
topic: Issue #2034 应用日志分层职责固定与映射层对齐
summary: 用户确认以新 Issue #2034 承载（#2032 仅保留历史）先做映射层 Responses 协议对齐（call_kind 从 AiNativeOperation 派生、协议名由映射层写入、compaction 与 turn 调用分开计数），任务投影实体与 application_conversations 重载留 Delta 3 另行 framing。
keywords:
  - issue-2035
  - issue-2034
  - issue-2032
  - application logs
  - ApplicationRunLogContext
  - call_kind
  - compaction
  - codex responses
match_when:
  - 处理 #2034 / #2032、应用日志列表/对话/追踪聚合、Codex Responses 身份或 compaction 日志归组时
created_at: 2026-09-12 01
updated_at: 2026-09-12 15
last_verified_at: 2026-09-12 15
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane/src/application_public_api/compat/openai/log_context.rs
  - api/crates/storage/durable/postgres/src/orchestration_runtime_repository/application_run_logs/
  - web/app/src/features/applications/components/logs/
---

# Issue #2032 日志分层现状与 Delta 2 决策

## 谁在做什么
用户（创始人 / 架构师）与 AI 在 #2032 内继续返工应用日志聚合。e18aed314 已把 gateway 双日志体系收敛回原日志体系，等待用户独立验收。

## 已确认诊断（2026-09-12）
- 三张原表（log_summaries / conversation_message_items / trace_projection）职责本身清晰，问题是"任务"没有归属实体：列表按 `coalesce(log_task_run_id, flow_run_id)` 查询时分组，同任务 / 同会话谓词在 SQL 中重复 3+4 处，`invocation_count` 列被约束恒为 1。
- 身份只在 OpenAI 兼容映射层捕获，AI Native 层是 Option 透传，存储绑定硬编码 `protocol = "openai_responses"`。
- Codex compaction 请求沿用用户 turn 的 `turn_id`，当前被计入用户任务调用数；prewarm 由 WS actor 本地应答不产生 run；memory 请求无 thread 身份。

## 已批准的活动计划（#2034）
活动计划：https://github.com/taichuy/1flowbase/issues/2034（#2032 正文与 Delta 2 评论已 superseded）
- `ApplicationRunLogContext` 增加 `protocol / call_kind / subagent_kind / request_kind`；`call_kind` 由 AI Native 层 `AiNativeOperation.kind()` 派生，日志层只消费。
- 映射层对账 Codex 三种投影（body flat、`x-codex-turn-metadata`、HTTP header），冲突即 `conflicting_identity`。
- 同 turn 仍同任务；任务行 `invocation_count` 只计 generate，新增 `compaction_count`，token 汇总含全部。
- 追加范围：同任务 / 同会话范围谓词收敛为唯一定义（AC-006），删除恒为 1 的 invocation_count 占位列。
- 非目标：任务投影实体表、列表分页结构重写、`application_conversations` 重载、任何 gateway 结构恢复。

## 为什么
先把映射层输入做干净，任务投影实体的取舍才有可靠输入；用户认可"先映射层、后任务表"的顺序。

## 截止与动机
无硬截止；用户希望日志三层（列表 / 对话 / 节点详情）成为容器式层级而非查询时拼接。Delta 3 未获批。

## 当前状态（2026-09-12 12）
- #2034 实现已提交到 `dev`，等待用户验收；AC-001～009 证据在 `tmp/test-governance/issue-2034/`。
- 遗留缺口（不在 #2034 范围，待另开 Issue）：Responses 网关不接受 Codex 本地摘要压缩请求体（`additional_tools` / `custom_tool_call` 输入项），真实 compaction 调用被拒；任务投影实体与 `application_conversations` 重载仍未 framing。

## #2035 三层投影（2026-09-12 15）
- 用户确认设计：列表 = 一次任务；详情 = 用户输入 + 最终输出；LLM 节点下子树承载工具回调 / 轮次 / subagent。已定口径：终态以"观察到最终回答"为准（活动成员存在则 in_progress）；子代理作为父任务 LLM 节点下的子树，不在列表独立成行。
- 已交付并推送 `dev`（`662770764`），#2035 进入 user-acceptance：`application_run_log_tasks` 表 + 写侧重算；run 投影回归纯 1 行 = 1 run；`task_summary.sql` 与 `application_run_log_task_runs` 已退役；追踪新增 `round_group / task_round / child_task` 节点。
- 遗留：网关不接受 Codex 本地摘要压缩请求体（另开 Issue 未建）；`application_conversations` 承载日志会话的重载未处理；前端 `application-run-detail-panel` 与 `lazy-trace-groups` 两组基线失败测试未修。

## 后续架构审计范围（2026-09-12 17）

- 用户重新明确：三层是同一任务的渐进读取，不是会话→任务→调用的实体层级。第一层高频单任务摘要最好单表；第二层包含请求当时历史、系统提示词、用户输入、最终输出；第三层包含工作流输入输出与节点树、工具/回调/subagent，支持 AI 优化分析。
- 保留映射协议→AI Native→供应商边界，不改成会话级统计入口。用户要求先审计成熟架构、数据结构、算法、建表索引，并以功能完整性为约束。
- 本轮只批准审计，未批准表迁移、删除或实现。旧 #2035 的“第二层仅用户输入+最终输出”不能覆盖本次更完整目标；旧交付状态不代表新目标已满足。
- 当前建议待用户决策：单 task 摘要 + 既有事实/用量账本 + 上下文版本引用 + 懒加载 trace，先迁消费者再评估退役 summaries。无硬截止；动机是减少重复职责且保证可复盘性，不以表数量作为唯一优化指标。
