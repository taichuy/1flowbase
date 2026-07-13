---
memory_type: project
topic: workflow trigger and Start input contract revision
summary: 用户确认 Workflow Start 继续使用独立 workflow_start 节点并与 AgentFlow Start 分离变量语义：Workflow 输入使用 input.*，只保留真实公共 sys.*，触发事实使用 trigger.*；schedule 与接口类触发器共享节点但使用不同输入 contract。Workflow 尚未发布，不处理任何历史 selector、mapping、绑定或数据兼容。线上 #1252 挂到 #1236。
keywords:
  - workflow
  - api-trigger
  - webhook-trigger
  - schedule-trigger
  - workflow_start
  - input_fields
  - parameter-source
  - system-variables
  - trigger-context
  - input-namespace
  - issue-1236
  - issue-1252
  - superseded-selector
match_when:
  - 调整 Workflow 创建与触发器配置
  - 实现 API Webhook Schedule trigger contract
  - 修改 workflow_start 输入参数或公开请求参数解析
  - 看到 extension response_mode target selector 或参数 mapping
created_at: 2026-07-02 08
updated_at: 2026-07-13 10
last_verified_at: 2026-07-13 10
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1187
  - https://github.com/taichuy/1flowbase/issues/1236
  - https://github.com/taichuy/1flowbase/issues/1252
  - web/app/src/features/workflow
  - web/app/src/features/agent-flow/components/detail/fields/StartInputFieldsField.tsx
  - web/app/src/features/agent-flow/lib/selector-options.ts
  - api/crates/control-plane/src/orchestration_runtime
  - api/crates/control-plane/src/application_public_api
---

# Workflow trigger 与 Start 输入参数 contract 修订

## 谁在做什么

用户在 `2026-07-10 17` 推翻了 `#1197/#1198` 中“Start 输入字段与触发器 parameter mapping 分开维护、通过 target selector 映射”的旧口径，并要求把修订挂到线上 issue。

AI 已创建 L2 Epic `#1236`，父 issue 为 Workflow ADR `#1187`；同时在 `#1187`、`#1189`、`#1197`、`#1198` 留下 superseded 说明，并将 `#1187` 从 `phase:ready / grade:g3` 调整为 `phase:discussion / grade:g4`。

## 为什么这样做

旧结构把同步 API 与异步行为藏在同一个 `extension.response_mode`，又让 Start 输入字段和 `source + name + target selector` mapping 重复定义同一批参数。它会泄露内部 selector、产生重命名失效和默认样例行噪音，也把触发器协议配置塞进 Start 节点。

## 当前已确认决策

- API 与 Webhook 是不同触发器；Schedule 保持独立。
- `Workflow Start` 只定义 Workflow 输入参数，不承载 slug、HTTP method、response mode、cron、timezone 或发布配置。
- API / Webhook 参数编辑直接复用 Start 输入字段新增与编辑模型。
- HTTP trigger 场景只在单个输入字段上增加 `path / query / body / form` 参数来源属性。
- 参数由用户点击 `+` 按需新增，不默认生成 path/query/form 示例行。
- 删除 target selector 和第二套 trigger-to-Start mapping；不得用隐藏 selector 或自动 mapping 伪装兼容。
- AgentFlow `start` 与 Workflow `workflow_start` 继续作为独立领域节点；只共享无应用语义的画布和字段编辑基础设施。
- Workflow 自定义输入使用 `input.*`，不展示 AgentFlow 的 `userinput.*` 内置输入。
- Workflow 公共系统变量只保留语义真实的 `sys.application_id`、`sys.workflow_id`、`sys.workflow_run_id`；AgentFlow 会话变量和 `sys.user_id` 不进入 Workflow。
- 触发运行事实使用 `trigger.*`；所有 Workflow 至少有 `trigger.type`，schedule 额外提供 `trigger.scheduled_at` 与 `trigger.timezone`。
- Schedule Start 不显示 HTTP source；接口类触发器才使用 `path/query/body/form` 将请求字段投影为 `input.*`。
- Workflow 功能尚未发布，不处理历史 Workflow 文档、selector、mapping、绑定或持久化数据兼容；不新增 migration、alias、fallback 或 deprecation 代码。该决策替代 #1236/#1237 中的历史迁移 blocker。
- 变量目录和触发上下文执行 issue 为 `#1252`，父 issue 为 `#1236`。

## 尚待确认

- `body` 来源第一版是否只支持顶层字段。
- API / Webhook 应用级配置入口位置。
- API 同步等待超时后的响应 contract。
- 自定义 `path` 参数需要怎样的真实 URL template；`/api/ex/{slug}` 当前无法承载额外 path 参数。

## 截止日期

本记忆在 `#1236`、`#1237`、`#1252` 完成 contract、实现与用户验收前有效；若上述待确认项有新决策，更新本文件和线上 issue。
