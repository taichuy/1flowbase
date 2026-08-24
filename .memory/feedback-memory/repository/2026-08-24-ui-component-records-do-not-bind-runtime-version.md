---
memory_type: feedback
feedback_category: repository
topic: UI 组件记录不承担运行时可用性与版本绑定
summary: UI 组件目录是人工维护的纯记录副本，不因当前运行时是否已开放对应模块而隐藏或过滤；记录可展示上游版本，前端区块源码与 import contract 只按模块身份和导出名解析，不与版本号绑定；实际宿主组件版本仍须进入 runtime fingerprint 使升级后旧编译缓存失效。
keywords:
  - ui-component-catalog
  - frontstage
  - import
  - module-identity
  - version
  - runtime
match_when:
  - 设计或修改 UI 组件目录、组件插入、前端区块 import 或依赖解析
  - 判断组件记录是否应按运行时可用性过滤
  - 判断上游版本是否应进入前端区块兼容性或 import contract
created_at: 2026-08-24 17
updated_at: 2026-08-24 17
last_verified_at: 2026-08-24 17
decision_policy: direct_reference
scope:
  - ../1flowbase-official-plugins/ui_components
  - api/crates/control-plane/src/frontstage
  - api/plugins/capability-plugins/1flowbase
  - web/app/src/features/frontstage
  - web/packages/page-runtime
---

# UI 组件记录不承担运行时可用性与版本绑定

## 规则

1. UI 组件目录保留人工维护的记录副本；记录存在不等于运行时已开放，也不要求系统自动验证、隐藏或过滤。
2. `upstream.version`、记录 `version` 等版本字段可以作为说明和维护信息展示，但不进入前端区块源码、裸模块 import 或要求调用方选版本的兼容性判定。
3. 前端区块 import 只绑定稳定模块身份与实际使用的导出名；本地组件升级由维护者逐项验证，平台不为此引入目录可用性过滤或按版本隔离的 import 语法。
4. “import 不绑定版本”不等于“缓存忽略实际运行版本”：宿主实际加载的组件版本必须进入 `runtime_fingerprint` 或等价缓存身份，使组件升级必然 cache miss、重新编译并加载当前宿主组件。

## 原因

组件目录的职责是承载人工维护的示例记录，运行时模块开放是另一条能力边界。把目录记录与运行时版本绑定会泄漏发布和依赖管理复杂度到用户区块源码，并阻碍本地组件自然升级；反过来若缓存身份不观察宿主实际版本，升级后可能误用旧编译产物，因此缓存 owner 必须吸收版本变化。

## 适用场景

- UI 组件目录同步、展示与人工维护。
- 前端区块组件插入、依赖解析和运行时模块注册。
- `@ant-design/x`、`@ant-design/x-markdown` 等本地组件升级。
