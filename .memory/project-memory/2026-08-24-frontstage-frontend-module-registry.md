---
memory_type: project
topic: Frontstage 区块依赖收敛为前端 module registry
summary: 用户已批准并实现 #1871：后端只保存源码与摘要/修订，前端拥有模块、exports、实现、样式和内容哈希资产；artifact identity 为 source_sha256 + compiler_abi + runtime_abi。2026-08-25 删除旧资产后发现创建区块失去目录来源，现已确认恢复不携带浏览器资产与 Tailwind 的轻量系统代码区块声明。
keywords:
  - frontstage
  - module registry
  - dependency lock
  - source_sha256
  - compiler_abi
  - runtime_abi
  - MCP
created_at: 2026-08-24 21
updated_at: 2026-08-25 08
last_verified_at: 2026-08-25 08
decision_policy: verify_before_decision
source_issue: "#1871"
scope:
  - api/crates/control-plane/src/frontstage
  - api/plugins/capability-plugins/1flowbase/manifest.yaml
  - web/app/src/features/frontstage
  - web/packages/page-runtime
---

# Frontstage 前端模块注册与内容寻址缓存

## 谁在做什么

线上 Single Issue #1871 已进入 `phase:ready`。后续实现由 Frontstage 前端 compiler/runtime 与后端区块写入口共同完成，但模块 registry、exports、实现、类型、样式和浏览器资产只由当前前端构建拥有。

## 为什么这样做

后端 `code_modules` / `dependency_lock` 会把前端已安装模块复制成第二真值，并把描述版本错误绑定到区块 import 与 artifact identity；这会使前端包升级、浏览器内容哈希缓存和区块源码身份互相耦合。

## 为什么要做

目标是让源码不变时复用编译 artifact，让依赖升级仍通过新的 Vite 内容哈希 chunk 加载当前实现；MCP/API 只写源码时也无需后端具备前端编译能力。

## 已确认 contract

- UI 组件目录是人工维护的记录副本，不因当前 module/export 可用性隐藏；`upstream.version` 只作描述。
- 后端保存源码、`source_sha256`/revision 和业务状态，不解析 import、不验证 export、不生成新 dependency lock。
- MCP/API 可以保存当前前端无法编译的源码；错误统一在前端编译/预览暴露。
- artifact identity 为 `source_sha256 + compiler_abi + runtime_abi`，不包含依赖版本或后端 lock。
- 依赖 JS/CSS 使用 Vite/browser 内容哈希 URL；依赖升级不要求源码变化或重新发布区块。
- 旧 dependency-lock 字段与数据继续保留但 execution-inert。
- 2026-08-25 早间删除旧内置包后，`frontend_block_catalog` 失去唯一目录来源并导致无法创建区块；用户随后确认恢复 `provider_code=1flowbase / contribution_code=frontstage.js-ui-block` 的轻量系统代码区块声明，版本为 `1flowbase@2.0.0`。该包只提供区块身份与默认 TSX 模板，不携带 `code_modules`、浏览器资产或 Tailwind。
- 前端 module registry 不再注册 `tailwindcss`；Tailwind 编译、样式资产和相关缓存 identity 已退出 Native React contract。

## 截止日期与动机

未指定截止日期。决策动机是把必要复杂度放回能观测并控制它的 owner：后端拥有用户源码，前端构建拥有实际模块，浏览器缓存拥有内容哈希资产；避免用版本号和跨端 lock 协调三个不同生命周期。
