---
memory_type: feedback
feedback_category: repository
topic: 管理台长表单必须复用统一弹窗壳
summary: 管理台中包含动态字段或可滚动内容的新增/编辑表单必须使用 FixedHeightModal，不得直接使用 Ant Design Modal。
keywords:
  - FixedHeightModal
  - modal shell
  - settings form
  - dynamic fields
created_at: 2026-08-22 10
updated_at: 2026-08-22 10
last_verified_at: 2026-08-22 10
decision_policy: direct_reference
scope:
  - web/app/src/shared/ui/fixed-height-modal/FixedHeightModal.tsx
  - web/app/src/features/settings
---

# 管理台长表单使用统一弹窗壳

## 规则

动态 schema、多个输入项或内容可能超过视窗的管理台表单，一律使用 `FixedHeightModal`；它提供统一的居中尺寸、固定容器和内部滚动区域。

## 原因

直接使用 `Modal` 会让长表单的滚动、页脚和视觉壳层偏离项目通用交互，截图中的“添加代理”即是该回归。

## 适用场景

设置页、网络中心、模型供应商、MCP 等新增或编辑多字段表单。
