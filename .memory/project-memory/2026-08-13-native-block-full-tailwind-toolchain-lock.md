---
memory_type: project
title: 代码区块完整 Tailwind 与可复现工具链锁定
created_at: 2026-08-13 18
updated_at: 2026-08-25 00
decision_policy: verify_before_decision
scope:
  - web/packages/page-runtime
  - web/packages/tailwindcss-catalog
  - web/app/src/features/frontstage
  - web/app/src/shared/code-block
  - api/crates/control-plane
  - api/crates/storage-durable
status: superseded
keywords:
  - native-react
  - tailwindcss
  - shadow-root
  - dependency-lock
  - durable-artifact
  - github-issue-1679
---

# 代码区块完整 Tailwind 与可复现工具链锁定

> Superseded at 2026-08-25 00：用户确认前端区块未实际采用 Tailwind，并授权完整移除 Tailwind 编译能力、依赖、治理设施与内置默认代码区块。后续不得再把 `import 'tailwindcss'`、Tailwind preset、编译 Worker 或内置 `frontstage.js-ui-block` 当作有效 contract。

## 谁在做什么？

用户已确认把代码区块从源码静态命中 utility 改为版本化 Tailwind Block Preset。`import 'tailwindcss'` 在区块 ShadowRoot 内挂载完整预设，不再由当前源码候选决定可用 utility；实现及目标区块升级已完成。

## 为什么这样做？

每个 Native React 区块已有独立 ShadowRoot，样式扩散由 runtime 隔离；继续维护 481 项私有 inventory 会误报块内自定义 CSS，并让标准 Tailwind utility、variant 与 arbitrary value 受 1flowbase 私有限制。

## 为什么要做？

恢复代码区块的标准 React、DOM、CSS 作者契约，同时保证平台升级 Tailwind 后历史页面不发生静默视觉漂移。

## 当前方向

- `import 'tailwindcss'` 使用版本锁定的 `block-preset-v1`，包含 theme、Preflight、默认 utility families 与标准 variants。
- 每区块 `generated_css` 允许为空；Tailwind 样式由 dependency lock 中的受控 `shadow_style` 资产提供。
- 源码、完整 Tailwind toolchain lock、样式资产摘要与编译器身份由后端可执行状态原子持久化。
- 缓存只作性能优化；缓存删除不得改变运行结果。
- 平台升级只改变新建区块默认工具链；已有区块只有显式升级并成功保存后才切换。
- runtime 继续把样式作为 `shadow_style` 注入当前区块 ShadowRoot。
- 插件安装事务按 `installation + module_source + SHA-256` 耐久保留校验后的浏览器资产；只有仍被 workspace 区块 lock 引用且安装归属有效时，历史 SHA 才可读取。
- `tailwindcss` runtime module 只导出区块作者所需的默认元数据；编译器实现仅从独立 `executable-contract` / CLI 入口使用，不进入受控 module registry。
- 不开放第三方 plugin、自定义配置、JavaScript 配置执行或主仓全局 Tailwind。
- Block Preset 取代源码静态命中 contract，但不推翻主仓 Tailwind 禁令和 ShadowRoot 隔离。

## 截止日期

无固定日期。当前实现与目标区块升级已完成，后续只保留用户人工视觉验收。

## 决策背后动机

样式隔离复杂度应由已拥有 DOM 边界的 runtime 吸收；依赖升级复杂度应由后端 lock 与耐久制品吸收，不能泄漏成作者需要理解的私有白名单，也不能由易失缓存冒充历史运行真值。

## 验收证据入口

- GitHub Issue：https://github.com/taichuy/1flowbase/issues/1679
- 重点验证：版本化 Block Preset、自定义 CSS 混用、A/B/host ShadowRoot 隔离、双版本 lock、cache-miss 可复现、原子保存、历史迁移 preview/rollback。
