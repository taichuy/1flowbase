---
memory_type: feedback
feedback_category: repository
topic: JSX Studio 模板入口与替换语义
summary: JSX Studio 的模板能力应位于右侧资源栏并与组件能力分区；应用模板替换整个当前代码草稿，组件能力才是在光标位置局部插入。
keywords:
  - jsx-studio
  - template
  - component
  - replace
  - resource-panel
match_when:
  - 调整 JSX Studio 的模板、组件或代码编辑资源入口
  - 设计模板应用与组件插入的交互、文案或测试
created_at: 2026-08-11 15
updated_at: 2026-08-11 15
last_verified_at: 2026-08-11 15
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage/components/jsx-studio
  - web/app/src/shared/code-block
---

# JSX Studio 模板入口与替换语义

## 规则

- 模板是编辑器资源，应通过 JSX Studio 右侧资源栏中的独立“模板”入口呈现，不占用代码编辑区顶部空间，也不与“组件”入口合并。
- 应用模板的语义是用模板源码替换整个当前代码草稿；它不是光标处插入，也不复用组件插入动作和文案。
- 组件保持局部组合语义：在当前编辑位置插入组件所需的 import 与 JSX 片段。

## 原因

模板决定完整文档基线，组件只扩充当前文档。两者影响范围和失败风险不同，若入口或动作语义混合，会让用户误判模板只是追加片段，也会使编辑器顶部被低频资源选择器长期占用。

## 适用场景

- JSX Studio 右侧资源栏的信息架构与标签顺序。
- 模板选择、替换确认、草稿 dirty 状态与撤销行为。
- 组件 Catalog 的插入动作、文案和自动化测试。
