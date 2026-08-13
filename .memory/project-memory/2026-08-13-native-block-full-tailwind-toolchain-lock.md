---
memory_type: project
title: 代码区块完整 Tailwind 与可复现工具链锁定
created_at: 2026-08-13 18
updated_at: 2026-08-13 18
decision_policy: verify_before_decision
scope:
  - web/packages/page-runtime
  - web/packages/tailwindcss-catalog
  - web/app/src/features/frontstage
  - web/app/src/shared/code-block
  - api/crates/control-plane
  - api/crates/storage-durable
status: issue_discussion
keywords:
  - native-react
  - tailwindcss
  - shadow-root
  - dependency-lock
  - durable-artifact
  - github-issue-1679
---

# 代码区块完整 Tailwind 与可复现工具链锁定

## 谁在做什么？

用户在 `2026-08-13 18` 确认把代码区块从 #1671 的有限 Tailwind utility inventory 改为标准 Tailwind v4 源码驱动编译方向。AI 已创建 GitHub Single Issue #1679 供用户在线审阅，当前为 `phase:discussion`，尚未授权实现。

## 为什么这样做？

每个 Native React 区块已有独立 ShadowRoot，样式扩散由 runtime 隔离；继续维护 481 项私有 inventory 会误报块内自定义 CSS，并让标准 Tailwind utility、variant 与 arbitrary value 受 1flowbase 私有限制。

## 为什么要做？

恢复代码区块的标准 React、DOM、CSS 作者契约，同时保证平台升级 Tailwind 后历史页面不发生静默视觉漂移。

## 当前方向

- 根据冻结 TSX 源码生成 utilities-only CSS，继续关闭 Preflight。
- 源码、完整 Tailwind toolchain lock、生成 CSS 耐久制品与摘要由后端可执行状态原子持久化。
- 缓存只作性能优化；缓存删除不得改变运行结果。
- 平台升级只改变新建区块默认工具链；已有区块只有显式升级并成功保存后才切换。
- runtime 继续把样式作为 `shadow_style` 注入当前区块 ShadowRoot。
- 不开放第三方 plugin、自定义配置、JavaScript 配置执行或主仓全局 Tailwind。
- #1679 取代 #1671 的有限 inventory contract，但不推翻主仓 Tailwind 禁令和 ShadowRoot 隔离。

## 截止日期

无固定日期。下一状态转换是用户审阅并确认 #1679 正文后，将 Issue 改为 `phase:ready`，再授权实现。

## 决策背后动机

样式隔离复杂度应由已拥有 DOM 边界的 runtime 吸收；依赖升级复杂度应由后端 lock 与耐久制品吸收，不能泄漏成作者需要理解的私有白名单，也不能由易失缓存冒充历史运行真值。

## 验收证据入口

- GitHub Issue：https://github.com/taichuy/1flowbase/issues/1679
- 重点验证：完整静态 Tailwind 能力、自定义 CSS 混用、A/B/host ShadowRoot 隔离、双版本 lock、cache-miss 可复现、原子保存、历史迁移 preview/rollback。
