---
memory_type: feedback
feedback_category: interaction
topic: Frontstage 性能诊断先区分宿主与区块内部交互
summary: 用户描述页面卡顿时，应先确认是加载滚动、区块生命周期，还是 ready 后区块内部组件点击；不得用冷加载 Trace 代替内部交互证据。
keywords:
  - frontstage
  - native block
  - interaction latency
  - Playwright
  - performance
match_when:
  - 诊断 Frontstage 页面或 Native Block 卡顿
  - 用户描述区块内部 AntD 组件点击、切换或动画不流畅
created_at: 2026-08-28 18
updated_at: 2026-08-28 18
last_verified_at: 2026-08-28 18
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage
  - scripts/node/page-debug
---

# 规则

性能取证必须锁定用户实际交互：等待目标 Block `ready` 后再录制 `pointerdown/click → next paint`；同步记录其他 Block 的 preparation 状态迁移，区分组件自身更新、宿主尺寸反馈与后台 compile/mount 竞争。

# 原因

冷加载、滚动渐进渲染与 ready 后 Menu 点击是不同性能路径。混用 Trace 会把网络、编译和首次挂载成本错误归因给区块内部组件。

# 适用场景

- Frontstage 页面加载正常，但 Native Block 内部组件点击或切换卡顿。
- 需要判断问题 owner 属于用户源码、Runtime Adapter、PageCanvas 或 preparation scheduler。
