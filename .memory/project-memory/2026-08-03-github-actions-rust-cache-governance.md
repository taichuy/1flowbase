---
memory_type: project
topic: GitHub Actions Rust cache governance approved and implemented
summary: 用户确认采用平衡方向治理 GitHub Actions Rust 缓存。release 与 quality gate cache 改为依赖输入键控，不再因任意 .rs 变化创建完整 target cache；AI Gateway 大缓存只在默认分支写入。构建、双架构、Trivy 和发布语义保持不变。
keywords:
  - github-actions
  - rust-cache
  - container-images
  - quality-gate
  - ai-gateway
match_when:
  - 诊断或调整后端容器打包耗时
  - 修改 Rust Actions cache key、restore key 或缓存预算
  - 评估 quality gate 与 release cache 的共享配额
created_at: 2026-08-03 11
updated_at: 2026-08-03 11
last_verified_at: 2026-08-03 11
decision_policy: verify_before_decision
scope:
  - .github/workflows
  - scripts/node/github-quality-gate
---

# GitHub Actions Rust cache governance approved and implemented

## 谁在做什么

用户确认按平衡方向优化后端打包缓存。AI 已在本地 `beta` 工作树调整 container release seed、quality gate Rust cache 和 AI Gateway cache 保存策略，并补充 workflow 配置回归断言；当前未 commit、未 push。

## 为什么这样做

2026-08-03 run `30780058083` 的 api-server amd64 / arm64 cache 均未命中，Cargo release 分别耗时 8 分 29 秒和 6 分 39 秒。仓库 Actions cache 当时约 10.43 GiB，前一日保存的两份 release cache 已被淘汰；cache key 又包含全部 `.rs` 哈希，使普通源码变化持续创建约 600 MiB × 2 的不可变缓存。

## 决策与边界

- release seed 使用显式 `v2` schema，并按架构、toolchain/profile、`Cargo.lock` 与 `Cargo.toml` 键控，不再包含 image tag 或 `.rs` 哈希。
- quality gate 恢复同一 release seed；自身 Rust cache 同样改为依赖输入键控并使用 `v2` schema。
- AI Gateway 的大 target cache 只在仓库默认分支保存，其他分支仍允许恢复。
- `cargo build --release`、amd64/arm64、Trivy、安全门禁与发布步骤不得改变。
- 本地 Dev Acceptance 证据：4 条定向 workflow 测试通过、3 个 YAML 文件解析通过、`git diff --check` 通过；完整测试中另有一个本任务开始前已存在的 PostgreSQL volume 断言失败。

## 截止日期与停止条件

首次部署后需观察冷构建、同提交重跑、普通源码增量构建三类 Actions 数据；确认 cache hit、依赖复用且 release seed 不再于一天内消失后，本阶段才能以运行态证据完全结算。若依赖输入、toolchain、profile 或 feature 策略改变，应递增 cache schema 或补充 key 输入。
