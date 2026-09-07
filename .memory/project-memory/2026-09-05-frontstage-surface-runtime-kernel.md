---
memory_type: project
topic: Frontstage Native Block Surface Runtime Kernel
summary: 用户确认以 Effect Declaration → Least Sufficient Realm → Host-mediated Capabilities 延续小而深的 Surface Runtime Kernel；Native 由 surface 持有滚动、浮层几何与样式隔离，独立 viewport 继续使用 isolated iframe。
keywords:
  - Frontstage
  - Native Block
  - Surface Runtime Kernel
  - ShadowRoot
  - overlay geometry
  - reveal
  - issue 1989
  - issue 1997
  - Least Sufficient Realm
created_at: 2026-09-05 01
updated_at: 2026-09-07 15
last_verified_at: 2026-09-07 15
decision_policy: verify_before_decision
status: user-acceptance
scope:
  - web/app/src/features/frontstage/lib/native-modules
  - web/app/src/features/frontstage/lib/native-trusted-block-react-adapter.tsx
  - web/packages/page-protocol/src/block-context.ts
  - web/packages/page-protocol/src/frontend-block-contract.ts
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

## 2026-09-07 Affix 组合收敛

- 用户批准 Issue `#1997` 并要求直接实现：把 `native_react` 固定映射为 `trusted_host_realm / surface positioning / content sizing`，把 `isolated_iframe` 固定映射为 `independent_realm / realm viewport / host sizing`；不新增第三套 Runtime 或持久化字段。
- Native Affix 的 bottom placement 改由 surface-local 绝对几何确定；每个 Affix ShadowRoot 获得独立 Ant Design style scope，避免同批次相同 CSS path 只注入第一个根。
- 指定 Affix demo 页四个 Block 已改为有限本地滚动容器与显式 target，移除 `height: 10000`。桌面和移动端运行证据、定向测试、协议测试、lint 与 diff check 均通过。
- `#1997` 已转为 `phase:user-acceptance`，未关闭、未 commit/push；完整 frontend build、全量 style-boundary 与全仓门禁留给 CI/beta。

## 2026-09-07 Notification Effect Resource

- 用户确认本轮直接实现 Native Trusted Block Notification：不再按局部变量名禁止 `notification`，保留标准 `App.useApp()` / `notification.useNotification()` 作者 API；只拒绝绕过 React 上下文的 Ant Design 静态 Notification。
- Frontstage 由 Block Surface 统一持有 Notification / Message Effect Resource，在 `layoutEpoch` 变化时失效、卸载时反向回收；单个资源失败不阻断其余资源清理，完成清理后聚合报告错误。
- 动机是把副作用生命周期与 Block Surface 的 generation/epoch fence 对齐，避免全局 holder、跨布局残留和变量名误判；不新增私有 `ctx.ui.notification`，本轮即时完成并进入用户验收。
- QA：Page Runtime 16 files / 171 tests、Native Block 12 files / 101 tests、App TypeScript 与 diff check 通过；真实 Block `01a07765-88b5-7370-aa22-26d4c3831413` 经 MCP 确认为 `const { notification } = AntdApp.useApp()` 场景。
