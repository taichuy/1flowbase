---
memory_type: project
topic: Frontstage 通用左侧栏滚动边界
summary: 平衡方案已在 dev 工作树实现并通过定向测试、类型检查、样式边界与桌面/响应式运行态验证；尚未 commit。
keywords:
  - frontstage sidebar
  - SectionPageLayout
  - scroll owner
  - page tree
  - 添加菜单
match_when:
  - 修复 Frontstage 左侧菜单无法滚动
  - 调整 SectionPageLayout viewport sidebar
  - 验收菜单树与主工作区独立滚动
created_at: 2026-08-28 23
updated_at: 2026-08-28 23
last_verified_at: 2026-08-28 23
decision_policy: verify_before_decision
status: active
scope:
  - web/app/src/shared/ui/section-page-layout
  - web/app/src/features/frontstage/components/FrontStagePageTreeSidebar.tsx
  - web/app/src/features/frontstage/components/frontstage-page-tree-sidebar.css
---

# Frontstage 左侧栏滚动方案已确认

## 谁在做什么

当前开发会话已在 `dev` 工作树实现桌面端 Frontstage 通用左侧栏滚动修复，产品代码尚未 commit。

## 为什么这样做

`heightMode="viewport"` 会裁切整体视口，而共享左栏和 Sider children 使用 `overflow: hidden`，菜单树没有独立滚动 owner。滚动复杂度应由能够控制高度的 `SectionPageLayout` 承担，Frontstage 菜单组件只负责树区域与底部操作的分区。

## 为什么要做

长菜单当前超出视口后无法访问，只能通过折叠分组偶然露出，属于内容可达性缺陷。

## 已批准结果与验收

无日历截止日期。桌面端菜单树与主工作区分别滚动；“添加菜单”固定在左栏底部；移动端保持自然页面流。

2026-08-28 23 的候选证据：定向 Vitest `58/58`、TypeScript no-emit、ESLint、Prettier、`page.frontstage` 与 `page.settings` style-boundary 通过；Playwright 在 1280×720 记录菜单树 `clientHeight=590`、`scrollHeight=741`、最大 `scrollTop=151`，滚动后底部操作位置和主工作区 `scrollTop=0` 均保持不变；1024×600 可滚至最后节点且底部操作仍在视口内；820px 使用整页自然滚动，390px 保持既有隐藏侧栏降级。
