---
memory_type: feedback
feedback_category: repository
topic: vditor-capability-policy
summary: 内置 Vditor 提供完整原生能力，不因底座额外 CSP 或文章过滤牺牲交互。
keywords: [vditor, CSP, rich-text]
created_at: 2026-10-28 00
updated_at: 2026-10-28 00
decision_policy: direct_reference
scope: [rich-text]
---

## 规则
用户明确要求放开 Vditor 封装额外设置的 CSP 与文章内容限制，恢复原生能力和复制图标；本轮仅前端。

## 原因
用户认为文章内容由用户负责，底座应完整提供内置组件能力，反对因为限制而改成简化的文字按钮。

## 适用场景
Vditor 内置编辑器与阅读器的封装维护。不要据此推导其他模块的权限策略变化。
