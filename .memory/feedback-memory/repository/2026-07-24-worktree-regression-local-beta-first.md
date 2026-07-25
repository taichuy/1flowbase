---
memory_type: feedback
feedback_category: repository
topic: worktree 分支回归先合并本地 beta 再推送
summary: worktree 开发完成后，交付顺序以本地 beta 为装配真值，先本地合并和验证，再推送远端 beta。
keywords:
  - worktree
  - beta
  - local-merge
  - regression
  - git-workflow
match_when:
  - 从隔离 worktree 交付代码到 beta
  - 用户要求代码回归、合并或推送 beta
created_at: 2026-07-24 15
updated_at: 2026-07-24 15
last_verified_at: 2026-07-24 15
decision_policy: direct_reference
scope:
  - repository-workflow
---

# Worktree 回归与 beta 合并顺序

## 规则

worktree 分支完成实现和定向回归后，默认按以下顺序交付：提交 worktree 分支，合并到本地 `beta`，在本地 `beta` 完成必要验证，然后直接推送远端 `beta`。

除非用户另行要求，不要先创建并合并远端 PR，再把远端 `beta` 反向同步到本地。

## 原因

用户以本地 `beta` 作为最终装配和重启验证入口，需要本地分支先形成明确的合并结果，再同步远端。

## 适用场景

隔离 worktree 功能开发、缺陷修复、代码回归以及明确要求合并到 `beta` 的交付任务。
