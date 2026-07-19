---
memory_type: project
topic: 代码区块以 TSX 为默认作者语言并兼容 JSX
summary: 用户确认低代码区块产品名统一为“代码区块”，编辑器名为“TSX 编辑器”；新模板默认 TSX，运行时继续接受 jsx | tsx，旧 JSX 无需迁移，并补齐纯 TypeScript 源码转译。
keywords:
  - frontstage
  - code block
  - tsx
  - jsx compatibility
  - low code
match_when:
  - 调整代码区块编辑器、默认模板或产品命名
  - 修改 code_template_language 或代码区块编译链
  - 评估旧 JSX 是否需要迁移
created_at: 2026-07-19 10
updated_at: 2026-07-19 10
last_verified_at: 2026-07-19 10
decision_policy: verify_before_decision
status: active
scope:
  - web/packages/page-runtime/src/js-block-tsx-compile.ts
  - web/app/src/features/frontstage/components/jsx-studio
  - web/app/src/features/frontstage/lib/block-templates.ts
  - api/plugins/capability-plugins/1flowbase/manifest.yaml
---

# 代码区块 TSX 默认语言

- 谁在做什么：Frontstage 代码区块把现有 TSX 编辑与编译能力正式作为默认作者体验，运行时仍保留旧 JSX 输入兼容。
- 为什么这样做：前端工程主体使用 TSX，代码区块需要支持类型声明、类型标注等纯 TypeScript 语法，而不能只在检测到 JSX 标签时才编译。
- 为什么要做：统一用户心智和产品命名，同时不要求已有 JSX 区块迁移或改变后端 `jsx | tsx` 协议。
- 截止日期：2026-07-19 当前 Single Issue 内完成。
- 决策动机：默认语言可以前进到 TSX，但兼容边界应留在既有协议和编译器中，不通过破坏性迁移实现统一。

冻结规则：产品名使用“代码区块”，编辑器名使用“TSX 编辑器”；新模板声明 `tsx`；旧 `jsx` 继续可运行；纯 TypeScript 源码必须经过 TSX 编译步骤。
