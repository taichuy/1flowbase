---
memory_type: project
topic: GitHub Actions Rust cache governance approved and implemented
summary: 2026-10-05 用户授权在 beta worktree 验证发布缓存优化；固定工具链、分离下载与 target 缓存、按内容校验恢复输入时间戳。beta 跨提交热构建流水线 2分48秒，修复已合入并推送 dev。旧 v2 阶段策略见历史正文，以当前源码和最新验证为准。
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
updated_at: 2026-10-05 17
last_verified_at: 2026-10-05 17
decision_policy: verify_before_decision
scope:
  - .github/workflows
  - scripts/node/github-quality-gate
---

# GitHub Actions Rust cache governance approved and implemented

## Current Decision — 2026-10-05 17

用户批准平衡方向，要求用 dev 小升级状态在 beta worktree 触发发布并得到实际提速后报告。目标是复用未变化输入且不掩盖代码编辑，避免仅凭 cache hit 宣称成功；本阶段无额外截止日期，停止条件为真实热构建和输入变更正确性验证完成。

AI 已在 beta 候选核验修复，并快进集成到 dev；本地与远端 dev/beta 均为 `1fe02bd97`。dev 并行 MCP 任务的 dirty 文件保留，未提交。未部署生产、未晋升 latest。

- 用户所指 run 37284320742 是新缓存格式冷构建；后续旧方案热构建 37286022399 仍需 8分02秒/8分28秒，因为 checkout 时间戳导致本地 crate 重编。
- 修复后冷构建 37287195341 成功，跨提交（仅文档变化）热构建 37289323809 成功；后端步骤 18秒/20秒，Cargo 0.28秒/0.23秒，整个流水线 2分48秒。
- 32 项定向测试通过，真实 Cargo fixture 覆盖编辑、回退、目录输入增删；双架构两种二进制 SHA256 与冷构建一致。
- beta 缓存不能自动供 sibling latest 分支使用，正式分支首次仍可能冷构建；实际代码/依赖变化要编译受影响部分，不能承诺所有发布均达到本次热缓存耗时。
- 证据入口：https://github.com/taichuy/1flowbase/actions/runs/37289323809

## Historical Stage — 2026-08-03

以下为旧阶段记录，不作为当前配置真值。

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
