---
memory_type: project
topic: 超限文件职责拆分已按 introduced-only 口径合并 dev
summary: 用户批准在隔离 worktree 拆分明确混合职责的超 1500 行文件，并在确认完整批次代表失败与 dev 基线同败后接受 existing-codebase introduced-only 口径。assembly 74ef0c974 已于 2026-08-03 合并为 dev 的 7b430465b，等待用户人工测试；既有测试债交由定期项目体检继续治理。
keywords:
  - file-size-pressure
  - file-boundary
  - worktree
  - batch-qa
  - refactor
  - assembly
match_when:
  - 继续超限文件职责拆分任务
  - 检查 codex/file-boundary-assembly 是否可合回 dev
  - 处理第二轮 QA 的 Rust visibility/helper owner 或 worktree fixture 问题
created_at: 2026-08-03 11
updated_at: 2026-08-03 14
last_verified_at: 2026-08-03 14
decision_policy: verify_before_decision
scope:
  - /home/taichuy/git/1flowbase_git_workspace/file-boundary-assembly
  - codex/file-boundary-assembly
  - api/crates/control-plane
  - api/crates/storage-durable/postgres
  - web/app/src/features/settings
  - scripts/node/repo-hygiene
---

# 超限文件职责拆分已按 introduced-only 口径合并 dev

## 时间

`2026-08-03 11`

## 谁在做什么

- 用户批准“平衡方案”：拆分明确混合职责的生产文件与测试/fixture，精确排除生成 bundle 行数误报，暂缓 8 个核心 runtime/contract 文件。
- AI 在两个开发 worktree 完成 Work Packet，并由 Root 串行装配到 `codex/file-boundary-assembly`；主工作树 `dev` 全程保持不变。
- assembly `74ef0c9747fdc1015d6d234068af62d8b6d3cc6a` 已通过 merge commit `7b430465ba4252d990361a3cd2594397028fa28c` 合回 `dev`。
- 用户下一步在 `dev` 做人工测试；本轮未推送远端。

## 为什么这样做

- 用户要求所有开发在 worktree 完成，集中测试无问题后才合回当前分支供人工测试。
- 长计划使用 Batch Acceptance：开发 Packet 不逐包回归，全部装配后只运行 fresh QA 集中 Test Batch。

## 为什么要做

- 把超过 1500 行且混合职责的文件按稳定领域 owner 拆分，保持 API、DTO、权限、SQL、状态和 UI 行为不变。
- 让 `repo-hygiene` 不再把官方 browser bundle 与 `api/plugins/installed` 安装副本当成手写代码压力，同时继续检查普通插件源码。

## 截止日期

- 无

## 当前证据与停止原因

- 108 个变更文件均低于 1500 行；repo-hygiene 单测 12/12、consumer contract 42/42、typecheck 通过。
- 第一、二轮 QA 暴露的 Rust derive/visibility/path、MCP mock path、storage helper owner、route DTO imports 已修复；完整 assembly 三 crate `cargo check` 通过。
- ignored plugin fixture 已通过 symlink 对齐主开发区；Settings 失败单独复跑通过。
- 修订后的 fresh QA：AC-001～AC-004 green；109 个变更文件均低于 1500，repo-hygiene 12/12、consumer contract 42/42、typecheck 通过，未证明 assembly-only 产品回归。
- 完整 batch 仍有既有失败：control-plane 849/40、api-server 911/24、storage-postgres 290/2/1 ignored、前端 aggregate 93/1。代表失败在 `dev@d43ac993` 同样复现，包括 plugin node context、scope grant、unavailable installation、dynamic inventory、MCP debug、provider schema 与 legacy backfill。
- 用户确认仓库有定期体检，因此接受 existing-codebase introduced-only 口径：代表失败与基线同败作为 warning，不扩大本次职责拆分范围。
- 已合并、未推送；临时 worktree 暂时保留到人工测试结论明确。

## 下一步决策

- 用户人工测试通过后可按现有分支策略决定 push，并统一回收本次临时 worktree。
- 定期体检继续跟踪 plugin node context、scope grant、动态 inventory、MCP debug 权限、provider schema 与 legacy migration fixture 等既有测试债。
