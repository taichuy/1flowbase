---
memory_type: project
topic: workflow 一期编辑器边界修正方向
summary: 用户于 2026-07-10 在人工验收后确认此前浅层装配仍是 AgentFlow 换皮；继续采用共享画布内核 + 独立 Workflow 编辑器装配，但允许重新划分 AgentFlowCanvasFrame 职责。一个 Workflow 应用仍只有一个触发器，触发器配置归 workflow_start，workflow_end 统一定义 Workflow Result，Workflow 使用测试运行 / Result / Trace 而非预览对话。
keywords:
  - workflow
  - issue-1186
  - feature-boundary
  - editor-shell
  - trigger
match_when:
  - 继续 workflow 一期修正的 issue 起草、实现或验收
  - 需要判断 workflow 前端架构方向或中间节点范围
created_at: 2026-07-03 00
updated_at: 2026-07-10 00
last_verified_at: 2026-07-10 00
decision_policy: verify_before_decision
scope:
  - web/app/src/features/agent-flow
  - web/app/src/features/applications
  - api/crates/control-plane/src/application_public_api
  - api/apps/api-server
---

# workflow 一期修正方向已拍板为方案 B

## 时间

`2026-07-03 00`

## 谁在做什么

用户（产品负责人）委托对 issue #1186 树（workflow 一期）做人工产品验收。2026-07-10 确认此前 #1207 虽建立了 feature 目录和 capabilities，但 Workflow 页面仍直接装配 `AgentFlowEditorPage`，只隐藏对话调试 / 会话变量 / 系统变量，开始节点、结束节点、画布和运行交互仍由 AgentFlow 领域模型主导，因此需要回到 #1187 / #1189 修订设计边界。

## 决策内容

- 继续采用 balanced 方向：共享纯画布内核，建立真正独立的 AgentFlow Editor Assembly 与 Workflow Editor Assembly；允许重新划分 `AgentFlowCanvasFrame` ownership，旧的“不大拆 editor store / canvas ownership”停止条件不再有效。
- 一个 Workflow 应用只有一个 `workflow_trigger_type`，不允许同时挂载多个触发器，也不按触发器拆 Application Type。
- 触发器完整配置继续归 `workflow_start`；开始节点按唯一 trigger type 渲染专属配置，并把手动表单、定时 payload 或 HTTP 参数映射为 Workflow 输入变量。
- `workflow_end` 对所有触发器统一表示 Workflow Result；不同触发器只改变结果交付方式，不拆不同 End 节点。
- Workflow 编辑器使用“测试运行 + Workflow Result + Trace”，不使用 AgentFlow 的预览对话、聊天消息或 Answer 语义。
- 中间节点范围：Workflow picker 直接复用 AgentFlow 通用执行节点（AgentFlow picker 可见 builtin 集合排除 start/answer，加插件贡献节点）。
- 后端 orchestration runtime 继续复用；实现前需确认现有 draft debug-run 是否按 application type 编译 Workflow document 并返回 `workflow_end` Result，若缺失则补职责单一的 Workflow 测试运行 contract。

## 为什么

审核结论：此前实现解决了目录边界、picker 和触发器运行门禁，但没有建立 Workflow 的独立编辑、测试输入和结果交互模型。必要复杂度应收敛到两个应用类型的 assembly，而不是继续向共享编辑器增加 capabilities / bool 分支，也不复制两套画布内核。

## 截止与状态

- 已完成（2026-07-04）：#1205（picker 放开，e9582ef7）、#1207（feature 边界与装配层，ce79e8f8）、#1208（调度接线，5a47e6e1，新增 time-tz 依赖）、#1209（运行时门禁，7950c19b）全部实现、统一 QA 通过、关闭并推送 beta。
- 关键实现事实：kernel 节点注册点在 `web/app/src/features/agent-flow/lib/node-definitions/registry.ts`，workflow 注册入口 `features/workflow/register.ts`（生产装配在 `App.tsx`，测试按需 import，不进全局 setup——全局 import 会破坏 vi.mock 与 i18n 时序）；调度 tick 与 worker 循环 spawn 在 api-server `app_from_config`；互斥门禁以 `workflow_trigger_type` 为运行时判定，非活跃类型配置存而不用。
- 待办：回到现有 #1186 -> #1187 -> #1188/#1189 issue 树，先修订 L1/L2 决策，再创建新的 L3 执行 issue；旧 #1207 保持关闭，作为“浅装配未满足人工验收”的历史实现证据，不重开。
