---
memory_type: project
topic: workflow trigger and Start input contract revision
summary: 用户于 2026-07-10 修订旧 Workflow trigger 边界：API 与 Webhook 是不同触发器；Workflow Start 只定义输入参数；HTTP 参数直接复用 Start 输入字段模型并增加 path/query/body/form 来源属性，不再使用 target selector、第二套 mapping 或默认参数行。线上 #1236 挂到 ADR #1187，旧 #1197/#1198 口径已标记 superseded。
keywords:
  - workflow
  - api-trigger
  - webhook-trigger
  - schedule-trigger
  - workflow_start
  - input_fields
  - parameter-source
  - issue-1236
  - superseded-selector
match_when:
  - 调整 Workflow 创建与触发器配置
  - 实现 API Webhook Schedule trigger contract
  - 修改 workflow_start 输入参数或公开请求参数解析
  - 看到 extension response_mode target selector 或参数 mapping
created_at: 2026-07-02 08
updated_at: 2026-07-10 17
last_verified_at: 2026-07-10 17
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1187
  - https://github.com/taichuy/1flowbase/issues/1236
  - web/app/src/features/workflow
  - web/app/src/features/agent-flow/components/detail/fields/StartInputFieldsField.tsx
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
- 历史 `extension+sync/async`、parameter mapping、OpenAPI 与公开请求解析迁移由 `#1236` 收敛。

## 尚待确认

- 历史 mapping 中 external name 与 Start field key 不一致时的迁移策略。
- `body` 来源第一版是否只支持顶层字段。
- API / Webhook 应用级配置入口位置。
- API 同步等待超时后的响应 contract。

## 截止日期

本记忆在 `#1236` 完成 contract、migration、实现与用户验收前有效；若上述待确认项有新决策，更新本文件和线上 issue。
