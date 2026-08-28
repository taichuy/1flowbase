---
memory_type: project
topic: Frontstage Block 自然尺寸与宿主分配尺寸双通道
summary: 用户批准 #1926 将 Native Block intrinsic measurement 与 allocated viewport 解耦；实现以 RenderIdentity 归属提交自然尺寸，再开放完整宿主内容区，禁止 allocated height 回写 intrinsic height。
keywords:
  - frontstage
  - native block
  - intrinsic size
  - allocated viewport
  - equal row height
  - issue 1926
match_when:
  - 修改 Frontstage 自动高度、同行等高或 Block sizing contract
  - 排查 Block 外框扩展但内部 Runtime 未占满
  - 修改 ctx.ui.sizing.available 或 reportIntrinsicSize
created_at: 2026-08-28 21
updated_at: 2026-08-28 21
last_verified_at: 2026-08-28 21
decision_policy: verify_before_decision
status: user-acceptance
scope:
  - https://github.com/taichuy/1flowbase/issues/1926
  - web/app/src/features/frontstage/components/PageCanvas.tsx
  - web/app/src/features/frontstage/lib/page-canvas/auto-height-layout.ts
---

# Frontstage 双尺寸通道

## 谁在做什么

用户于 2026-08-28 确认把 Block 自然内容需求与宿主分配空间拆成双通道，并授权建立 Single Issue #1926 后实现。当前实现和 Dev Acceptance Gate 已完成，等待用户在目标页面验收。

## 为什么这样做

自动等高已把目标 Block 外框扩展到同行最大高度，但 Native Runtime 只有显式调用 `reportIntrinsicSize` 后才会填满，导致宿主已经分配的空间没有真正开放给用户代码。问题属于 PageCanvas/Runtime sizing owner，不属于 Menu 源码或 Overlay 样式。

## 已确认机制

- 单向关系保持为 `IntrinsicHeight -> RowHeight -> AllocatedHeight -> Runtime available`，禁止分配高度反向进入自然尺寸。
- `FrontstageAutoHeightBatch` 使用 `Map + Set + committed measurement queue`；稳定提交同时携带 Block、RenderIdentity 与 rows，即使新身份行数相同也会重新取得 measurement ownership。
- 只有当前 RenderIdentity 的自然测量稳定提交后，auto Block 才切换到 AllocationViewport；fixed Block 直接消费宿主分配空间。
- `reportIntrinsicSize` 仍只表达自然需求，不再作为是否可以使用宿主分配空间的开关。

## 验收候选

- 定向回归 5 files / 28 tests、TypeScript 和 Native React foundation fast receipt 通过；ESLint 0 error，1 条既有 Hook dependency warning 已落到 `tmp/test-governance/frontstage-sizing-contract/eslint.json`。
- 目标桌面页面：frame 764px、AllocationViewport 762px、Runtime available 738px，intrinsic record 434px；1.5 秒后仍为 434px，无反馈膨胀、无 page error。
- 390px 移动视口：AllocationViewport 432px、Runtime available 408px、intrinsic record 434px，1.5 秒稳定且无 page error。
- 无日历截止日期。用户确认目标页面效果后关闭 #1926；若需要改变 page protocol ABI、后端数据或持久化布局语义，返回 problem-framing。
