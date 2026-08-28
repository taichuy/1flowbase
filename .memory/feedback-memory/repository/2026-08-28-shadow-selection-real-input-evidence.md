---
memory_type: feedback
feedback_category: repository
topic: ShadowRoot 划词验收必须使用真实用户输入
summary: Native Block 的 Selection / Range 行为不得用程序化 Range + 合成 mouseup 结算；必须用 Playwright 真实双击或拖选覆盖浏览器 Shadow DOM retargeting。
keywords:
  - frontstage
  - native block
  - ShadowRoot
  - Selection
  - Range
  - Playwright
  - real input
match_when:
  - 实现或验收 Native Block 划词、选区菜单、Range DOMRect
  - 验证 ShadowRoot 内鼠标、Selection 或 Dropdown 虚拟锚点
created_at: 2026-08-28 16
updated_at: 2026-08-28 16
last_verified_at: 2026-08-28 16
decision_policy: direct_reference
scope:
  - web/packages/page-runtime
  - web/app/src/features/frontstage
  - scripts/node/page-debug
---

# 规则

Native Block 中的划词、Selection、Range 和虚拟锚点浏览器验收必须使用 Playwright 真实鼠标输入（至少真实双击选词，拖选场景优先真实 drag）。程序化 `document.createRange()` 后只派发合成 `mouseup` 只能验证应用回调，不能结算真实用户路径。

# 原因

Chromium 对真实 ShadowRoot 用户选区会 retarget 外层 `window.getSelection()`：文本仍存在，但其 Range 可能是 `0×0`；同一时刻 `ShadowRoot.getSelection()` 才持有真实矩形。程序化 Range 不经过该行为，曾导致 #1923 首轮浏览器证据错误判绿，用户实际复验仍失败。

# 适用场景

- Native React Block 的划词菜单、文本工具栏和上下文操作。
- 依赖 `Range.getBoundingClientRect()` 的 Dropdown、Popover、Tooltip 虚拟锚点。
- Shadow DOM 中的 Selection API 兼容、事件顺序和坐标验收。
