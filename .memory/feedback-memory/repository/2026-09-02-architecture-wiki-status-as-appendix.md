---
memory_type: feedback
feedback_category: repository
topic: 架构 Wiki 只保留长期架构事实
summary: 架构 Wiki 以介绍、作用、场景和流程图解释长期稳定的机制；候选 SHA、Actions、验收状态与阶段进度留在对话和交付记录。
keywords:
  - architecture-wiki
  - document-status
  - issue-relations
  - appendix
match_when:
  - 编写或更新架构 Wiki
  - 需要记录候选 SHA、Actions 或验收状态
  - 需要解释多个相关 Issue 的职责
created_at: 2026-09-02 21
updated_at: 2026-10-18 09
last_verified_at: 2026-10-18 09
decision_policy: direct_reference
scope:
  - 1flowbase.wiki
  - architecture documentation
---

# 架构 Wiki 只保留长期架构事实

## 时间

`2026-09-05 00`

## 规则

架构 Wiki 和长期 ADR 只记录稳定的原理、结构、责任边界与不变量。候选 SHA、质量门禁、阶段标签、Issue 进度、临时代码现状、覆盖缺口和待办只在对话中阶段性汇报；不得为了可追溯性把这些易变状态追加到长期架构文档。

面向读者的架构介绍必须解释“是什么、有什么作用、用于哪些场景、怎样运行”，用流程图表达主要路径，再展开契约和边界。中英文都提供完整说明，不能把稀疏契约列表或实施纪要当作使用指南。

## 原因

阶段状态变化频繁，写入长期架构文档会混淆“目标不变量”和“某次候选事实”，并造成文档持续陈旧。阶段证据由对话、Issue、Actions 和 Assembly Receipt 承载。

2026-10-18，用户指出事件生命周期 Wiki 缺少介绍、作用、场景和流程图，过于简陋。仅列限制无法帮助新读者理解机制或作出选型判断。

## 适用场景

- 架构 Wiki、ADR 导读和长期维护文档。
- 核对最新代码与长期架构是否一致时。
- 汇报候选 SHA、Actions、Issue、覆盖缺口或阶段性验收结果时。
