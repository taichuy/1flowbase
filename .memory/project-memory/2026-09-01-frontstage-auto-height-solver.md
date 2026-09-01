---
memory_type: project
topic: Frontstage 自动高度采用 Measure Solve Arrange 求解协议
summary: 用户确认以纯计算 Auto Layout Solver 修复 Frontstage intrinsic height 与 allocated height 混用导致的非确定性收敛；GitHub Single Issue #1966 已进入 implementation。
keywords:
  - Frontstage
  - auto height
  - intrinsic sizing
  - Measure Solve Arrange
  - issue 1966
created_at: 2026-09-01 07
updated_at: 2026-09-01 08
decision_policy: verify_before_decision
status: completed
scope:
  - web/app/src/features/frontstage
---

# Frontstage Auto Height Solver

- 谁在做什么：Root agent 按 GitHub Single Issue `#1966` 在 Frontstage 内实现纯计算 Auto Layout Solver，并用定向 TDD 与 Playwright 重复加载证据结算验收点。
- 为什么这样做：现有自动高度把自然尺寸与父布局分配尺寸混为同一测量值，形成非收缩反馈环；相同页面重复加载会随机锁定在 `176px / 320px` 等高度。
- 为什么要做：用户要求基础区块布局采用成熟、长期可演进的 Measure → Solve → Arrange 协议，而不是继续修补局部 owner 判断。
- 截止日期：本轮连续实现并进入 QA；若需要改变持久化 schema、自由网格语义或后端 contract，则停止并返回 discussion。
- 决策动机：Block/runtime 只拥有 intrinsic contribution，solver 拥有同行约束与几何，`react-grid-layout` 只负责拖拽、resize、collision 与像素投影，从结构上保证确定性、幂等性、单调性和 epoch 隔离。
- QA 状态：2026-09-01 08 Dev Acceptance Gate 通过。定向 21 项算法 / 布局测试、组件 AC-1926-002、TypeScript 与 `git diff --check` 通过；真实页面在 4 个 Block ready 后按连续 5 个 100ms 几何样本稳定取证，10 次重复加载只有 1 个最终签名 `47/59/47/47px`，无 `176/320px` 锁定。实现于 `70a82fb96` 提交并推送到 `dev`，GitHub Issue #1966 已评论验收证据并关闭；3300 本地 web 容器已替换当前生产包并重启。
