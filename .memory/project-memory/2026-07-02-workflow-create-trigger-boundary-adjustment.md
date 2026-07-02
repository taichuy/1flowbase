---
memory_type: project
topic: workflow create trigger boundary adjustment
summary: 用户确认 Workflow 创建应用弹窗只保留触发器类型选择，完整触发器参数配置进入工作流页；开始节点 workflow_start.config.input_fields 作为业务输入参数契约，触发器只做外部参数到 start input 的映射。线上 L3 issue #1197 已创建并挂到 #1189。
keywords:
  - workflow
  - application-create-modal
  - trigger
  - workflow_start
  - input_fields
  - issue-1197
match_when:
  - 调整 Workflow 创建弹窗
  - 实现 Workflow 触发器配置
  - 处理 workflow_start 输入参数与扩展接口参数映射
created_at: 2026-07-02 08
updated_at: 2026-07-02 08
last_verified_at: 2026-07-02 08
decision_policy: verify_before_decision
scope:
  - web/app/src/features/applications
  - web/app/src/features/agent-flow
  - workflow
---

# workflow create trigger boundary adjustment

## 谁在做什么

用户在 `2026-07-02 08` 确认 Workflow 创建应用流程需要做一个小调整：创建弹窗不再承载完整触发器参数配置，只保留应用类型、Workflow 触发器类型、名称和描述。AI 已创建线上 L3 issue #1197，并把它追加到 L2 #1189 的子 issue 列表。

## 为什么这样做

当前 #1193 实现让创建弹窗提交完整扩展接口 / 定时触发参数，同时工作流页内也有触发器配置入口，导致同一配置深度在创建阶段和应用内重复出现。更合理的边界是：创建应用只生成 workflow 应用壳并选择触发器类型；进入工作流页后再配置触发器入口和发布参数。

## 为什么要做

Workflow 的业务输入参数应由 `workflow_start.config.input_fields` 定义，开始节点是业务输入契约的 owner。触发器配置只负责外部 path / query / form / body 参数到 start input 的映射；扩展接口 target 应从 `workflow_start` 输入字段生成候选项，避免用户自由手写 selector 导致前后端 contract 口径漂移。

## 决策

- 创建 Workflow 应用弹窗只保留最小创建字段和 `extension` / `schedule` 触发器类型选择。
- 扩展接口 slug、HTTP method、response mode、parameters，以及定时 cron、timezone、input payload 进入工作流页内触发器配置入口。
- `workflow_start.config.input_fields` 是 workflow 业务输入参数契约；触发器参数 target 从这些 input fields 生成候选项。
- 不改变后端 `/api/ex/{slug}`、OpenAPI 注册、发布、sync / async 运行 contract。
- 不重开 #1193；新调整由 L3 #1197 承载，父 issue 是 #1189。

## 截止日期

该记忆在 #1197 实现和验收期间有效；若后续决定让创建弹窗重新承载完整触发器配置，或改变 target selector 语义，需要更新。
