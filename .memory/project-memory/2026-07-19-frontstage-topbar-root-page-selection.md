---
memory_type: project
topic: Frontstage 顶部菜单根路由只默认打开直属页面
summary: 用户确认 Frontstage 顶部菜单进入分组根路由时，只按侧边栏顶层顺序从上到下选择第一个直属 page；group 是信息收敛边界，不递归选择分组内页面。只有分组内页面时保留根工作区。
keywords:
  - frontstage
  - topbar
  - sidebar
  - routing
  - default page
  - group boundary
match_when:
  - 调整 Frontstage 顶部菜单根路由
  - 决定页面树的默认页面选择规则
  - 处理 group 与 page 的自动下钻语义
created_at: 2026-07-19 09
updated_at: 2026-07-19 09
last_verified_at: 2026-07-19 09
decision_policy: verify_before_decision
status: active
scope:
  - web/app/src/app/router.tsx
  - web/app/src/features/frontstage/lib/page-tree.ts
---

# Frontstage 顶部菜单根路由默认页面

- 谁在做什么：Frontstage 路由层负责在顶部菜单根路由下选择直属顶层页面，并规范化到包含 `pageId` 的 URL。
- 为什么这样做：页面放入 group 后即表示收敛，不应被根路由默认选择逻辑自动深入。
- 为什么要做：避免点击顶部菜单后出现“未选择 pageId”的空态，同时保持侧边栏层级语义可预测。
- 截止日期：2026-07-19 当前 Single Issue 内完成。
- 决策动机：默认行为应尊重信息架构边界；只有用户显式展开并点击分组内页面时才进入下层。

规则：从顶部菜单直属 children 自上而下寻找第一个 `page`，跳过所有 `group`；没有直属 `page` 时保留根工作区，不递归 fallback。
