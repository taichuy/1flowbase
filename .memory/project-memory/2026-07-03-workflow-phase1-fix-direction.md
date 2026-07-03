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

- 下一步：4 个 L3 fix issue 草案（前端架构边界、picker 放开、后端调度接线、互斥下沉）待用户确认后创建；前端两个挂 #1189，后端两个挂 #1188。
- #1190/#1193/#1194 关闭状态与实际不符，按 #1197/#1198 先例开新 fix issue 承接，不重开旧 issue。
