---
memory_type: project
topic: workflow 一期修正方向已拍板为方案 B
summary: 用户于 2026-07-03 确认按方案 B 修正 GPT 实现的 workflow 一期：新建 web/app/src/features/workflow 边界 + 装配层复用编辑器内核；中间节点直接复用 AgentFlow 通用执行节点。
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
updated_at: 2026-07-03 00
last_verified_at: 2026-07-03 00
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

用户（产品负责人）委托对 issue #1186 树（workflow 一期，GPT 实现，提交 6bb8ca9d..712e7a01）做代码审核后，拍板修正方向；AI 按 problem-framing 进入 issue gate 起草 L3 fix issue。

## 决策内容

- 采用方案 B：新建 `web/app/src/features/workflow`，迁入 workflow 专属节点 contract/meta、picker options、trigger renderer/表单/lib；canvas/editor 内核以装配层方式复用，workflow 装配不含聊天语义 UI（对话调试、会话变量等）；控制为装配层拆分，不大拆 AgentFlow editor store（否则触发 #1189 停止条件回 L2）。
- 中间节点范围：Workflow picker 直接复用 AgentFlow 通用执行节点（AgentFlow picker 可见 builtin 集合排除 start/answer，加插件贡献节点）。
- 后端不动架构，仅补定时调度接线（dispatch_due_schedule / consume_one_workflow_schedule_run 无生产调用方）与触发器互斥一致性下沉后端。

## 为什么

审核结论：GPT 实现是"寄生"而非专属边界——workflow 代码散在 agent-flow 与 applications 两个 feature 互相 import，聊天语义 UI 无分流出现在 workflow 编辑器；两个 QA 未拦截的 blocker：picker 只有起止两节点（后端 compiler 放行全部中间节点）、定时触发无调度循环形同虚设。方案 A 会固化寄生架构，方案 C 与 L1 ADR 一期精神冲突。

## 截止与状态

- 已完成（2026-07-04）：#1205（picker 放开，e9582ef7）、#1207（feature 边界与装配层，ce79e8f8）、#1208（调度接线，5a47e6e1，新增 time-tz 依赖）、#1209（运行时门禁，7950c19b）全部实现、统一 QA 通过、关闭并推送 beta。
- 关键实现事实：kernel 节点注册点在 `web/app/src/features/agent-flow/lib/node-definitions/registry.ts`，workflow 注册入口 `features/workflow/register.ts`（生产装配在 `App.tsx`，测试按需 import，不进全局 setup——全局 import 会破坏 vi.mock 与 i18n 时序）；调度 tick 与 worker 循环 spawn 在 api-server `app_from_config`；互斥门禁以 `workflow_trigger_type` 为运行时判定，非活跃类型配置存而不用。
- 待办：父 issue #1189/#1188/#1186 未关，等用户人工验收产品效果后收口。
