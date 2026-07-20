---
memory_type: project
topic: Frontstage 代码区块浏览器运行时编排方向
summary: 用户确认保持“浏览器编译 + Worker 执行”，先以状态机、有界调度、可视区懒运行和短期会话加速解决误超时与浏览器资源竞争；本期不迁移后端编译或后端执行。
keywords:
  - frontstage
  - code block
  - browser compilation
  - web worker
  - runtime coordinator
  - state machine
  - bounded scheduling
match_when:
  - 规划或实现 Frontstage 代码区块运行时、超时、Worker 调度、可视区懒运行或页面返回加速
  - 重新评估后端预编译、SharedWorker、Workbox 或 IndexedDB 是否应进入当前范围
created_at: 2026-07-20 10
updated_at: 2026-07-20 10
last_verified_at: 2026-07-20 10
decision_policy: verify_before_decision
status: root_ready
source_issue: "#1382"
delivery_issues:
  - "#1383"
  - "#1384"
  - "#1385"
scope:
  - web/packages/page-runtime
  - web/app/src/features/frontstage
  - web/packages/block-renderer
---

# Frontstage 代码区块浏览器运行时编排方向

## 谁在做什么

Frontstage 将继续在浏览器 Worker 中编译并执行用户 TSX/JSX，通过页面级 Runtime Coordinator 负责需求优先级、并发、取消、短期缓存与 Worker 生命周期；`@1flowbase/page-runtime` 负责区块运行协议、状态机和分阶段失败语义。

## 为什么这样做

当前简单 Demo 的 `runtime_timeout` 主要来自 1000ms 预算同时覆盖 Worker 冷启动、调度、编译、执行与 effect，而不是编译位置本身。迁移后端编译不能消除 Worker 启动、执行与 effect 成本，还会给 Rust 后端引入新的编译服务和 Artifact 版本治理。

## 为什么要做

需要在不改变静态前端部署形态的前提下，让首屏区块优先、离屏区块延迟、并发受控、旧请求可取消、页面短期返回可恢复，并保留无限循环硬终止和区块级加载边界。

## 截止日期

无预设日期；Root Issue #1382 与三个 Delivery #1383、#1384、#1385 已于 `2026-07-20 10` 进入 `phase:ready`，全部结果以 Root 集中 QA 和用户验收结算。

## 决策背后动机

先把真实复杂度放回能够观察并控制它的 owner：页面级 Coordinator 决定“何时运行谁”，区块 Runtime 决定“如何安全运行”。后端 AOT 编译、后端执行、SharedWorker、Workbox、IndexedDB 编译产物缓存和节点级 Schema 流均不进入当前范围，只有阶段耗时与使用规模形成明确证据后再单独评估。

## 在线计划真值

- Root：`https://github.com/taichuy/1flowbase/issues/1382`
- Delivery 1：`https://github.com/taichuy/1flowbase/issues/1383`
- Delivery 2：`https://github.com/taichuy/1flowbase/issues/1384`
- Delivery 3：`https://github.com/taichuy/1flowbase/issues/1385`
