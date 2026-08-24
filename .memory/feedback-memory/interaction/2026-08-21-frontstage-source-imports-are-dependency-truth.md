---
memory_type: feedback
feedback_category: interaction
topic: Frontstage 区块依赖以源码 import 为唯一真值
summary: Frontstage 区块依赖仍以源码 import 触发，但解析、module/export 校验和加载全部归前端 registry；后端不得再从 import 生成 dependency lock。组件目录只是记录副本，不按运行时可用性隐藏。
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
updated_at: 2026-08-24 21
last_verified_at: 2026-08-24 21
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage
  - web/app/src/features/settings/components/ui-management/UiCodeTemplateStudio.tsx
  - api/crates/control-plane/src/frontstage
---

# Frontstage 区块依赖以源码 import 为唯一真值

## 规则

- 组件目录是人工维护的持久化记录副本；记录存在就展示，不按 module/export 当前是否可运行隐藏，也不按 Catalog `code_modules` 或 workspace 区块分配过滤。
- 区块编辑器消费后台 UI 管理的同一持久化组件目录；目录不承担 module registry 或可执行性真值。
- 插入组件时同时插入或补齐其命名 `import`，并加载该组件类型声明供编辑器使用。
- 前端 compiler/preview 根据源码 `import` 查询当前前端 module registry，校验 module/export 并加载实现、类型和 Shadow DOM 样式。
- 后端只保存源码、源码摘要/修订与业务状态，不解析 import、不校验 module/export，也不生成依赖锁；MCP/API 可以保存当前前端无法编译的源码，错误延迟到前端编译/预览。
- artifact cache identity 使用 `source_sha256 + compiler_abi + runtime_abi`；依赖升级通过 Vite/browser 内容哈希 chunk 换版，不绑定后端版本号。

## 原因

组件目录负责可插入记录，workspace assignment 负责区块类型可否创建，前端 module registry 负责当前构建真正可加载的模块；三者是不同对象。后端从源码复制依赖版本会制造第二真值，并让前端包升级与历史 lock 漂移。

## 适用场景

- Frontstage JSX Studio、后台 UI 代码模板编辑器和预览。
- 区块源码保存、source patch、运行预览的 Native React 模块锁解析。
