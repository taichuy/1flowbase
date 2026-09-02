---
memory_type: project
topic: Frontstage 当前页面局部刷新协调器
summary: "#1975 平衡方案已在 dev 工作树实现并通过定向测试、类型、i18n 与运行态菜单取证；尚未 commit，等待用户验收。"
keywords:
  - frontstage refresh
  - Refresh Coordinator
  - current page
  - query refetch
  - generation fencing
match_when:
  - 验收 Frontstage 当前页面局部刷新
  - 扩展页面刷新范围或引入 revision
  - 处理刷新期间切换页面的竞态
created_at: 2026-09-02 18
updated_at: 2026-09-02 18
last_verified_at: 2026-09-02 18
decision_policy: verify_before_decision
status: active
scope:
  - web/app/src/features/frontstage/pages/FrontstageWorkspacePage.tsx
  - web/app/src/features/frontstage/pages/FrontStagePage.tsx
  - web/app/src/features/frontstage/pages/frontstage-page
  - web/app/src/features/frontstage/i18n
---

# Frontstage 当前页面局部刷新已实现

## 谁在做什么

当前开发会话已在 `dev` 主工作树实现 GitHub Single Issue #1975，产品代码尚未 commit。

## 为什么这样做

刷新被建模为当前 Page/Tab 投影的重新同步：页面私有 coordinator 并行刷新 PageContent、BlockRoots 与可选 RuntimeAssembly，页面内部使用 stale-while-revalidate、防重入和 generation fencing，菜单只触发狭接口。

## 为什么要做

用户需要在页面配置菜单中刷新当前局部页面，不重载应用壳层、左侧导航、路由、当前 Tab 和设计模式。

## 已批准结果与验收

无日历截止日期。#1975 的 AC-001～AC-004 已由菜单测试、coordinator 故障注入测试、页面 pending/failure/跨 scope 竞态测试、TypeScript no-emit 和 `/demo` Playwright 运行态截图覆盖。后续若需强一致快照、多人编辑合并或刷新页面树，需重新进入 `problem-framing`。
