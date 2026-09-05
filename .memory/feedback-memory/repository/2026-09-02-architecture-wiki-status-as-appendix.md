---
memory_type: feedback
feedback_category: repository
topic: 架构 Wiki 只保留长期架构事实
summary: 架构 Wiki 只记录长期稳定的架构事实；候选 SHA、Actions、验收状态、代码覆盖缺口和阶段进度只在对话中汇报，不写入长期架构文档。
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
updated_at: 2026-09-05 00
last_verified_at: 2026-09-05 00
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

## 原因

阶段状态变化频繁，写入长期架构文档会混淆“目标不变量”和“某次候选事实”，并造成文档持续陈旧。阶段证据由对话、Issue、Actions 和 Assembly Receipt 承载。

## 适用场景

- 架构 Wiki、ADR 导读和长期维护文档。
- 核对最新代码与长期架构是否一致时。
- 汇报候选 SHA、Actions、Issue、覆盖缺口或阶段性验收结果时。
