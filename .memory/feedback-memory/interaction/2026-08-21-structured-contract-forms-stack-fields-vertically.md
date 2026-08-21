---
memory_type: feedback
feedback_category: interaction
topic: 结构化契约表单的重复字段保持单列纵向排列
summary: 用户要求后台组件契约的 Props、限制、示例和上游来源字段直向逐行排列，不将相关输入横向压缩在同一行。
keywords:
  - settings
  - structured form
  - component contract
  - vertical layout
match_when:
  - 后台编辑结构化契约、schema 或配置对象
  - 表单包含 props、examples、limitations 或上游元数据等重复字段
created_at: 2026-08-21 22
updated_at: 2026-08-21 22
last_verified_at: 2026-08-21 22
decision_policy: direct_reference
scope:
  - web/app/src/features/settings
  - 后台结构化编辑抽屉
---

# 结构化契约表单使用单列字段流

## 规则

重复对象的字段与操作从上到下依次排列；每个输入占完整可用宽度。示例标题与代码、上游包名/组件名/版本不得横向拼成一行。

## 原因

横向排列会让字段拥挤、扫描顺序不清晰，尤其在窄屏抽屉中造成可读性与编辑效率下降。

## 适用场景

设置后台的组件契约、配置、schema 和类似列表型结构化编辑表单。
