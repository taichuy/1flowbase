---
title: "当前前端开发调试先调整效果再运行门禁"
memory_type: feedback
feedback_category: interaction
created_at: "2026-11-11 23"
updated_at: "2026-11-13 07"
decision_policy: direct_reference
status: active
tags: [frontend, development, browser, verification]
---

- 规则：当前用户主导的前端开发调试阶段，在用户调整并确定效果之前，不运行测试门禁；可以用 Playwright 浏览器操作复现和查看效果。
- 原因：用户需要先迭代可见效果，避免每次调整提前进入测试验收。
- 适用场景：2026-11-11 开始的页面开发调试及其连续调整；用户于 2026-11-13 再次明确先调整效果、后运行测试门禁。用户确认效果或明确变更阶段后，按新的授权选择验证。不推定全项目永久停用测试。
