---
memory_type: feedback
feedback_category: repository
topic: ECharts 宿主必须能随 viewport 收缩
summary: ECharts 位于固定 viewport 的 Grid/滚动容器时，first-party chart host 必须显式 min-width 0、max-width 100% 并裁住旧 canvas，避免窗口缩窄后 canvas 的旧 intrinsic width 撑大页面并产生横向无限衍生。
keywords:
  - echarts
  - resize
  - viewport
  - grid
  - overflow
match_when:
  - ECharts 页面窗口缩放后出现横向滚动条
  - canvas 宽度大于 chart panel 或滚动容器
  - ResizeObserver 没有让 chart 随父容器缩小
created_at: 2026-07-17 14
updated_at: 2026-07-17 14
last_verified_at: 2026-07-17 14
decision_policy: direct_reference
scope:
  - web/app/src/features
  - web/app/src/shared
  - web/packages/block-renderer
---

# ECharts 宿主必须能随 viewport 收缩

## 规则

ECharts 放在 CSS Grid、Flex 或固定 viewport 滚动区时，自有 chart host 必须显式允许收缩：`min-width: 0`、`max-width: 100%`；需要跨 resize 保护时由 host 使用 `overflow: hidden`，不要覆盖 ECharts 内部 canvas。

## 原因

ECharts 会给 canvas 写入当前像素宽度。若页面先在宽视口初始化再缩窄，Grid item 的默认 intrinsic minimum 可能保留旧 canvas 宽度，导致 host 自己没有变窄，ResizeObserver 也无法触发正确 resize，最终撑大外层 `scrollWidth`。

## 适用场景

- 图表处于 settings viewport、抽屉、分栏、可折叠侧栏或可调整宽度容器。
- 验收必须覆盖“宽视口打开后再缩窄”，并断言滚动容器 `scrollWidth === clientWidth`、chart 与 canvas 宽度一致；只在固定宽度首次打开不足以发现该回归。
