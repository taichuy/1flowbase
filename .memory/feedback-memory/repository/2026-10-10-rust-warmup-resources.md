---
memory_type: feedback
feedback_category: repository
topic: Rust 预热顺序与固定资源配置
summary: 保持共享资源配置；Rust 预热先获取依赖并分阶段错开普通库与库测试，单阶段采用一半可用逻辑 CPU 的预算。
keywords: [Rust, cargoJobs, cargoTestThreads, reset-rust-cache, 预热]
match_when: [诊断 Rust 编译耗时, 修改 Rust 缓存清理与预热流程]
created_at: 2026-10-10 10
updated_at: 2026-10-18 23
last_verified_at: 2026-10-18 23
decision_policy: direct_reference
scope: [scripts/node/reset-rust-cache, .1flowbase.verify.local.json]
---

## 规则

用户明确纠正：共享的构建和测试并发已经调好，不准因机器 CPU 数量或编译耗时擅自调整；预热入口的独立预算按下述明确授权处理。

2026-10-18 用户在体验串行和分阶段预热后，明确允许单个阶段使用一半 CPU：预热独立采用一半可用逻辑 CPU 的 Cargo 并发预算，先获取锁定依赖，再按顺序完成普通库和全量测试预编译，避免前后阶段竞争；不修改共享的 `cargoJobs` / `cargoTestThreads` 配置，保留内存保护。

源码开发环境仍需完整预热测试；先完成启动目标编译，再串行预热测试，不把取消测试预热作为优化结果。

## 原因

资源配置已有用户调优依据；缺少预热会让后续 AI 开发时的测试构建成本难以控制。

用户设置两个任务时考虑了多个 worktree 同时工作的资源需求，同时希望避免单核等待；需区分跨 worktree 构建任务和单次 Cargo 的内部并行预算，不把 CPU 空闲直接解释为资源限制失效，也不只为提升 CPU 占用而恢复大型目标同时编译。

## 适用场景

Rust 冷构建、缓存清理、预热脚本与 AI 开发验证成本优化。
