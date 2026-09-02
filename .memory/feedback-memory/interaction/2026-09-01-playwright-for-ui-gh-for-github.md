---
memory_type: feedback
feedback_category: interaction
topic: playwright-for-ui-gh-for-github
summary: Playwright 用于前端页面验证和开发交互；GitHub Issue、PR 等仓库托管操作继续使用 gh，不应因用户强调 Playwright 验证而改用浏览器维护 GitHub。
keywords:
  - Playwright
  - GitHub
  - gh
  - issue
  - browser verification
match_when:
  - 用户要求使用 Playwright 验证前端页面
  - 同一任务需要创建或维护 GitHub Issue、PR、labels
  - 需要选择 Playwright 浏览器操作或 gh CLI
created_at: 2026-09-01 17
updated_at: 2026-09-01 17
last_verified_at: 2026-09-01 17
decision_policy: direct_reference
scope:
  - frontend verification
  - GitHub operations
  - user interaction
---

# Playwright 用于 UI 验证，GitHub 操作使用 gh

## 时间

`2026-09-01 17`

## 规则

- Playwright 默认用于前端页面打开、交互复现、截图和浏览器验收。
- GitHub Issue、PR、labels、评论等仓库托管操作继续使用 `gh`。
- 用户在开发任务中强调使用 Playwright 时，不据此推断 GitHub 管理也必须经浏览器完成；只有用户明确要求网页端操作 GitHub 时才改变工具选择。

## 原因

Playwright 是前端运行态验证工具，`gh` 是 GitHub 仓库托管操作的直接入口。混用会增加登录、浏览器状态和交互脆弱性，且偏离用户对工具职责的预期。

## 适用场景

- 前端实现同时需要浏览器验收和 GitHub Issue / PR 管理。
- 用户说“使用 Playwright”但语境指向页面验证、开发操作或 UI 取证。
- 创建、更新、查询或关闭 GitHub Issue / PR。

## 备注

本规则不覆盖用户明确要求通过网页端演示或复现 GitHub UI 的场景。
