---
memory_type: project
topic: mcp-oauth-connector-scope
summary: 用户批准保留 ChatGPT、增加通用 OAuth 页签和 Gemini 指引，后端共用 OAuth。
created_at: "2026-11-16 12"
updated_at: "2026-11-16 12"
decision_policy: verify_before_decision
scope: [mcp-client-oauth]
---

用户在 2026-11-16 批准 Root 实现：保留现有 ChatGPT 入口，在其后增加 OAuth 页签，先提供 Gemini 网页连接说明。共用现有 API key 授权页和后端 OAuth，完善发现与通用 DCR；保留既有 ChatGPT 客户端身份、授权和刷新行为。

动机：Gemini 网页拒绝现有发现/注册链路，用户希望继续使用现有 1flowbase API key 完成授权，并避免影响 ChatGPT。

边界：不改“通用”名称，不引入独立 OAuth 网关或第三方依赖。实际账号内连接成功需用户侧验证，协议测试不能代替该事实。未指定截止日期；交付及用户验收时以当前代码、测试和运行态重新核对。

2026-11-16 12：实现提交 `103bbb02b` 已合入并推送 `dev`，本地 7800 已重启。任务行为证据：7 个 service/PostgreSQL、9 个 HTTP OAuth、20 个前端用例通过，TypeScript 检查通过；公网发现 200、Google 回调 DCR 201。原始证据在 `tmp/test-governance/mcp-oauth/`，实际 Gemini 账号内连接仍未验证。
