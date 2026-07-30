---
memory_type: feedback
feedback_category: interaction
topic: 针对既有设计的局部问题不要重新评价已确认架构
summary: 当用户明确说明既有架构是其有意设计且没有问题时，后续只回答当前局部问题；除非局部目标与既有设计存在可证冲突，否则不要重新诊断或评价已确认架构。
keywords:
  - interaction
  - scope
  - approved design
  - architecture
  - targeted question
match_when:
  - 用户说明现有设计是有意为之且无需评价
  - 用户在既有架构上询问局部 contract、文档或实现选择
  - AI 准备把局部问题扩大为整体架构诊断
created_at: 2026-07-28 10
updated_at: 2026-07-28 10
last_verified_at: 2026-07-28 10
decision_policy: direct_reference
scope:
  - 对话流程
  - 需求对齐
  - 架构讨论
---

# 针对既有设计的局部问题不要重新评价已确认架构

## 时间

`2026-07-28 10`

## 规则

用户明确声明现有架构是其有意设计且没有问题后，将其视为当前讨论的已确认前提。后续只收敛用户正在询问的局部问题，不重复评价整体设计；只有发现局部目标与该前提存在直接、可验证冲突时才指出冲突。

## 原因

把局部问题重新扩张为整体架构审查会偏离用户真实问题，也会重复消耗已经完成的设计决策。

## 适用场景

- 既有架构上的 API contract 或文档增强
- 用户明确限定“不讨论现有设计”的后续问题
- 已确认方案中的局部实现选择
