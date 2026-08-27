---
memory_type: project
topic: Frontstage 新建 Block 使用响应式底部前沿
summary: 用户于 2026-08-27 确认 #1913 采用 bottom-frontier 分配未定位 Block；Ordered Tree 继续拥有文档顺序，响应式布局 allocator 按断点把新 Block 追加到全部已有行之后。
keywords:
  - issue 1913
  - bottom frontier
  - create block
  - responsive layout
  - missing y
  - automatic layout
match_when:
  - 实现或验收 #1913
  - 修改新建 Block 默认位置或缺失 x-layout 坐标的 fallback
  - 判断 Ordered Tree 顺序与响应式布局位置的所有权
created_at: 2026-08-27 23
updated_at: 2026-08-27 23
last_verified_at: 2026-08-27 23
decision_policy: verify_before_decision
status: active
scope:
  - https://github.com/taichuy/1flowbase/issues/1913
  - web/app/src/features/frontstage/lib/responsive-grid-layout.ts
  - web/app/src/features/frontstage/pages/frontstage-page/block-catalog-helpers.ts
  - web/app/src/features/frontstage/pages/FrontStagePage.tsx
---

# Frontstage Bottom Frontier 创建方向获批

## 谁在做什么

当前开发会话以 #1913 为新的 Single Issue，修复未定位 Block 使用 `index * 320px` 导致新建结果插入中间的问题；后续实现使用 test-driven-development 与 frontend-development。

## 为什么这样做

指定页面的后端证据显示新 Block 的 `rank/order` 已在末尾，但 descriptor 缺少各断点 `y`。前端 fallback 得到 `y=440`，小于已有后续行的 `815/1659`，自动行归一化因此把它排到上方。根因属于布局投影，不属于 Ordered Tree、viewport 或 Anchor Affix。

## 为什么要做

用户期望底部“创建区块”表达文档追加：不论前方 auto Block 多高、最后一行是否并排或当前滚动位置如何，新 Block 都应从全部已有行之后开始。

## 硬边界

- 每个断点使用 `F_b = max(y_i,b + h_i,b)`；未定位 Block 按文档顺序从 `F_b` 追加。
- 不以 viewport、scrollTop 或渐进渲染 demand 决定持久顺序。
- 不修改后端 Ordered Tree append 语义，不填上方空洞，不改写已有已定位坐标。
- 自动布局与自由布局都遵守底部追加；需要改变自由布局填洞语义时返回 discussion。

## 截止与下一事件

无日历截止日期。#1913 当前为 `phase:ready`；下一事件是用户明确开始实现后执行 AC-001～008，完成集中 Dev Acceptance 后进入用户验收。
