---
memory_type: feedback
feedback_category: interaction
topic: Frontstage 区块依赖以源码 import 为唯一真值
summary: Frontstage 区块是代码；组件目录应展示系统已安装且可运行的全局组件，插入时自动写入 import，运行依赖锁只能从源码静态 import 解析，不能由当前区块 Catalog 的 code_modules 或 workspace 区块分配限制或推断。
keywords:
  - frontstage
  - JSX
  - component catalog
  - import
  - dependency lock
  - source of truth
match_when:
  - 调整 Frontstage 或 UI 代码模板编辑器的组件面板、插入动作或 Monaco 诊断
  - 调整 Native React 区块的依赖锁、保存、预览或运行时模块解析
  - 排查全局组件目录中组件可见但当前区块不能使用的情况
created_at: 2026-08-21 18
updated_at: 2026-08-21 19
last_verified_at: 2026-08-21 19
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage
  - web/app/src/features/settings/components/ui-management/UiCodeTemplateStudio.tsx
  - api/crates/control-plane/src/frontstage
---

# Frontstage 区块依赖以源码 import 为唯一真值

## 规则

- 组件面板以系统已安装且当前节点可运行的全局组件为范围，不按正在编辑区块的 Catalog `code_modules` 或 workspace 区块分配过滤。
- 组件目录必须复用后台 UI 管理的同一候选查询；编辑器的 API 只做该候选集的权限约束、展示投影、搜索和分页，不能在另一个 service 中再次遍历模块清单。
- 插入组件时同时插入或补齐其命名 `import`，并加载该组件类型声明供编辑器使用。
- 后端只根据保存或预览源码中的静态 `import` 解析已注册模块、资产和版本锁；没有 import 的 Catalog 模块不得进入依赖锁。

## 原因

Catalog 负责区块模板和运行时身份，workspace assignment 负责区块类型可否创建；两者都不是作者源码可用组件的依赖真值。用它们推断依赖会让已安装的全局组件被错误隐藏或拒绝，也会给没有实际使用的模块加锁。候选集有两个构建入口会让后台和编辑器再次漂移。

## 适用场景

- Frontstage JSX Studio、后台 UI 代码模板编辑器和预览。
- 区块源码保存、source patch、运行预览的 Native React 模块锁解析。
