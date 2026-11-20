---
title: "Actions 精简要核对常用分支真实触发的工作流"
memory_type: feedback
feedback_category: repository
created_at: "2026-11-18 18"
updated_at: "2026-11-18 18"
decision_policy: direct_reference
status: active
tags: [github-actions, triggers, cleanup]
---
- 规则：讨论测试 Actions 是否必要，要分清正式打包质量门禁、独立业务专项与历史展示；不要把仍配置了日常分支触发当成必须保留的理由。
- 原因：仅减少启用的入口总数，没有解决用户在 beta 合入 dev 时一次出现多条独立专项 run 的问题。
- 场景：调整或验收 Actions 触发策略时，核对具体推送的 SHA、分支、paths 与真实 run；单独确认需保留的 PR、夜间、发布和回滚行为。本反馈不授权关闭或删除任意门禁或历史记录。
