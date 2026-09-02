---
memory_type: project
topic: Native React ShadowRoot 外部资源能力
summary: 用户批准并完成 #1895：保留每区块 ShadowRoot 隔离，通过 ctx.root/ctx.assets 显式开放 HTTPS ESM、style、script、SVG Sprite，并开放管理员区块的浏览器 API；React mount/Portal owner 限制不变。
keywords:
  - native React
  - ShadowRoot
  - ctx.assets
  - HTTPS ESM
  - SVG Sprite
  - IconFont
match_when:
  - 调整 Native React 区块的外部资源或浏览器 API
  - 判断第三方 icon、CSS、script、ESM 应注入主文档还是 ShadowRoot
  - 修改 BlockContext 作者契约或 React mount/Portal 限制
created_at: 2026-08-26 16
updated_at: 2026-08-26 16
last_verified_at: 2026-08-26 16
decision_policy: verify_before_decision
status: user_acceptance
source_issue: "#1895"
scope:
  - web/packages/page-protocol
  - web/packages/block-sdk
  - web/packages/page-runtime
  - web/app/src/features/frontstage
  - web/app/src/features/auth
---

# Native React ShadowRoot 外部资源能力

## 谁在做什么

1flowbase 为管理员与 AI 编写的 `native_react` 区块提供原生浏览器外部资源能力：Host 继续拥有每个区块的 ShadowRoot 和 React Portal，在完整 `BlockContext` 中注入当前 `ctx.root` 与 ShadowRoot-aware `ctx.assets`。

## 为什么这样做

第三方 IconFont 把 symbols 注入主文档时，ShadowRoot 内的 `<use>` 无法解析，图标会成为 `0 × 0`；同时仅允许宿主 Module Registry 会限制 AI 使用固定版本 HTTPS ESM、外部样式和普通脚本。样式隔离和作者自由不是二选一：隔离由 DOM owner 决定，外部资源可由 Runtime 显式注入当前 owner。

## 为什么要做

目标是让 AI 继续使用原生 React、DOM 和浏览器生态，不为外部资源发明私有组件 DSL，同时保持区块样式不污染宿主或相邻区块，并让资源随区块生命周期可靠释放。

## 截止日期

未指定；#1895 已在 2026-08-26 完成本地实现与 Dev Acceptance Gate，当前等待用户验收。

## 已确认决策

- 保留 ShadowRoot，不切换 Light DOM 或 iframe。
- 宿主裸包名继续由 Module Registry 解析；固定版本 HTTPS ESM 使用 `ctx.assets.importModule()`。
- `ctx.assets.loadStyle/loadScript/loadSvgSprite` 将可释放资源放入当前 ShadowRoot；单 handle 与整个 scope 的释放均幂等，pending element load 在卸载时也必须取消并结算。
- 当前管理员原生区块开放 `window/document/globalThis/self/fetch/XMLHttpRequest/WebSocket/storage/cookie` 等浏览器能力；该 runtime capability guard 不是安全 sandbox。
- `require`、`eval`、`Function` 以及 React root/Portal takeover 仍禁止，React mount 与 Portal owner 继续由 Host 持有。
- 第三方脚本主动写入 `document.body/head` 的副作用不会被 loader 自动重定向；作者需自行选择并清理。
- 不扩大到后端代理、资源持久化、生产 CSP 或未注册裸包名自动安装。

## 验收真值

MCP 示例区块 `01a03cdb-9fde-79c3-8ac3-e71bae3fe49a` 使用阿里 IconFont 与 `esm.sh/dayjs@1.11.13`。真实浏览器证据显示 ESM/CDN 请求为 200、三个图标尺寸非零、121 个 symbols 只存在于当前 ShadowRoot、主文档泄漏为 0，页面显示 `ESM: 2026-08-26`。最终源码 revision 为 `7f8e59269892d5e8d7e985d2eb7cc37122aae55e67ab0d9f822d599aab9cce79`。
