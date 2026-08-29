---
memory_type: project
topic: Frontstage Native Block Dropdown Top Layer adapter
summary: 用户确认 Block-scoped Top Layer Overlay；#1931 已把 Dropdown/Menu 的组件级 layer 收敛为 surface 统一 Host，并覆盖通用 AntD rc-trigger 浮层。
keywords:
  - issue 1915
  - issue 1923
  - issue 1924
  - issue 1928
  - issue 1931
  - native dropdown
  - top layer
  - popover
  - ShadowRoot
  - overlay lifecycle
match_when:
  - 实现或验收 #1915
  - 修改 Native Block Dropdown、overlay surface 或 layout epoch
  - 修改 Native Block Menu popupRender、getPopupContainer 或 openKeys
  - 处理 popup 被 overflow/contain 裁剪或 UI 模式切换后无法打开
  - 处理 Selection Range 虚拟锚点受 RGL transform 坐标偏移
created_at: 2026-08-28 09
updated_at: 2026-08-29 11
last_verified_at: 2026-08-29 11
decision_policy: verify_before_decision
status: user-acceptance
scope:
  - https://github.com/taichuy/1flowbase/issues/1915
  - https://github.com/taichuy/1flowbase/issues/1923
  - https://github.com/taichuy/1flowbase/issues/1924
  - https://github.com/taichuy/1flowbase/issues/1928
  - https://github.com/taichuy/1flowbase/issues/1931
  - web/app/src/features/frontstage/lib/native-modules/native-dropdown-runtime.tsx
  - web/app/src/features/frontstage/lib/native-modules/native-overlay-layer.ts
  - web/app/src/features/frontstage/lib/native-modules/menu/native-menu-runtime.tsx
  - 01a045ce-dc56-7062-983e-2202347ba32e
  - 01a04631-7dd6-7352-b711-1fd1a1df44c8
---

# Frontstage Native Dropdown 等待用户验收

## 谁在做什么

当前开发会话已为 Native React Block 增加 `Dropdown` runtime adapter：trigger 与 Block React state 保持原 owner，popup 使用同一 ShadowRoot 内的浏览器 Top Layer；PageCanvas 只发布 `preview/design` layout epoch。

#1923 在此基础上把 `position: fixed + aria-hidden + pointer-events: none` 定义为 viewport virtual trigger。Adapter 使用 authored `left/top` 作为 viewport 真值，并根据真实 `getBoundingClientRect()` 反馈修正 transformed containing block 偏移；滚动、resize 与 layout epoch 会重新归一化，普通 DOM trigger 不进入该路径。

用户首轮实际复验仍失败后，已撤销“程序化 Range + 合成 mouseup”的错误浏览器绿灯。真实鼠标选择证明 Chromium 会把外层 `window.getSelection()` 的 Range retarget 为 `0×0`，而 `ShadowRoot.getSelection()` 保留有效矩形。Page Runtime 浏览器能力绑定现在只在外层 Range 不可渲染且选中文本匹配时选择 Native Block ShadowRoot Selection；普通页面 Selection 保持原值。

#1924 进一步确认一级 Dropdown 通过组件 prop 进入 Top Layer，但 Menu 的级联 submenu 会回退到外层 `ConfigProvider.getPopupContainer` 并直接挂到 ShadowRoot。`NativeBlockDropdown` 现使用 Ant Design 局部 `ConfigProvider`，让根 popup 与全部子级 popup 共享同一 `NativeOverlayLayer.container`，不新增自定义 overlay manager。

#1928 将同一个容器能力扩展到原生 AntD `Menu`：Runtime registry 导出 `NativeBlockMenu`，默认把水平/垂直子菜单与 `popupRender` 放入当前 Block 的 Top Layer；作者显式 `getPopupContainer` 仍优先，AntD/rc-trigger 继续负责 placement 与视口翻转。Adapter 保留 `openKeys`、`onOpenChange`、ref 与静态成员合同，并在 layout epoch 变化时清理非受控展开状态。

#1931 将组件级能力收敛为每个 Native Block surface 唯一的 `NativeOverlayHost`：顶层 ConfigProvider 默认把 Cascader、Select、TreeSelect、DatePicker、Tooltip、Popover 等 rc-trigger 浮层路由到同一个 ShadowRoot 内 Browser Top Layer；Dropdown/Menu 复用该 Host，不再创建第二层 owner。Host 观察容器内 popup 的实际可见性来控制 Top Layer，多个 popup 并存时保持开启，最后一个关闭或 surface 卸载时确定性回收；作者显式容器仍优先。

## 为什么这样做

原 popup 留在 Block ShadowRoot 的普通渲染层，会被设计态 `overflow: clip` 和 `contain: layout paint` 裁剪；UI 模式切换时 rc-trigger 的 hover intent 还可能停留在 hidden 状态。Top Layer 负责视觉逃逸，adapter 的受控 open transition 与 overlay generation 负责确定性恢复，Ant Design 继续拥有 placement、flip、菜单和 Escape 语义。

## 为什么要做

用户希望官方 Dropdown 示例无需修改即可在 Native Block 中可靠工作：关闭/开启 UI 模式后仍能打开，且下拉选项不再被区块底部遮挡。

用户进一步要求官方划词 Dropdown 示例无需理解 RGL transform 或宿主坐标系；选区 DOMRect 应直接锚定可见菜单，而不是由每个 Block 手工减宿主偏移。

## 验收候选与下一事件

无日历截止日期。#1915 的 UI 模式往返与裁剪证据保持有效；#1923 的最终认证 Chromium 真实双击证据显示 Shadow Selection center / bottom target `(275.015625, 192)` 与 trigger 完全一致，误差为 0，菜单由真实 `mouseup` 后进入 open Top Layer，且无 page error。用户已于 2026-08-28 16 确认问题解决，#1923 已关闭。

#1924 的认证 Chromium 边界 fixture 将设计态裁剪边界压到 `bottom=297`，submenu 实际延伸到 `bottom=362` 后仍完整可见；子菜单 DOM 父链包含当前 Block 的 NativeOverlayLayer，选择子项后 layer 回到 closed，且无 page error。用户已于 2026-08-28 确认修复，Issue 已关闭。

#1928 的真实 Block `01a047b3-4e31-7250-93c2-c5bc73214fe8` 浏览器证据显示自定义 Menu popup 为 `761×245`，属于 open Top Layer 且完整位于视口内。触发器下方仅余 15px，因此 rc-trigger 正确向上翻转；这不是裁剪回归，也不应由 Runtime 强制改为向下。Issue 已进入用户验收。

#1931 的真实 Block `2b6e34f3-2d9c-4504-a625-32305f312f0a` 浏览器证据显示 Cascader popup 进入 open Top Layer，并越过 Block 右边界；UI 模式往返后可再次打开，page/console errors 均为 0。定向回归 5 files / 30 tests、Native React foundation fast pack 180 tests、TypeScript、ESLint、Prettier 与 diff check 均通过。无日历截止日期；Issue 进入 `phase:user-acceptance`，等待用户刷新页面验收后关闭。
