---
memory_type: feedback
feedback_category: interaction
topic: 模板预览不能人为阉割已登记的运行能力
summary: 用户质疑默认官方模板在预览中被拒绝，以及为何预览区块还要严格受限；后续设计必须先区分已登记模块的完整运行契约与没有真实页面语义的外部副作用，而不能把前者错误归入“受限预览”。
keywords:
  - ui-code-template
  - preview
  - catalog
  - dependency-lock
  - frontstage
match_when:
  - 调整或诊断代码模板、区块草稿的预览运行时
  - 决定已注册模块、API、事件、导航、outputs 在预览中的能力
created_at: 2026-08-20 22
updated_at: 2026-08-20 22
last_verified_at: 2026-08-20 22
decision_policy: direct_reference
scope:
  - web/app/src/features/settings/components/ui-management/UiCodeTemplateStudio.tsx
  - web/app/src/features/frontstage/lib/block-catalog.ts
---

# 模板预览先保证注册依赖完整

## 规则

- 官方或用户模板引用其贡献 Catalog 已登记的模块时，模板预览必须复用该 Catalog 的 import policy、依赖锁和资产，不得以“预览受限”为由拒绝。
- 讨论 API、事件、导航和 outputs 时，要明确它们缺少的真实 page / tab / block consumer 语义或实际副作用；不能与模块加载完整性混为一谈。

## 原因

模块白名单、导出 contract 与 digest-locked assets 是运行时正确性机制。它们对已登记的官方模板应自动成立；否则默认模板与真实区块实例行为分裂，人工测试无法成立。
