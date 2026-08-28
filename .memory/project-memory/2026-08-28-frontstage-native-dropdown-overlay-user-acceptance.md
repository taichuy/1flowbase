---
memory_type: project
topic: Frontstage Native Block Dropdown Top Layer adapter
summary: 用户确认 #1915 采用 Block-scoped Top Layer Overlay；实现已完成定向 QA，当前等待用户验收 Dropdown 在 UI 模式切换与区块裁剪边界中的效果。
keywords:
  - issue 1915
  - native dropdown
  - top layer
  - popover
  - ShadowRoot
  - overlay lifecycle
match_when:
  - 实现或验收 #1915
  - 修改 Native Block Dropdown、overlay surface 或 layout epoch
  - 处理 popup 被 overflow/contain 裁剪或 UI 模式切换后无法打开
created_at: 2026-08-28 09
updated_at: 2026-08-28 09
last_verified_at: 2026-08-28 09
decision_policy: verify_before_decision
status: active
scope:
  - https://github.com/taichuy/1flowbase/issues/1915
  - web/app/src/features/frontstage/lib/native-modules/native-dropdown-runtime.tsx
  - web/app/src/features/frontstage/lib/native-modules/native-overlay-layer.ts
  - 01a045ce-dc56-7062-983e-2202347ba32e
---

# Frontstage Native Dropdown 等待用户验收

## 谁在做什么

当前开发会话已为 Native React Block 增加 `Dropdown` runtime adapter：trigger 与 Block React state 保持原 owner，popup 使用同一 ShadowRoot 内的浏览器 Top Layer；PageCanvas 只发布 `preview/design` layout epoch。

## 为什么这样做

原 popup 留在 Block ShadowRoot 的普通渲染层，会被设计态 `overflow: clip` 和 `contain: layout paint` 裁剪；UI 模式切换时 rc-trigger 的 hover intent 还可能停留在 hidden 状态。Top Layer 负责视觉逃逸，adapter 的受控 open transition 与 overlay generation 负责确定性恢复，Ant Design 继续拥有 placement、flip、菜单和 Escape 语义。

## 为什么要做

用户希望官方 Dropdown 示例无需修改即可在 Native Block 中可靠工作：关闭/开启 UI 模式后仍能打开，且下拉选项不再被区块底部遮挡。

## 验收候选与下一事件

无日历截止日期。认证 Chromium 已完成 10 次 UI 模式往返；设计态 popup 超出 Block 裁剪边界约 48px 仍完整可见，Resize 后 trigger/popup 间距保持 4px，菜单点击、Escape、SPA 卸载和 scroll-close cleanup 正常，无 page/console error。下一事件是用户刷新指定 Block 检查效果；确认后关闭 #1915。
