---
memory_type: feedback
feedback_category: repository
topic: 代码区块以原生 React/CSS 为作者契约并在区块边界隔离样式
summary: 代码区块不应通过私有 style token 白名单限制 CSS；作者应使用通用模型可直接生成的原生 React、DOM 和 CSS，样式隔离由区块运行时边界承担。
keywords:
  - code block
  - native React
  - raw CSS
  - style isolation
  - UI design mode
  - block runtime
match_when:
  - 设计或修改代码区块 renderer、runtime、JSX/TSX contract
  - 调整 antd-facade、BlockUiStyle、CSS 白名单或样式隔离
  - 评估 UI 设计模式下代码区块权限和作者体验
created_at: 2026-07-26 22
updated_at: 2026-07-26 22
last_verified_at: 2026-07-26 22
decision_policy: direct_reference
scope:
  - web/packages/page-runtime
  - web/packages/page-protocol
  - web/packages/block-renderer
  - web/packages/antd-facade
  - web/app/src/features/frontstage
---

# 代码区块的 React/CSS 边界

## 规则

代码区块的作者契约使用常规 React、DOM 与 CSS 语义，包括原生 `style`、`className` 和完整 CSS 表达能力；不得要求作者或通用聊天模型掌握 1flowbase 私有样式分类与 token 映射。区块的职责是让样式影响留在本区块内，隔离复杂度由 runtime/DOM owner 承担，不由 CSS 属性白名单承担。

UI 设计模式权限是作者进入与发布能力的授权门槛；CSS 不再逐属性做权限式限制。后端 API、数据与动作授权仍由各自 owner 执行，不能与样式表达能力混为一谈。

## 原因

私有 facade style schema 会降低代码区块表达力，并让不了解仓库源码的模型生成可编译但运行时被静默丢弃的样式。长期架构应让标准 React/CSS 知识直接有效，同时以真正的区块隔离边界阻止样式污染宿主和相邻区块。

## 适用场景

代码区块、认证器公开 UI Block、Frontstage Block Studio、渲染运行时、组件导入契约和样式隔离设计。该方向属于新架构，不为旧 `BlockUiSchema`/facade 内容保留兼容分支；具体替换方案仍需完成需求对齐并由用户确认。
