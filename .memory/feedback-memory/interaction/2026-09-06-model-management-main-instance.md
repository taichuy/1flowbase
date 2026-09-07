---
memory_type: feedback
feedback_category: interaction
topic: 模型管理以主实例为单元
summary: 简化模型管理 GUI 时必须保留后台以主实例为管理单元的领域关系，不能平铺来源实例替代主实例。
keywords: [模型管理, 主实例, 来源实例, 低代码]
match_when: [构建模型管理页面, 简化模型接入流程]
created_at: 2026-09-06 10
updated_at: 2026-09-06 10
decision_policy: direct_reference
scope: [Frontstage, model-providers]
---

## 规则

以 `/settings/model-providers/providers` 当前实现为参考，模型管理以主实例为单元；来源实例是主实例下的接入配置，不应平铺来源连接作为页面的管理主体。

## 原因

用户明确纠正了低代码页面平铺来源实例的做法。简化交互不能改变主实例聚合模型与来源实例的领域关系。

## 适用场景

模型接入低代码页面和下游应用分组选模；主实例聚合状态与路由以当前后端契约为准。
