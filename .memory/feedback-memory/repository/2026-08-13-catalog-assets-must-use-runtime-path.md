---
memory_type: feedback
feedback_category: repository
topic: Catalog 浏览器资产必须通过真实 Runtime 路径验收
summary: 验收开放注册组件包时，必须加载生成后的 browser_module 并通过 Native React Module Registry 实际渲染；宿主源码直接 import 同名 npm 包不能证明 Catalog 资产可用。
keywords:
  - Catalog Runtime
  - browser_module
  - Native React Module Registry
  - official browser assets
  - low-code component packages
match_when:
  - 构建、修复或验收 Frontstage 低代码开放组件包
  - 修改 official browser assets、dependency lock 或 Native React Runtime
  - Demo 要求展示所有已注册包
created_at: 2026-08-13 08
updated_at: 2026-08-13 08
last_verified_at: 2026-08-13 08
decision_policy: direct_reference
scope:
  - web/scripts/build-official-browser-assets.mjs
  - web/packages/page-runtime
  - web/app/src/features/frontstage
  - api/plugins/capability-plugins/1flowbase/browser-assets
---

# Catalog 浏览器资产必须通过真实 Runtime 路径验收

## 时间

`2026-08-13 08`

## 规则

开放注册组件包的验收必须使用仓库生成的 `browser_module` 字节、真实 dependency lock 和 Native React Module Registry，并至少实际渲染一个公开组件。宿主测试文件直接 import npm 包只能证明宿主构建链可用，不能作为 Catalog 资产验收证据。

## 原因

`@ant-design/icons` 在宿主 Vite 导入路径能正常渲染，但官方压缩 ESM 资产经过 Runtime 的 Sucrase import transform 后发生标识符碰撞，实际页面抛出 `Cannot read properties of undefined (reading 'createElement')`。此前浏览器夹具直接 import `UserOutlined`，因未经过 Catalog Runtime 而产生假绿灯。

## 适用场景

- 新增、升级或重新生成官方浏览器组件资产。
- 调整 Catalog ESM 转换、dependency lock、模块注册和缓存。
- 验收声称“所有已注册包可用”的低代码 Demo。
