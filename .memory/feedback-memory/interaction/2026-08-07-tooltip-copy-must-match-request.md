---
memory_type: feedback
feedback_category: interaction
topic: tooltip-copy-request
summary: 用户明确指定 Tooltip 文案时，必须逐字采用指定术语，不以已有 aria-label 或相近 i18n 文案替代。
keywords:
  - Tooltip
  - 文案
  - i18n
match_when:
  - 用户为图标、按钮或入口指定了可见 Tooltip 文案
created_at: 2026-08-07 00
updated_at: 2026-08-07 00
last_verified_at: 2026-08-07 00
decision_policy: direct_reference
scope:
  - frontend visible copy
---

# Tooltip 文案须匹配用户指定术语

## 时间

`2026-08-07 00`

## 规则

用户明确列出 Tooltip 文案时，新增独立的 i18n key 并使用该文案；不要复用含义相近但粒度不同的 aria-label、按钮文案或既有翻译。

## 原因

Tooltip 是用户可见的产品文案，其术语和信息密度需要与用户指定的界面语义一致。

## 适用场景

图标按钮、操作菜单、帮助入口和其它需要新增或调整可见 Tooltip 的前端改动。
