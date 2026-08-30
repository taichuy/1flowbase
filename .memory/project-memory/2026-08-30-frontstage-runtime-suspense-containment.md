---
memory_type: project
topic: Frontstage Native Runtime 异步依赖隔离与单调生命周期
summary: 每个 Native Block Portal 以局部 Suspense 形成 Runtime Cell；图标叶子模块用 Promise single-flight 合并，其他 Block 的 lazy 依赖不得让 ready Block 回退。
keywords:
  - frontstage
  - native runtime
  - suspense
  - single-flight
  - render identity
  - issue 1950
created_at: 2026-08-30 10
updated_at: 2026-08-30 10
last_verified_at: 2026-08-30 10
decision_policy: verify_before_decision
scope:
  - web/app/src/features/frontstage/lib/native-trusted-block-react-adapter.tsx
  - web/app/build/native-ant-design-icons-modules.ts
  - issue 1950
---

# Frontstage Native Runtime 异步依赖隔离与单调生命周期

## 当前阶段

`#1950` 已由 Root agent 实现并推送 `dev`，当前处于 `phase:user-acceptance`。用户负责在实际页面体验滚动往返；没有截止日期。

## 决策

Native Runtime Adapter 是 Block 异步悬挂的 owner。每个 Native Block Portal 使用局部 Suspense，Block 内部 lazy 模块不得传播到页面级 Suspense。图标叶子模块按 `moduleSource` 使用 `Map` 保存当前 Promise flight；并发调用共享结果，拒绝 flight 从 Map 移除后允许重试。Scheduler 继续用既有 generation、abort、current task/epoch 判定拒绝过期提交，不建立第二套 token。

## 动机

目标页面曾在下方 Block 首次加载图标时，把上方已 ready Block 从 `ready` 拉回 `module_resolve / artifact_lookup / idle`，造成 Runtime root 重建、重复 code/catalog/tabs 请求和视觉闪烁。滚动只是触发条件，根因是异步依赖越过 Block 隔离边界。

## 验证基线

Playwright 证据表明修复后顶部三个 Runtime root node id 在滚动到底部再返回时保持 `2 / 4 / 6`，状态保持 `ready / generation=0`；初始稳定后只请求最后一个未加载 Block 的 code 一次。定向测试、Native React fast foundation pack、TypeScript、ESLint（0 error）和 production build 已通过。后续若局部 Suspense 下仍发生 Scheduler 重建，停止扩大模块缓存，转而追踪第二个生命周期 owner。
