---
memory_type: feedback
feedback_category: repository
topic: Frontstage 未绑定页面空态只显示 Empty
summary: Frontstage 工作区没有绑定页面时只显示 Ant Design Empty 图形，不增加标题、解释文案、分割线或创建区块入口。
keywords:
  - frontstage
  - empty state
  - Ant Design Empty
  - page binding
match_when:
  - 调整 Frontstage 未选中或未绑定页面的工作区空态
  - 为明显空态设计标题、说明文字或操作入口
created_at: 2026-07-19 11
updated_at: 2026-07-19 11
last_verified_at: 2026-07-19 11
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage/components/PageCanvas.tsx
  - web/app/src/features/frontstage/pages/FrontStagePage.tsx
---

# Frontstage 未绑定页面空态只显示 Empty

## 时间

`2026-07-19 11`

## 规则

Frontstage 工作区没有绑定页面时，只渲染 Ant Design `Empty` 图形并关闭 description；不显示技术标题、解释文案、分割线或“创建区块”入口。

## 原因

用户指出该状态本身已经足够直观，额外提示会制造视觉和认知噪声；同时，没有页面作为区块 owner 时不应暴露区块创建能力。

## 适用场景

- Frontstage 根工作区没有选中或绑定页面。
- 页面树为空时的中央工作区空态。
- 容器 owner 不存在、因此下级资源操作不成立的类似空态。

## 备注

左侧页面树可以保留自己的页面创建引导；本规则只约束中央页面工作区。
