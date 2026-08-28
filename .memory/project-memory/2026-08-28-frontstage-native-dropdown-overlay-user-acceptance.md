---
memory_type: project
topic: Frontstage Native Block Dropdown Top Layer adapter
summary: 用户确认 #1915 采用 Block-scoped Top Layer Overlay，#1923 进一步由 Runtime Adapter 归一化 fixed virtual trigger 的 viewport 坐标；两项实现均已完成定向 QA，当前等待用户验收。
keywords:
  - issue 1915
  - issue 1923
  - native dropdown
  - top layer
  - popover
  - ShadowRoot
  - overlay lifecycle
match_when:
  - 实现或验收 #1915
  - 修改 Native Block Dropdown、overlay surface 或 layout epoch
  - 处理 popup 被 overflow/contain 裁剪或 UI 模式切换后无法打开
  - 处理 Selection Range 虚拟锚点受 RGL transform 坐标偏移
created_at: 2026-08-28 09
updated_at: 2026-08-28 16
last_verified_at: 2026-08-28 16
decision_policy: verify_before_decision
status: active
scope:
  - https://github.com/taichuy/1flowbase/issues/1915
  - https://github.com/taichuy/1flowbase/issues/1923
  - web/app/src/features/frontstage/lib/native-modules/native-dropdown-runtime.tsx
  - web/app/src/features/frontstage/lib/native-modules/native-overlay-layer.ts
  - 01a045ce-dc56-7062-983e-2202347ba32e
  - 01a04631-7dd6-7352-b711-1fd1a1df44c8
---

# Frontstage Native Dropdown 等待用户验收

## 谁在做什么

当前开发会话已为 Native React Block 增加 `Dropdown` runtime adapter：trigger 与 Block React state 保持原 owner，popup 使用同一 ShadowRoot 内的浏览器 Top Layer；PageCanvas 只发布 `preview/design` layout epoch。

#1923 在此基础上把 `position: fixed + aria-hidden + pointer-events: none` 定义为 viewport virtual trigger。Adapter 使用 authored `left/top` 作为 viewport 真值，并根据真实 `getBoundingClientRect()` 反馈修正 transformed containing block 偏移；滚动、resize 与 layout epoch 会重新归一化，普通 DOM trigger 不进入该路径。

## 为什么这样做

原 popup 留在 Block ShadowRoot 的普通渲染层，会被设计态 `overflow: clip` 和 `contain: layout paint` 裁剪；UI 模式切换时 rc-trigger 的 hover intent 还可能停留在 hidden 状态。Top Layer 负责视觉逃逸，adapter 的受控 open transition 与 overlay generation 负责确定性恢复，Ant Design 继续拥有 placement、flip、菜单和 Escape 语义。

## 为什么要做

用户希望官方 Dropdown 示例无需修改即可在 Native Block 中可靠工作：关闭/开启 UI 模式后仍能打开，且下拉选项不再被区块底部遮挡。

用户进一步要求官方划词 Dropdown 示例无需理解 RGL transform 或宿主坐标系；选区 DOMRect 应直接锚定可见菜单，而不是由每个 Block 手工减宿主偏移。

## 验收候选与下一事件

无日历截止日期。#1915 的 UI 模式往返与裁剪证据保持有效；#1923 的认证 Chromium 证据显示 Selection center `(275.015625, 327)` 与归一化 trigger `(275.015625, 327)` 的误差为 0，菜单已进入 open Top Layer。下一事件是用户刷新指定 Block 检查划词效果；确认后关闭 #1923。
