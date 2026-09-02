---
memory_type: project
topic: Frontstage 累积式渐进渲染
summary: Frontstage 累积式渐进渲染已完成实现与集中回归；活动 Single Issue #1896 进入 phase:qa，AC-008 的离屏 layout/paint 收益仍缺少正向浏览器证据。
keywords:
  - frontstage
  - accumulative-rendering
  - progressive-rendering
  - viewport-demand
  - native-react
  - issue-1896
match_when:
  - 实现或评估 Frontstage 区块渐进渲染
  - 修改 Native React 区块 mountIntent、Runtime Demand 或生命周期
  - 排查 Frontstage 滚动重挂载、抖动、自动高度或滚动锚点
created_at: 2026-08-26 17
updated_at: 2026-08-26 18
last_verified_at: 2026-08-26 18
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1896
  - web/app/src/features/frontstage
  - web/packages/page-runtime
---

# Frontstage 累积式渐进渲染

## 谁在做什么

- 用户在 `2026-08-26 17` 确认采用平衡方向，并要求将计划挂到线上 Issue。
- AI 已完成 Single Issue [#1896](https://github.com/taichuy/1flowbase/issues/1896) 的实现和集中回归；Issue 推进到 `phase:qa`，不关闭，等待剩余性能证据与用户验收。

## 为什么这样做

- 当前 Runtime Demand priority 2/3 会撤销已经 ready 区块的 `mountIntent`，造成 Portal、React state、effect、DOM 和第三方组件实例被销毁，滚回时重新挂载。
- 自动高度从固定估值经过 Loading Shell 再到真实内容，多次写入布局并重新压排，造成滚动漂移和视觉抖动。
- 现有系统已经积累编译 artifact、组件工厂和共享样式，但没有积累最接近用户体验的活实例、运行状态与稳定布局成果。

## 为什么要做

- 让已经呈现的区块在页面会话内复用活实例、状态和已测高度，避免浏览器重复初始化与布局绘制。
- 保留沙箱/ShadowRoot 隔离边界，同时把 Lifecycle、Demand 与布局稳定职责放回各自 owner。

## 已确认决策与动机

- Lifecycle 与 Demand 正交；滚动只改变首次准备优先级，不使 `mounted` 生命周期回退。
- 同一 Render Identity 下 inputs、signals、props、主题、语言、宽度和可视区变化只 update，不 remount。
- 页面离开、删除、source/ABI/permission revision 变化和显式刷新才允许确定性 dispose/new generation。
- 自动高度使用离散 grid rows、帧级批处理、已知高度缓存与滚动锚点守恒。
- 验证 `content-visibility` / intrinsic size 是否能在保留活实例时降低离屏渲染工作；不成立时停止该优化层，不回退为离屏卸载。
- 本 Issue 不引入活实例 LRU/CLOCK、React state 序列化、DOM 快照恢复、后端 contract 或持久化 migration。

## 截止日期

- 未指定。#1896 在全部 AC 有证据、集中 QA 通过并由用户最终验收前保持活动。
- 若需要改变权限/安全边界、后端 contract、持久化历史高度，或引入 iframe/Worker 统一冻结恢复协议，回到 `problem-framing`。

## 2026-08-26 18 阶段结果

- 已实现 sticky mount intent、稳定 `ctx.state`、Signal coordinator 增量协调、Schmitt-trigger Hysteresis、Map 帧批处理离散行高、基于 `verticalCompactor` 行坐标差的滚动锚定，以及 intrinsic height containment。
- 定向回归为 7 个测试文件 42 项全绿；生产构建通过；原有 Native/isolated/public-modules Playwright 桌面与移动验收通过。
- #1896 专用长页面 Playwright 中 priority `1 → 2 → 3 → 1` 不新增 mount/unmount，Hook identity 不变；桌面锚点漂移 `0.94px`，390px 漂移 `0px`。
- `content-visibility:auto` 与 intrinsic size 正确生效且无高度塌陷，但 Chromium 对照未观察到 skipped subtree；Layout/RecalcStyle/Task 指标也没有稳定正向差异。因此 AC-008 只结算“无 remount、无振荡、containment contract”，离屏 layout/paint 降幅仍为待验证，不能进入 `phase:user-acceptance`。
