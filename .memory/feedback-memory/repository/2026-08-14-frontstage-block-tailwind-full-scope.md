---
memory_type: feedback
feedback_category: repository
topic: 前台区块 Tailwind 应是区块级完整能力而非静态命中裁剪
summary: 用户否定仅从区块源码静态候选按需生成 Tailwind CSS；导入 Tailwind 后应按区块边界提供完整、接近普通代码环境的 Tailwind 使用语义。
keywords:
  - frontstage
  - JS Block
  - Tailwind CSS
  - source-driven-utilities
  - ShadowRoot
match_when:
  - 设计或修改前台 JS Block 的 Tailwind 导入、编译、运行时样式与能力边界时
created_at: 2026-08-14 11
updated_at: 2026-08-14 11
last_verified_at: 2026-08-14 11
decision_policy: direct_reference
scope:
  - web/packages/tailwindcss-catalog
  - web/packages/page-runtime
  - web/app/src/features/frontstage
---

# 前台区块 Tailwind 使用边界

## 时间

`2026-08-14 11`

## 规则

不要把“只扫描区块源码中的静态字符串并生成命中 utility”作为最终 Tailwind 产品 contract。用户期望 `import 'tailwindcss'` 后在该区块 ShadowRoot 内获得完整、接近普通代码环境的 Tailwind 使用能力，而不是由当前源码候选决定可用样式范围。

## 原因

静态命中裁剪会让动态 class、条件组合和后续运行态 class 缺失，导入语义与用户理解的“正常代码块导入 Tailwind”不一致。

## 适用场景

前台 JS Block 的 Tailwind catalog、compiler、generated CSS、ShadowRoot 样式注入、依赖锁和相关运行态测试。

## 备注

Tailwind 任意值和无限变体组合无法被有限静态 CSS 数学意义上全部预生成；具体“完整”contract 仍需在问题对齐中明确为版本化预设或运行时编译能力。
