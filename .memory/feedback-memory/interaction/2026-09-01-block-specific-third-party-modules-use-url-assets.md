---
memory_type: feedback
feedback_category: interaction
topic: block-specific-third-party-modules-use-url-assets
summary: 单个 Block 的第三方算法或展示依赖没有平台业务要求时，优先在 Block 源码中通过 ctx.assets.importModule 加载固定 HTTPS URL，不提升为宿主公共依赖。
keywords:
  - Native Block
  - third-party module
  - ctx.assets.importModule
  - URL import
  - host dependency
match_when:
  - 单个 Block 因第三方 npm import 未进入模块目录而预览失败
  - 讨论是否把示例专用算法库加入宿主依赖
  - Block 需要兼容第三方示例但平台没有对应业务需求
created_at: 2026-09-01 23
updated_at: 2026-09-01 23
last_verified_at: 2026-09-01 23
decision_policy: direct_reference
scope:
  - Native Block source
  - frontend runtime dependencies
  - external assets
---

# Block 专用第三方模块优先使用 URL Asset

## 规则

- 第三方算法或展示能力只服务单个 Block，且平台没有对应业务要求时，不因示例源码的静态 import 将其加入宿主 `package.json` 或公共 Module Registry。
- 使用 Native Block 已有的 `ctx.assets.importModule()` 加载固定版本 HTTPS ESM URL；静态 `import ... from 'https://...'` 仍不属于受控源码导入。
- 源码只声明实际使用的最小类型，并提供不使整个 Block 预览失败的业务合理降级。
- 只有多个 Block 形成稳定公共需求、远程依赖不可接受或必须离线运行时，才重新评估宿主受控依赖。

## 原因

示例专用库提升为宿主依赖会扩大安装、版本、Module Registry、类型声明和供应链治理范围。URL asset 把可选复杂度留在使用它的 Block，同时保留平台静态模块白名单。

## 已验证样例

`lunar-typescript@1.8.6` 通过 `ctx.assets.importModule('https://esm.sh/lunar-typescript@1.8.6?bundle')` 加载，CDN 返回 200；失败时 Calendar 保持可运行的公历降级。
