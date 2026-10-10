---
memory_type: feedback
feedback_category: repository
topic: Rust 预热顺序与固定资源配置
summary: 保持用户已调好的构建和测试并发，先编译启动目标再完整预热测试。
keywords: [Rust, cargoJobs, cargoTestThreads, reset-rust-cache, 预热]
match_when: [诊断 Rust 编译耗时, 修改 Rust 缓存清理与预热流程]
created_at: 2026-10-10 10
updated_at: 2026-10-10 10
last_verified_at: 2026-10-10 10
decision_policy: direct_reference
scope: [scripts/node/reset-rust-cache, .1flowbase.verify.local.json]
---

## 规则

用户明确纠正：构建和测试并发已经调好，不准因机器 CPU 数量或编译耗时擅自调整。沿用现有本地资源配置。

源码开发环境仍需完整预热测试；先完成启动目标编译，再串行预热测试，不把取消测试预热作为优化结果。

## 原因

资源配置已有用户调优依据；缺少预热会让后续 AI 开发时的测试构建成本难以控制。

## 适用场景

Rust 冷构建、缓存清理、预热脚本与 AI 开发验证成本优化。
