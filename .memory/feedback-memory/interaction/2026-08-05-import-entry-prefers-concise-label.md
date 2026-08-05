---
memory_type: feedback
feedback_category: interaction
topic: 高密度操作入口优先只保留简洁动作标签
summary: 高密度创建菜单中的操作入口优先只保留简洁动作词；不要自动添加格式或能力说明作为副文案。
keywords:
  - frontend
  - ui-copy
  - action-label
match_when:
  - 导入、导出、创建等高频动作
created_at: 2026-08-05 10
updated_at: 2026-08-05 10
last_verified_at: 2026-08-05 10
decision_policy: direct_reference
scope:
  - web/app/src/features
  - .memory/feedback-memory/interaction
---

# 高密度操作入口优先只保留简洁动作标签

## 时间

`2026-08-05 10`

## 规则

- 主入口表达动作即可，例如“导入”。
- 未被明确要求时，不为格式、对象范围或兼容性自动增加紧邻的辅助说明。

## 原因

- 创建菜单是高密度操作区；额外副文案会拉高视觉噪音，即使它解释了格式边界。

## 适用场景

- 应用、模板、文件或数据的导入入口。
- 其他高密度创建菜单中的高频操作入口。
