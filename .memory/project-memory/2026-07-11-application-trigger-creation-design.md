---
created_at: 2026-07-11 00
updated_at: 2026-07-11 00
memory_type: project
decision_policy: verify_before_decision
scope: application trigger creation and OpenAPI publication
---

# 应用创建时配置触发器

## 谁在做什么

- 用户要求在创建 Workflow 应用时直接配置定时触发器或接口扩展触发器。
- 后续实现需要统一前端创建表单、后端创建合同、触发器持久化和 OpenAPI 发布状态。

## 为什么这样做

- 当前创建入口只保存 `workflow_trigger_type`，具体定时配置和接口扩展配置分散在创建后的编辑与发布流程。
- 用户希望创建应用时完成触发器初始定义，减少二次配置入口和概念割裂。

## 已确认决策

- 接口扩展在创建后可进入 OpenAPI 草稿，但只有应用成功发布后才允许调用。
- 定时触发器创建时默认禁用，不自动执行空流程或未发布流程。

## 待确认决策

- 触发器配置的统一持久化结构，以及接口扩展配置是否从 API Mapping JSON 中拆出。

## 截止日期

- 未指定。

## 决策背后动机

- 保持应用创建体验完整，同时不破坏工作流版本发布与运行时可调用性的边界。
- 避免未发布工作流被外部请求或调度器执行。
