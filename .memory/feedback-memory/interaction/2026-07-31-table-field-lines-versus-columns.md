---
memory_type: feedback
feedback_category: interaction
topic: 表格字段的行列语义必须按可见结构确认
summary: 用户说“名称一行、简介一行”但现状已经上下堆叠时，不应做无视觉差异的 CSS 加固；应先确认用户是否要求拆成独立表格列，并以可见变化验收。
keywords:
  - table
  - columns
  - rows
  - name
  - description
  - ui clarification
match_when:
  - 用户要求表格中的名称与简介分开
  - 用户使用“一行”“一列”描述表格结构
  - 当前界面已经看似满足用户描述但用户仍要求调整
created_at: 2026-07-31 19
updated_at: 2026-07-31 19
last_verified_at: 2026-07-31 19
decision_policy: direct_reference
scope:
  - web/app/src/features/settings
  - .memory/feedback-memory/interaction
---

# 表格字段的行列语义必须按可见结构确认

## 时间

`2026-07-31 19`

## 规则

- 用户要求调整表格字段，但当前视觉已经符合字面描述时，不做只有实现差异、没有可见差异的改动。
- “一行”可能指单元格内上下布局，也可能指独立字段；结合表格场景仍有歧义时，必须确认是否要拆成独立列。
- 验收必须证明表头和 `<td>` 已实际分离，不能只证明 CSS 或 DOM class 变化。

## 原因

- 用户关注的是可见的信息结构；只把纵向 `Flex` 换成显式 CSS 不会改变体验，属于无效修改。
- 表格独立列会影响表头、字段配置、列宽和横向滚动，与单元格内部两行是不同产品结构。

## 适用场景

- 后台管理表格的名称、简介、状态或元数据拆分
- 用户对已经“看起来一样”的 UI 修改提出质疑
- 表格字段配置和列宽设计
