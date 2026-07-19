---
memory_type: feedback
feedback_category: repository
topic: Frontstage 未绑定页面空态只显示 Empty
summary: Frontstage 中央工作区未绑定页面时只显示 Ant Design Empty 图形；左侧页面树不显示 Empty，“添加菜单”始终作为树的末尾行，空树时成为第一行。
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
  - web/app/src/features/frontstage/components/FrontStagePageTreeSidebar.tsx
  - web/app/src/features/frontstage/pages/FrontStagePage.tsx
---

# Frontstage 未绑定页面空态只显示 Empty

## 时间

`2026-07-19 11`

## 规则

Frontstage 中央工作区没有绑定页面时，只渲染 Ant Design `Empty` 图形并关闭 description；不显示技术标题、解释文案、分割线或“创建区块”入口。左侧页面树不渲染 Empty；“添加菜单”属于树列表，始终紧跟最后一个顶层节点，空树时就是第一行，默认透明、悬停浅蓝。

## 原因

用户指出该状态本身已经足够直观，额外提示会制造视觉和认知噪声；同时，没有页面作为区块 owner 时不应暴露区块创建能力。

## 适用场景

- Frontstage 根工作区没有选中或绑定页面。
- 页面树为空时的中央工作区和左侧树入口。
- 容器 owner 不存在、因此下级资源操作不成立的类似空态。

## 备注

左侧页面树保留“添加菜单”能力，但不额外显示空态插图或解释文案。
