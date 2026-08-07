---
memory_type: feedback
feedback_category: repository
topic: Agent Flow 注册节点编辑器应沿用既有 Inspector 视觉语法
summary: 新增注册节点的复杂字段编辑器应优先参考 If / Else 等成熟节点的视觉语法；注册制负责 field/renderer 装配，renderer 仍须保持一致交互，公开输出变量名应允许用户配置并维护引用完整性。
keywords:
  - agent-flow
  - node-registration
  - inspector
  - visual-consistency
  - variable-aggregator
  - if-else
match_when:
  - 新增或调整 Agent Flow 注册节点的 Inspector 编辑器
  - 复杂节点字段需要分组、候选行或增删操作
  - 节点编辑器视觉上与 If / Else 等成熟节点不协调
created_at: 2026-08-07 15
updated_at: 2026-08-07 15
last_verified_at: 2026-08-07 15
decision_policy: direct_reference
scope:
  - web/app/src/features/agent-flow/components/bindings
  - web/app/src/features/agent-flow/components/editor/styles/inspector.css
---

# Agent Flow 注册节点编辑器视觉一致性

## 时间

`2026-08-07 15`

## 规则

注册机制只决定节点能力如何进入系统，不应让每个节点另起一套 Inspector 视觉语言。复杂字段优先复用 If / Else 已验证的语义 section、浅边框、统一圆角、10px 级间距、紧凑 header 与响应式控件排列；只复用视觉语法，不为一次复用提前抽象通用组件。

注册节点的复杂字段可以绑定专用 renderer，但专用 renderer 不能绕开注册链或自行形成孤立交互。Variable Aggregator 的组名同时是公开输出变量 key，应允许用户编辑；重命名时必须同步物化 output，并对当前文档内下游 selector 维持引用完整性，不能只改标题或留下静默失效引用。

## 原因

Variable Aggregator 直接使用 Ant Design `Card`，而 detail panel 的上层通用 `.ant-card` 规则会把边框、背景、圆角和 padding 扁平化，导致变量组层级、组内候选和组外新增动作失去清晰边界，视觉上明显区别于 If / Else。

## 适用场景

Agent Flow / Workflow 节点注册后的 Inspector 表单、变量组、条件组、分支组以及其他可增删嵌套编辑器。
