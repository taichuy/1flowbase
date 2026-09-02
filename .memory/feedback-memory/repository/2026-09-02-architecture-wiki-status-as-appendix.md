---
memory_type: feedback
feedback_category: repository
topic: 架构 Wiki 状态与 Issue 关系放在文末附录
summary: 架构 Wiki 正文开头直接解释架构；候选 SHA、Actions、验收状态和 Issue 关系压缩到文末附录，不用长篇状态流水账占据正文。
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
updated_at: 2026-09-02 21
last_verified_at: 2026-09-02 21
decision_policy: direct_reference
scope:
  - 1flowbase.wiki
  - architecture documentation
---

# 架构 Wiki 状态信息放在附录

## 时间

`2026-09-02 21`

## 规则

架构文档正文开头直接进入核心问题、原理和结构。候选 SHA、质量门禁、阶段标签和 Issue 关系放到文末“实施状态与关联 Issue”附录，并只保留当前结论、关键证据链接和每个 Issue 的一句话职责。

不要在正文开头堆叠“文档状态”和完整 Actions 流水账，也不要重复列出大量测试数字。

## 原因

状态元数据会遮挡架构正文，且更新频繁。集中到附录既保留可追溯性，也让读者先理解架构本身。

## 适用场景

- 架构 Wiki、ADR 导读和长期维护文档。
- 多个 Issue 共同完成一个架构结果时的关系说明。
- 需要记录验收候选和 Actions 链接，但不需要完整 QA 报告正文时。
