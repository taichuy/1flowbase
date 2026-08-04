---
memory_type: project
topic: 项目健康架构审计整改已合并 beta
summary: 用户批准六 Delivery 平衡 Issue Tree，在不改变功能、UI、URL、API 语义和翻译文案的前提下完成质量门禁、Frontstage owner、OpenAPI fragments、共享 SHA-256、确定性死代码与 ownerless i18n 清理；冻结候选 0e886f35f 通过集中 QA，并于 2026-08-04 合并为 beta 的 5403ec442，等待用户人工测试。HostExtension native activation 始终排除。
keywords:
  - project-health
  - architecture-refactor
  - frontstage
  - openapi
  - sha256
  - dead-code
  - i18n
  - centralized-qa
match_when:
  - 继续项目健康审计整改
  - 检查 codex/project-health-refactor 或 beta 合并状态
  - 调研 HostExtension native activation 风险
created_at: 2026-08-04 12
updated_at: 2026-08-04 12
last_verified_at: 2026-08-04 12
decision_policy: verify_before_decision
scope:
  - beta
  - codex/project-health-refactor
  - web/app/src/features/frontstage
  - api/apps/api-server/src/openapi
  - web/packages/page-runtime/src/sha256.ts
  - scripts/node/testing/verify-runtime.js
---

# 项目健康架构审计整改已合并 beta

## 时间

`2026-08-04 12`

## 谁在做什么

- 用户批准“六个 Delivery 的平衡 Issue Tree”，要求全部开发在隔离 worktree 完成、集中 QA 通过后合回当前 `beta` 供人工测试。
- AI 完成 D1 质量门禁、D2 Frontstage owner、D3 OpenAPI fragments、D4 单一 SHA-256、D5 确定性死代码、D6 ownerless i18n 清理。
- 冻结候选 `0e886f35f3c46a7930b69264d3c67c094e4b8cf4` 已通过集中 QA，并通过 merge commit `5403ec442eeade997200f30abecb1ac510ef906a` 合回 `beta`。

## 为什么这样做

- 审计发现路由业务职责、OpenAPI 手工注册、重复 SHA 核心、孤儿组件和治理脚本漂移是主要可收敛复杂度。
- 选择平衡方向是为了降低协调热点和维护漂移，同时保持现有产品行为与 UI 可见性不变。

## 为什么要做

- 项目代码规模增长后，需要区分必要复杂度与无 owner 的历史残留，避免重复实现、死代码和跨层职责耦合继续膨胀。

## 截止日期

- 无

## 验收证据

- frontend fast：172/172 files、940/940 tests。
- Rust static、style-boundary 18/18、contracts 42/42、SHA-256 5/5、Frontstage routing 5/5、OpenAPI targeted 1/1 均通过。
- i18n hygiene 为 0 error，仍保留 132 个没有充分 owner 删除证据的 warning。
- R2 后只修改 QA fixture 与本地 Vitest worker heuristic；最终本地 16 available parallelism 使用 8 workers，完整 fast gate 稳定通过。
- `beta` 合并后的文件树与冻结候选完全一致；本轮未 push。
- 三个本任务临时 worktree 已回收，分支和提交保留。

## 边界与下一步

- HostExtension native activation 从未进入本 Root；相关 High 风险仍待用户独立调研与重新对齐。
- 用户下一步在 `beta` 做人工功能与 UI 测试。
- 既有 i18n warning、重复 React key、Rust dead-code/deprecation warning 不在本轮扩大处理。
