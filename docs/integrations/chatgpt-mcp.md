# ChatGPT MCP：使用 API Key 完成 OAuth 授权

ChatGPT 通过 OAuth 连接受保护的 MCP。用户在 **1flowbase 自己的授权页**填写已有用户 API Key、核对工作区和 MCP 实例后授权，不需要登录管理后台。原 API Key 不发送给 ChatGPT，不放入 URL；ChatGPT 获得独立的访问令牌与刷新令牌。

## 部署

1. 部署包含此功能的 API 和前端，运行正式 migrations。
2. 设置 API 服务环境变量，值是用户和 ChatGPT 都能访问的**网站 origin**，不包含 `/api`、MCP 路径、查询参数或凭据：

   ```dotenv
   API_MCP_OAUTH_ISSUER=https://1flowbase.example.com
   ```

   未设置时 OAuth 关闭。生产必须使用 HTTPS；本地开发允许 loopback HTTP。修改后重启 API。Docker 的三种应用 compose 文件均透传该变量。
3. Web、API 与授权页使用同一公开 origin。反向代理应将 `/api/` 和 `/.well-known/oauth-*` 转给 API，`/mcp/authorize` 转给前端。仓库 Nginx 和 Vite 配置已包含对应路由。
4. 确认 Cloudflare/WAF 没有阻止 ChatGPT 获取元数据、注册客户端或调用 MCP；这些机器请求不能要求浏览器挑战。不要通过关闭 MCP 鉴权解决网络拦截。

OAuth 公开地址是服务端配置真值，不从不可信 `Host` 或 `X-Forwarded-Host` 推断。公开地址改变后应重新建立连接。

## 连接

1. 打开「设置 → MCP → 连接客户端 → ChatGPT」，确认 OAuth 配置已启用，复制服务器 URL。
2. 在 ChatGPT 创建自定义 MCP，填写该 URL，身份验证选择 **OAuth**。
3. 注册方式选择 **动态客户端注册（DCR）**；无需手动填写 OAuth 客户端 ID/密钥，也不要把 API Key 填到这些字段。
4. 创建并连接后，在打开的 1flowbase 授权页填写 API Key。
5. 核对服务端解析出的工作区和 MCP 实例，点击授权；浏览器返回 ChatGPT。

不需要在「MCP 客户端配置」弹窗保存 API Key。已有通用、Codex、Claude Code、OpenCode 的请求头接入方式保持可用。

## 权限与失效

- OAuth 只授权指定 MCP 实例和对应 Key 的工作区，不能用该令牌登录管理台或访问其他实例。
- 后端每次使用令牌都重新校验原 Key、用户及角色权限；实际工具调用继续通过既有业务权限检查。
- 后台网页登录会话到期不会使该连接失效。
- 访问令牌到期后使用刷新令牌续期；刷新令牌轮换，重复使用旧刷新令牌会使关联授权失效。
- 原 Key 到期、撤销或用户被禁用后，访问和刷新都应失败。可通过撤销原 Key 终止其关联连接；恢复使用需填写有效 Key 重新授权。
- 授权请求或授权码过期、刷新失败时，从 ChatGPT 重新连接，不要反复复用旧授权页面。

## 协议与验证边界

采用授权码 + PKCE S256，客户端注册只接受 ChatGPT 的 HTTPS 回调；不支持任意客户端回调或 CIMD 外部元数据获取。授权码与刷新令牌由数据库原子消费，持久化的是令牌 hash 和关联数据，而不是 API Key 原文。

本地测试验证协议、权限及界面行为。真实 ChatGPT 账号的授权回跳、公网证书和 Cloudflare 可达性需要在部署后验证，不能由本地测试替代。

参考：[OpenAI MCP 认证要求](https://developers.openai.com/apps-sdk/build/auth/)；[MCP Rust SDK OAuth 示例](https://github.com/modelcontextprotocol/rust-sdk/blob/main/examples/servers/src/complex_auth_streamhttp.rs)。示例用于核对协议形状，不复用其演示用内存令牌存储或日志策略。
