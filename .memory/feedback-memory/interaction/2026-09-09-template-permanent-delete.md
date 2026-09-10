---
title: 代码模板使用永久删除
memory_type: feedback
feedback_category: interaction
created_at: 2026-09-09 17
updated_at: 2026-09-09 17
decision_policy: direct_reference
---
- 规则：代码模板管理使用真实永久删除，前后端语义一致，不以归档、软删除或恢复入口代替。
- 原因：用户明确要求“删了就是删了，不需要归档”。
- 适用场景：代码模板管理；不要扩展为全项目任意资源都必须硬删除。
- 历史数据：用户明确要求已有归档模板恢复普通列表并保留内容，之后按需手动删除。
