---
memory_type: feedback
feedback_category: repository
topic: 合并门禁只由 PR 触发
summary: 普通提交或直接 push 不触发合并质量门禁；只有存在 PR 时由 pull_request 事件触发，定时与手动入口继续承担完整质量体检。
keywords:
  - quality-gate
  - pull-request
  - workflow-trigger
  - dev-push
created_at: 2026-07-31 12
updated_at: 2026-07-31 12
last_verified_at: 2026-07-31 12
decision_policy: direct_reference
scope:
  - .github/workflows/quality-gate.yml
  - .github/workflows/ai-gateway-concurrency.yml
---

# Quality Gate PR-only Trigger

## Rule

- 普通 commit 或直接 `push` 不触发合并质量门禁。
- 分支存在 PR 后，创建、重新打开以及后续提交导致的 `pull_request` / `synchronize` 事件触发门禁。
- `schedule` 与 `workflow_dispatch` 可以继续运行完整 nightly / project quality gate。
- workflow contract 不得再把 `push: branches: [dev]` 当成 required gate 验收条件。

## Reason

门禁属于合并决策，不应让没有 PR 的普通分支提交或直接 push 消耗完整 CI 资源；定时项目体检与 PR merge gate 是不同 lane。

## Applies When

- 合并或重构 AI Gateway 与全局质量门禁 workflow。
- 调整 GitHub Actions `on:` 触发器、required check 或 workflow source 测试。
