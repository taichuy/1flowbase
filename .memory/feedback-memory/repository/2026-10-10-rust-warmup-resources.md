---
memory_type: feedback
feedback_category: repository
topic: Rust 预热顺序与半机资源预算
summary: 开源开发环境须自动适配半数可用逻辑 CPU，不把当前机器算出的数值写成通用默认；预热先获取依赖再分阶段编译，保留内存保护。
keywords: [Rust, cargoJobs, cargoTestThreads, reset-rust-cache, 预热]
match_when: [诊断 Rust 编译耗时, 修改 Rust 缓存清理与预热流程]
created_at: 2026-10-10 10
updated_at: 2026-10-19 10
last_verified_at: 2026-10-19 10
decision_policy: direct_reference
scope: [scripts/node/reset-rust-cache, .1flowbase.verify.local.json]
---

## 规则

资源预算按用户明确授权调整，不仅因 CPU 空闲或编译耗时就继续扩大。

2026-10-18 用户在体验串行和分阶段预热后，明确允许单个阶段使用一半 CPU：预热独立采用一半可用逻辑 CPU 的 Cargo 并发预算，先获取锁定依赖，再按顺序完成普通库和全量测试预编译，避免前后阶段竞争。

2026-10-19 用户进一步明确授权把日常开发测试也开放到一半可用 CPU；共享 `cargoJobs` / `cargoTestThreads` 与系统 Cargo 包装器可据此调整，不再沿用此前固定为 2 的限制。保留多 worktree 的任务并发语义、预热阶段顺序及内存保护。

同日用户纠正：项目是开源开发环境，需要适配其他开发者的机器；半机 CPU 必须按运行环境自动计算，不能把当前 20 CPU 得到的 10 写成通用固定值。显式本地覆盖仍属于开发者自己的选择。

源码开发环境仍需完整预热测试；先完成启动目标编译，再串行预热测试，不把取消测试预热作为优化结果。

## 原因

资源配置已有用户调优依据；缺少预热会让后续 AI 开发时的测试构建成本难以控制。

用户设置两个任务时考虑了多个 worktree 同时工作的资源需求，同时希望避免单核等待；需区分跨 worktree 构建任务和单次 Cargo 的内部并行预算，不把 CPU 空闲直接解释为资源限制失效，也不只为提升 CPU 占用而恢复大型目标同时编译。

## 适用场景

Rust 冷构建、缓存清理、预热脚本与 AI 开发验证成本优化。
