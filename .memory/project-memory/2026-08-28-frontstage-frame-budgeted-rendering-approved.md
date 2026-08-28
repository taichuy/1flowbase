---
memory_type: project
topic: Frontstage 帧预算增量渲染协调器已批准
summary: 用户批准的 Frontstage 帧预算调度与稳定尺寸提交已实现并通过定向 QA，#1925 等待真实页面用户验收。
keywords:
  - Frontstage
  - Native Block
  - scheduler
  - ResizeObserver
  - content-visibility
  - performance
created_at: 2026-08-28 18
updated_at: 2026-08-28 18
last_verified_at: 2026-08-28 18
decision_policy: verify_before_decision
scope:
  - web/app/src/features/frontstage
  - issue:1925
---

# 当前决策

- Root agent 后续按 Single Issue [#1925](https://github.com/taichuy/1flowbase/issues/1925) 实现 Frontstage 帧预算增量渲染协调器。
- 用户已批准：输入优先于 visible update / prepare / preload / background；ResizeObserver 只采样到 PendingSizeMap，稳定后一次提交 PageCanvas 布局；按 RenderIdentity 积累尺寸；安全使用 `content-visibility` 与 intrinsic size。
- 目的：已 ready Native Block 的内部交互不再被邻近 compile/mount 抢占，内部 motion 不再逐帧触发全局网格 compaction，离屏实例保留 state 但减少浏览器 style/layout/paint。
- 只有 cooperative scheduling 仍留下不可切分且超过阈值的同步编译时，才进入 Issue 已授权的 Worker 路径；ABI、artifact 格式、RGL 替换或后端 contract 变化必须重新讨论。
- 截止日期：未指定；以 #1925 AC-001～AC-009、定向 QA 和用户真实页面验收为完成条件。

# 动机

问题属于浏览器主线程竞争与动态几何反馈的通用场景，不接受 Menu 专用补丁或单纯 debounce；方案借鉴 React Scheduler、TanStack Virtual、react-window、fastdom 与 WICG Scheduling APIs 的成熟机制，并由当前 Runtime/PageCanvas owner 吸收复杂度。

# 实现与验收结果

- preparation 的 compile、module resolve 和 ready/mount commit 都经过 interaction lease 与 `scheduler.postTask`/timer fallback 准入；abort 与 generation fencing 保留。
- 自动高度以 `Map + Set` 保存 RenderIdentity、observed/committed rows、稳定帧和变更时间；250ms 静默窗覆盖主线程拥塞造成的稀疏 ResizeObserver 样本，PageCanvas 只提交稳定终值。
- `RenderPlanSlot` 使用 memo 隔离无关 ready Block tree；既有 `content-visibility:auto` 与 intrinsic size 合同保留。
- 2026-08-28 定向测试 7 files / 49 tests、TypeScript、diff check 通过；ESLint 0 error，3 条既有 Hook dependency warning 已进入 `tmp/test-governance/frontstage-frame-budget/eslint.json`。
- 独立 Event Timing 的 20 次 Menu 点击 P95 duration 40ms、processing delay 24.1ms，点击窗口无 Long Task；Trace 显示相邻 preparation 在点击窗口结束后才推进。目标高度只提交 296/434 终值。
- 编译原本已运行在 Worker；未命中新增 Worker 的停止条件。Issue 保持打开并进入用户验收，真实页面确认后方可关闭。
