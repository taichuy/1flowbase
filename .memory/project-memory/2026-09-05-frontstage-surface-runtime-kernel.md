---
memory_type: project
topic: Frontstage Native Block Surface Runtime Kernel
summary: 用户确认并完成小而深的 retained-mode Surface Runtime Kernel；以 Surface 统一样式、滚动、浮层几何和 stale commit 边界，不自研布局引擎、不 patch Ant Design。
keywords:
  - Frontstage
  - Native Block
  - Surface Runtime Kernel
  - ShadowRoot
  - overlay geometry
  - reveal
  - issue 1989
created_at: 2026-09-05 01
updated_at: 2026-09-05 01
last_verified_at: 2026-09-05 01
decision_policy: verify_before_decision
status: user-acceptance
scope:
  - web/app/src/features/frontstage/lib/native-modules
  - web/app/src/features/frontstage/lib/native-trusted-block-react-adapter.tsx
  - web/packages/page-protocol/src/block-context.ts
  - web/scripts/run-surface-runtime-kernel-browser-acceptance.mjs
---

# Frontstage Surface Runtime Kernel

- 谁在做什么：Root agent 按 GitHub Root Issue `#1989` 与 Delivery `#1990/#1991` 完成 Native Block Surface Runtime Kernel，并已将冻结 candidate `083e4592c7f327318d5ad3469c5a1dd2658e2948` fast-forward 合入、推送 `dev`；Root 等待用户真实页面验收。
- 为什么这样做：两个 Block 未渲染的共同根因跨越 ShadowRoot 样式归属、局部 scroll owner、浮层 anchor 更新与异步 commit 时序，逐 Block 打补丁无法守住共同运行时不变量。
- 为什么要做：低代码 Native React 区块需要稳定支持 artifact-local CSS、Popover/Tooltip 定位和局部 reveal，同时保持 Ant Design/rc-trigger 的公开 placement 所有权与动态模块 identity 一致。
- 截止日期：2026-09-05 已完成 Dev Acceptance；用户验收后关闭 Root `#1989`。
- 决策背后动机：采用 retained-mode 小内核统一 composed-tree traversal、observer/listener lifecycle、dirty anchor Set、single-rAF Measure → Commit、generation/dispose stale rejection 与 `ctx.ui.surface.reveal(target)`；不引入 R-tree、自研布局引擎或数学 DSL，不 patch/fork Ant Design、antd-style、rc-trigger，不依赖私有 ref、`.ant-*` class、DOM index 或固定 sleep。
- 动态模块边界：`ContextProducerModule = ContextConsumerModule ∨ boundary uses explicit capability`。
- 范围边界：Popover/Tooltip 通过公开 `TooltipRef.nativeElement/forceAlign` 获得新增 realign 保证；Dropdown 只保留既有 adapter 回归，不承诺没有公开 API 支撑的强制 realign。
- Block 结果：Static Block `01a06a11-326d-7291-afdb-aeee729183f0` 源码不变；Reveal Block `01a06a11-33ab-7f80-b106-312b7d34f5ac` 使用 `ctx.ui.surface.reveal(trigger)`，不再写 `document.documentElement`。
- QA 状态：Centralized QA cycle 9 为 `QA_PASS`；App 79/79、page-protocol 24/24、build green、AC-001～AC-008 全 green。认证浏览器中 Static/Reveal 的 scroll 与 resize 相对误差均为 `0px`，document 始终 `(0,0)`，page/console errors 为 0。full workspace、coverage、正式 foundation receipt 未运行，按 Dev Acceptance 范围不阻断。
