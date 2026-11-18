# 通过 OAuth 连接 MCP 客户端

[English](README.en.md)

在 **设置 → MCP → 连接客户端 → OAuth** 复制服务器 URL。ChatGPT 页签继续保留；“通用”及其他客户端配置不变。

1flowbase 支持 HTTPS 回调、OAuth 授权码流程、PKCE S256 和动态客户端注册（DCR）。令牌端点认证方式为 `none`；客户端 ID 自动注册，无需客户端密钥。用户 API Key 只提交给 1flowbase 授权页，不填写在客户端密钥或服务器 URL 中。

## Gemini 网页连接

1. 打开 Gemini 的 **Settings → Connected Apps**，添加自定义应用。
2. 粘贴 OAuth 页签中的完整服务器 URL，包括 `origin` 参数。
3. **Client ID / Client secret 留空**，点击 **Next**。
4. 在 1flowbase 授权页输入用户 API Key，验证后核对客户端回调域名、工作区和 MCP 实例，再确认授权。
5. 返回 Gemini，输入 `@` 选择应用并执行一次有权限的工具调用。仅创建连接不代表工具调用已验证。

API Key 无需在连接配置弹窗保存。访问令牌可自动刷新，每次授权最长持续 30 天；Key 到期、撤销、权限变化或 MCP 实例停用后，连接会失效，需要重新授权。

可用账号与地区以 [Google 自定义应用说明](https://support.google.com/gemini/answer/17209137) 为准。工具 schema 和调用排障可使用 [Gemini Developer Mode](https://developers.google.com/connected-apps/mcp/dev-mode)。

## 部署检查

公网 HTTPS 必须能访问 `/api/` 与 `/mcp/authorize`。按 MCP 未认证请求返回的 `WWW-Authenticate: Bearer resource_metadata="…"` 获取资源 metadata，再读取 `authorization_servers` 中的 issuer。

以下两个地址应返回 HTTP 200 和相同的 OAuth metadata，包括 `registration_endpoint`、授权码及 PKCE 能力：

```text
<issuer>/.well-known/oauth-authorization-server
<issuer>/.well-known/openid-configuration
```

保留服务器生成的完整 issuer，特别是 `/origins/…` 上下文，不手工删减。Google 可能在首个发现地址返回错误后停止探测，不能只确认第二个地址可用。反向代理如果截获根路径 `/.well-known/`，仍需确保本节的 `/api/` 发现入口完整可用。参见 [Google OAuth 指引](https://developers.google.com/connected-apps/mcp/oauth)。

DCR 接受符合条件的 HTTPS 回调，不按客户端品牌限制注册。授权请求中的回调必须精确匹配已登记地址；客户端名称是自报信息，授权页同时显示回调 origin，不能仅凭名称判断可信。服务器不主动访问回调 URL。

[ChatGPT 专用连接说明](../chatgpt-mcp.md)继续适用。已有 ChatGPT 客户端身份和刷新令牌轮换行为保持兼容。

通用 OAuth 客户端在授权期内复用刷新令牌，刷新不会延长 30 天授权上限。ChatGPT 保持现有轮换策略。

OAuth metadata 提供 `revocation_endpoint`；客户端断开时可用 access token 或 refresh token 调用它，撤销整次授权。
