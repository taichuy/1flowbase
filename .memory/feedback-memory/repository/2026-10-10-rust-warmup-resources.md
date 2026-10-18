---
memory_type: feedback
feedback_category: repository
topic: Rust 预热顺序与固定资源配置
summary: 保持共享资源配置；Rust 预热分阶段错开大型普通库与库测试，阶段内部使用配置的有限并行。
keywords: [Rust, cargoJobs, cargoTestThreads, reset-rust-cache, 预热]
match_when: [诊断 Rust 编译耗时, 修改 Rust 缓存清理与预热流程]
created_at: 2026-10-10 10
updated_at: 2026-10-18 22
last_verified_at: 2026-10-18 22
decision_policy: direct_reference
scope: [scripts/node/reset-rust-cache, .1flowbase.verify.local.json]
---

## 规则

用户明确纠正：构建和测试并发已经调好，不准因机器 CPU 数量或编译耗时擅自调整。沿用现有本地资源配置。

2026-10-18 用户在体验全程单任务预热后，确认继续优化并实际测试：大型普通库与库测试分阶段错开，阶段内部恢复配置中的有限并行；不再将全程固定 `jobs=1` 作为最终方案，不修改共享的 `cargoJobs` / `cargoTestThreads` 配置。

源码开发环境仍需完整预热测试；先完成启动目标编译，再串行预热测试，不把取消测试预热作为优化结果。

## 原因

资源配置已有用户调优依据；缺少预热会让后续 AI 开发时的测试构建成本难以控制。

用户设置两个任务时考虑了多个 worktree 同时工作的资源需求，同时希望避免单核等待；需区分跨 worktree 构建任务和单次 Cargo 的内部并行预算，不把 CPU 空闲直接解释为资源限制失效，也不只为提升 CPU 占用而恢复大型目标同时编译。

## 适用场景

Rust 冷构建、缓存清理、预热脚本与 AI 开发验证成本优化。
