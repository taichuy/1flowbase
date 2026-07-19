---
memory_type: project
topic: Schema UI 局部加载边界
summary: 加载反馈由当前最小可观察渲染边界承载；现阶段 Frontstage 使用区块级加载壳，未来有节点级状态后再下沉到 schema 子树。
keywords:
  - schema ui
  - loading shell
  - block renderer
  - frontstage
match_when:
  - 设计或实现 Frontstage、BlockUiRenderer、schema UI 的加载状态与分级渲染
created_at: 2026-07-19 17
updated_at: 2026-07-19 17
last_verified_at: 2026-07-19 17
decision_policy: verify_before_decision
scope:
  - web/packages/block-renderer
  - web/app/src/features/frontstage
  - web/packages/page-protocol
---

# Schema UI 局部加载边界

## 时间

`2026-07-19 17`

## 谁在做什么

Frontstage 与 `@1flowbase/block-renderer` 使用统一的区块局部加载壳承载无 session、source loading、idle 和 running；ready 渲染真实 schema，错误与终态保持显式。

## 为什么这样做

当前 runtime 只在 ready 后提供完整 schema，运行阶段无法诚实推断内部标题、表格或表单形状；最小可观察边界是区块，而不是 schema 子树。

## 为什么要做

消除整块“运行中” Alert 和提前出现的空诊断摘要，并为后续多层 schema UI 建立稳定的加载复杂度 owner。

## 截止日期

当前区块级结果已于 `2026-07-19` 实现；schema 子树级加载只在协议提供节点级异步状态后推进，无预设日期。

## 决策背后动机

加载反馈应由当前最小已知渲染边界负责：页面 → 区块 → schema 子树 → 控件。不得在缺少部分 schema 时伪造节点级骨架，也不得把 loading 压扁成 skipped 或 error。

## 关联文档

- `web/packages/block-renderer/src/BlockUiLoadingShell.tsx`
- `web/app/src/features/frontstage/hooks/use-frontstage-page-canvas-runtime-sessions.ts`
- `web/app/src/features/frontstage/components/PageCanvas.tsx`
