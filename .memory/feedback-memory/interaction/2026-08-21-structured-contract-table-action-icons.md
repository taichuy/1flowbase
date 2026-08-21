---
memory_type: feedback
feedback_category: interaction
topic: 结构化契约子表格使用图标操作
summary: 用户要求组件契约抽屉的 Props、备注和示例子表格将“编辑 / 移除”文字操作替换为图标按钮。
keywords:
  - settings
  - component contract
  - structured table
  - icon action
match_when:
  - 后台结构化表单包含行内编辑与删除操作
  - 组件契约 Props、备注或示例表格需要紧凑呈现
created_at: 2026-08-21 23
updated_at: 2026-08-21 23
last_verified_at: 2026-08-21 23
decision_policy: direct_reference
scope:
  - web/app/src/features/settings/components/ui-management
  - 后台结构化编辑抽屉
---

# 子表格行操作使用图标

## 规则

在组件契约的 Props、备注、示例等子表格中，编辑和移除使用项目已有的编辑、删除图标按钮，而不显示重复的文字链接；图标按钮必须保留本地化的 `aria-label`。

## 原因

子表格的行操作需要紧凑，重复文字会干扰对字段内容的扫描；可访问名称保证键盘和读屏用户仍能理解操作。

## 适用场景

设置后台的组件契约、配置、schema 和类似列表型结构化编辑表单。
