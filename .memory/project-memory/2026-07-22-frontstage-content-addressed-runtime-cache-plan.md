---
memory_type: project
topic: Frontstage 内容寻址 L1/L2 运行缓存规划
summary: #1382 的内容寻址缓存增量已实现并通过集中 QA：#1401 交付后端 source_sha256 与同会话 L1 BlockResult 零执行，#1402 交付 IndexedDB CompiledBlockArtifact；不分析静态/动态源码，不持久化运行数据，不改变 BlockModule.main(ctx)。
keywords:
  - frontstage
  - source_sha256
  - runtime_fingerprint
  - L1 render cache
  - L2 compiled artifact cache
  - IndexedDB
  - byte-weighted LRU
  - Worker runtime
match_when:
  - 规划或实现 #1382 的缓存 superseding delta
  - 修改 Frontstage 页面返回缓存、编译产物持久化或 Worker 请求协议
  - 调整 source_sha256、runtime_fingerprint、IndexedDB namespace 或登出清理
created_at: 2026-07-22 00
updated_at: 2026-07-22 03
last_verified_at: 2026-07-22 03
decision_policy: verify_before_decision
status: delivered_pending_user_acceptance
source_issue: "#1382"
depends_on_issue: "#1393"
delivery_issues:
  - "#1401"
  - "#1402"
scope:
  - api/apps/api-server/src/routes/frontstage
  - web/packages/api-client/src/console/frontstage.ts
  - web/packages/page-runtime
  - web/app/src/features/frontstage
  - web/app/src/state/auth-store.ts
---

# Frontstage 内容寻址 L1/L2 运行缓存规划

## 谁在做什么

Frontstage 已重构既有 Root #1382，而没有创建并行 Root。既有 #1383～#1386 保留为已集成历史；新 Delivery #1401 纵向交付后端权威 `source_sha256` 与同会话 L1 零执行恢复，#1402 纵向交付刷新级 L2 编译产物持久缓存。

## 为什么这样做

当前 #1382 的成功快照缓存命中后仍后台重跑，并把 TTL 当作有效性边界；这与用户确认的“导航不是刷新、同源码身份不因切页重新编译执行”冲突。当前 Worker 还会重复加载编译模块，刷新后无法复用已经完成的 TSX transform。

## 为什么要做

目标是在不改变 `BlockModule.main(ctx)`、不持久化 API 数据和用户运行状态的前提下，把同会话切页成本收敛到内存查找与 DOM 挂载，并让刷新后的已保存源码跳过重复编译，同时保持 actor/workspace 隔离、权限边界和冷启动恢复。

## 截止日期

未指定。实现已于 2026-07-22 完成并通过 Dev Acceptance Gate，等待用户最终验收 Root #1382。

## 决策背后动机

按执行阶段而不是源码行分类：整份源码统一编译为只由 `source_sha256 + runtime_fingerprint` 决定的程序产物；API Response、inputs、BlockResult、日志、effect 和运行状态只存在内存。必要复杂度由后端源码真值、Page Runtime 编译协议和 Frontstage 缓存 owner 分别吸收，不泄漏到页面路由或作者编程模型。

## 已确认边界

- L1 仅内存保存最新 `BlockResult`，按实例、`source_sha256` 和显式 dependency generation 命中；同会话切页命中时不创建 Worker、不编译、不发 API、不执行 `main`。
- L2 通过 IndexedDB 只持久化已保存源码的 `CompiledBlockArtifact`；不持久化 `BlockResult`、token、headers、API Response、context、outputs、logs、effects、interface calls 或用户运行状态。
- L2 使用 actor/workspace namespace、`runtime_fingerprint + source_sha256`、byte-weighted LRU；登出清理，默认不加密，缺失、损坏、quota 或浏览器回收时冷启动。
- 不做静态/动态源码分类，不新增 `load/render/loading` 或 `StaticBlockModule`，继续使用 `BlockModule.main(ctx)`。
- 分阶段观测 `source_fetch / worker_boot / compile / api_wait / main / schema_validate / present`；Compiler/Executor Worker 拆分只有测量证明必要时才重新决策，不在当前授权范围。
- 现有 #1382 AC-011 的后台 revalidate 与 AC-013 的 TTL 有效性语义被 superseding delta 取代；TTL 不再表示内容失效，资源容量由 LRU 管理。
- 不新增数据库 hash 列；`source_sha256` 是后端根据持久化源码原文计算并返回的派生 contract，源码仍是唯一真值。

## 当前计划状态

- 在线唯一真值：Root #1382；Delivery #1401、#1402 已实现并通过 Root 集中 QA。
- 计划形态：重构现有两层 Issue Tree Root #1382，新增两个纵向 Delivery；没有新建并行 Root。
- 既有 Delivery：#1383、#1384、#1385、#1386 已关闭并作为回归基础保留；#1385 已写入 superseded 评论且没有 reopen。
- 冻结 assembly：`c4ed23dbd841ff5ad6b9871ed58521f941649df6`；cycle 2 QA 结算 AC-017～024 全部 green。
- 集成结果：主工作树保持 `beta`，以 merge commit `991ed444f` 合入；用户既有 JSX Studio 与旧记忆脏改未进入 assembly、未被覆盖。
- 验收证据：后端 route 1/1、page-runtime 227/227、api-client 165/165；真实 Chrome 验证 cold/L2/A→B→A、动态 API sequence、七阶段 observation、IndexedDB canary clean、桌面与 390px。
- 剩余生命周期：push `beta` 后把 #1401/#1402 结算关闭，Root #1382 进入 `phase:user-acceptance`，由用户完成最终验收。
