---
memory_type: project
topic: 发版前业务代码瘦身与三层质量门禁治理已全绿合入 dev
summary: 基于 v0.6.6 与 dev 基线完成 B1–B5 业务瘦身、F1 PostgreSQL 快照迁移无锁修复及 G0–G4 三层质量门禁提速，最终提交 2b7eb937261156425a881a526653d804df1f452e 通过独立 QA 与线上全部三条 GitHub Actions 流水线并快进合入 dev。
keywords:
  - quality-gate
  - verify
  - slimming
  - resource-budget
  - release
match_when:
  - 准备从 dev 发版或评估当前三层质量门禁、本地 60% CPU / 8 GiB 内存预算与 API 覆盖率分片机制
  - 回溯 v0.6.6 后发版前质量治理与业务代码精简范围
created_at: 2026-11-26 07
updated_at: 2026-11-26 07
last_verified_at: 2026-11-26 07
decision_policy: verify_before_decision
scope:
  - api
  - web
  - scripts
  - .github/workflows
  - docs/architecture/quality-gates/README.md
---

# 发版前业务代码瘦身与三层质量门禁治理已全绿合入 dev

## 时间

`2026-11-26 07`

## 谁在做什么

通过本机 `codex-commander` 调度远程 `taichuy-dev` 上的 Codex 与独立 QA 子代理，围绕 `docs/draft/zhiliangtishici.md` 完成发版前业务代码精简（`B1–B5`）、PostgreSQL 可重复读快照迁移死锁修复（`F1`）以及三层质量门禁治理与 CI 加速（`G0–G4`），并将通过全部线上门禁的候选提交 `2b7eb937261156425a881a526653d804df1f452e` 快进同步至远程与本机 `dev`。

## 为什么这样做

遵循“功能完整性 > 稳定性 > 性能 > 可演进性”与零新增第三方依赖原则：
- 业务侧先做形式化规范建模与复杂度验证，消除插件托管凭据/临时缓存租约重复样板、精简 `applications/interface.rs`、将前端原生块源码缓存改造为多连接原子持久计数的 IndexedDB v3 索引结构，不增加任何服务端业务容量硬上限或 PostgreSQL 存储负担。
- 门禁侧按 Level 1（即时反馈）、Level 2（PR 精确影响路由与契约）、Level 3（单次插桩构建复用的全量回归）分层，并将本地资源预算统一收敛到 `max(1, floor(0.60 * CPU))` 与聚合 `min(有效容量, 8 GiB)` 内存硬上限。

## 为什么要做

在 `v0.6.6` 之后快速迭代的 77 个提交中积累了格式化/Clippy/前端单测与 React Doctor 门禁欠账、本地门禁内存冲高风险，以及线上 `quality-gate.yml` 中 `storage-postgres` 55 分钟死锁超时和 `api-server` 覆盖率五次重复插桩编译的高耗时问题，需要在正式发版前一次性跑通并稳固底座门禁。

## 截止日期

本次发版前底座质量门禁闭环已于 `2026-11-26 07` 完成；后续正式打 tag 发版前以最新候选 SHA 的线上门禁收据为准。

## 决策背后动机

保证日常开发即时反馈在数十秒与百兆级内存内完成、工作机多 worktree 验证不再突破 8 GiB 触发宕机，同时让 PR 门禁与发布级全量回归保持 100% 缺陷检测保留率并显著降低 GitHub Actions Runner 分钟消耗。

## 关联文档

- `docs/architecture/quality-gates/README.md`
- `docs/architecture/testing/README.md`
- `scripts/README.md`
- `api/AGENTS.md`
