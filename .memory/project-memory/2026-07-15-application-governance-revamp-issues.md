---
memory_type: project
topic: 应用治理改造五个 issue 已拍板并挂上 GitHub
summary: 用户确认 /settings/applications 治理改造方向并回答全部拍板点，2026-07-15 已创建 #1286-#1290 五个 issue；#1286 生命周期闭环是地基，用户指示"继续"后进入实现。
keywords:
  - application
  - settings
  - lifecycle
  - publication
  - revert-to-draft
  - issue
match_when:
  - 继续实现或验收 #1286-#1290 任一 issue
  - 讨论应用发布、下线、复制、模板导出或设置页改版
  - 需要回忆用户对生命周期语义的拍板结论
created_at: 2026-07-15 15
updated_at: 2026-07-15 15
last_verified_at: 2026-07-15 15
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane/src/application_public_api
  - web/app/src/features/settings/components/application-management
  - web/app/src/features/applications
---

# 应用治理改造五个 issue 已拍板并挂上 GitHub

## 时间

`2026-07-15 15`

## 谁在做什么

- 用户对 `/settings/applications` 提出治理改造需求，经 problem-framing 三方向对齐后拍板平衡方向，并要求先用 `gh` 挂 issue 再开发。
- AI 创建了五个 issue：
  - #1286 [待实现]应用发布生命周期闭环：已发布可退回草稿，并按触发器统一启停语义（地基，grade:g3）
  - #1287 [待实现]修复应用复制不包含流程定义的假复制缺陷（缺陷）
  - #1288 [待实现]workflow 类型应用支持模板导出导入闭环
  - #1289 [待实现]/settings/applications 改版为表格加详情抽屉，编辑表单与新增对齐（依赖 #1286）
  - #1290 [讨论]应用日志与监控页信息架构重构（后置讨论，不动代码）

## 用户拍板结论（决策原文语义）

- 编辑表单必须与新增表单能力一致：新增有类型/触发器/图标而编辑只有名称/描述是核心痛点。
- 生命周期收敛为「草稿 ⇄ 已发布」双态，新增"从已发布退回草稿"转换；不引入第三个用户可见状态。
- 按触发器落地停用语义：extension = 接口取消注册；schedule = 停止调度；agent_flow 公开 API = 停止对外调用。统一为一个发布开关心智，`api_enabled` 从用户心智退位。
- 复制必须包含流程定义（真复制），按缺陷处理；环境变量默认随复制。
- 日志报表问题独立后置为讨论 issue。

## 为什么这样做

- 发布单向是后端能力缺口：port 层只有 `create_active_application_publication_version` 与 `set_application_api_enabled`，没有下线命令；workflow 类型前端被过滤掉 API 分区，连开关都不可达。
- schedule 调度只读 `workflow_schedule_triggers.enabled` 与 publication 无关，状态列会失真——实现 #1286 时必须核实调度执行的是草稿还是发布快照，再定发布语义方案（发布=创建 publication+启用 trigger，或 调度=enabled AND published）。

## #1286 实现进展（2026-07-16 核实）

- 已核实：schedule 调度真值其实**同时**依赖 publication——`workflow_schedule.rs` 的 dispatch 要求存在 active + api_enabled publication，trigger.enabled 只是附加条件。故发布语义采用「调度 = enabled AND published」，unpublish 不触碰 trigger 配置。
- 已合入 dev（commit 0ae048298）：control-plane `unpublish` 命令 + port `deactivate_application_publication_versions` + postgres/in-memory 双实现 + `DELETE /api/console/applications/{id}/api-publication` 路由 + 设置页「退回草稿」菜单项。附带修复 postgres `load_active_application_publication` 漏 `active` 过滤的存量缺陷。
- AC-001/002/003/004/006 已由 control-plane 6 测 + 路由 2 测结算；AC-005（单一开关心智）未完成。
- 剩余两项都是前端 UI，按用户偏好（前端视觉先看效果再提交）停在此等确认：① workflow 编辑器可达的发布/退回入口（workflow 应用当前 UI 无法发布，`saveWorkflowScheduleTrigger`/`publishApplicationApiVersion` 在 workflow 前端零调用方）；② `ApplicationApiPage` 把 `api_enabled` Switch 收敛为发布状态语义。

## 截止与状态

- 无硬截止；用户 2026-07-15 指示"继续"，按依赖顺序从 #1286 开始实现，落 dev 分支。#1287/#1288/#1289/#1290 尚未动工。
