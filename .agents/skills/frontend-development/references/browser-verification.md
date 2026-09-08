# Frontend Browser Verification

## Default Toolchain

- 前端页面的浏览器打开、检查、截图和交互复现，默认使用 `Playwright`。
- 只给一个前端路由，且需要自动登录、等待稳定态、导出截图 / 控制台 / `html/css/js` 证据时，默认使用 `node scripts/node/page-debug/cli.js`；当前入口与选项见 [scripts/README.md](../../../../scripts/README.md)。
- 不要把 Chrome 浏览器 MCP / `chrome-devtools` 当成前端默认链路，除非用户明确指定。

## Execution Rules

- 优先复用项目已有 `Playwright` 或 `style-boundary` 运行时验收链路。
- `page-debug` 用法优先级：
  - 需要结构化证据目录：`node scripts/node/page-debug/cli.js snapshot <route> --wait-for-selector ...`
  - 需要保留浏览器继续人工检查：`node scripts/node/page-debug/cli.js open <route> --wait-for-selector ...`
  - 只验证 root 登录链路：`node scripts/node/page-debug/cli.js login`
- 受保护页面与认证 API 使用 `page-debug` / `node scripts/node/tooling.js api-debug` 的临时 session owner。自定义 Playwright 流程复用 `scripts/node/page-debug/auth.js` 并在 `finally` 调用 `dispose()`，不要在临时脚本直接调用 `/api/public/auth/sign-in`；`open` 的会话在浏览器断开后回收。
- 页面存在规范化跳转或路由收敛时，补 `--wait-for-url <final-url>`；不要先假设最终 URL，先以当前运行态为准。
- 截图、点击、等待都应基于业务 ready signal，例如稳定文案、关键节点、页面主标题，而不是页面一打开就直接操作。
- `snapshot` 成功后，按实际 `outputDir` 检查 `meta.json`、`page.png`、`console.ndjson`，需要 DOM / 资源证据时再读 `index.html` 等产物。`storage-state.json` 属于认证辅助产物，不作为普通报告证据或复用长期会话的入口。
- 需要补充页面证据时，优先保留 `uploads/` 中的截图或失败证据，而不是只给口头判断。

## Fallback

- 若 Playwright 当前环境缺浏览器二进制、截图命令不可用或链路受限，先看已有 `tool-memory/playwright` 和 `style-boundary` 的已验证替代方案。
- 若 `page-debug` 失败，先读取它输出的错误 JSON 或已落盘证据，再决定是修等待条件、修路由假设，还是回退到更底层 Playwright 链路。
- 不要因为 Playwright 一次失败，就默认回退到 Chrome 浏览器 MCP。
- 等待条件、路由或运行环境仍无法满足时说明缺失证据；只要相关实现和环境未改变，不重复取证，不把受限截图写成页面验收通过。
