---
memory_type: feedback
feedback_category: repository
topic: reference-page-reuse-shared-primitives
summary: 用户指定参考现有页面时，先识别并复用该页面已有共享组件，不复制近似 UI，也不重复抽象同类 wrapper。
keywords:
  - frontend reference page
  - shared component
  - DataTable
  - settings applications
  - ui reuse
match_when:
  - 用户要求新页面参考或模仿仓库中的现有页面
  - 两个设置页具有相同的表格、工具栏、分页或字段配置模式
  - 准备复制现有页面 UI 或新增相似共享组件
created_at: 2026-08-01 00
updated_at: 2026-08-01 00
last_verified_at: 2026-08-01 00
decision_policy: direct_reference
scope:
  - web/app/src/features/settings
  - web/app/src/shared/ui
---

# Reference Page Reuses Shared Primitives

## 规则

- 用户指定参考仓库内现有页面时，先检查该页面依赖的共享 UI primitive，并优先直接复用。
- 已有共享组件能够承载相同 invariant 时，不复制一份近似交互，也不再新增只改名或转发参数的 wrapper。
- 只把稳定、跨领域的视觉与交互机制共享；筛选条件、行操作、详情内容和领域状态继续由各 feature 拥有。

## 原因

- “参考页面”通常同时表达了视觉、交互和维护方式的一致性要求。
- 复制原生组件会让列宽、分页、工具栏和持久化行为继续分叉；重复抽象则增加没有职责的新层。

## 适用场景

- `/settings/applications` 与其他设置页表格管理界面的统一。
- 已存在 `DataTable`、`SettingsSectionSurface` 等共享 primitive 的页面开发。
