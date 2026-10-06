# 把 1flowbase 连接到 ChatGPT：自动发现 → API Key 授权

[English version](chatgpt-mcp.en.md)

在 ChatGPT 填写 MCP 服务器地址、选择 OAuth。ChatGPT 自动读取认证配置；连接时打开 **1flowbase 的授权页面**。你在该页面填写自己的 **1flowbase 用户 API Key**，确认授权后返回 ChatGPT。

```text
ChatGPT：填写服务器 URL，选择 OAuth
                    ↓
ChatGPT：自动发现认证端点，动态注册客户端
                    ↓
1flowbase 授权页：填写 API Key → 验证 → 确认授权
                    ↓
ChatGPT：完成连接，调用你有权限使用的工具
```

API Key 不填在 ChatGPT 的客户端 ID 或客户端密钥里，也不放进服务器 URL。它只提交给 1flowbase；ChatGPT 获得单独的访问令牌和刷新令牌。

> 更新后的服务默认提供 OAuth 自动发现，无需新增环境变量。连接前确认自动发现地址可以访问；部署检查方法见文末。

## 第一步：复制 MCP 服务器 URL

在 1flowbase 打开 **设置 → MCP → 连接客户端**，然后在下方选择 **ChatGPT** 标签。标签顺序为「通用 → ChatGPT → Codex → Claude Code → OpenCode」，默认打开「通用」。

上方 API Key 输入和提示区域保持原有位置；连接 ChatGPT 无需在这里填写或保存 Key，稍后在跳转打开的授权页填写。

- 显示 **OAuth 配置已启用**：复制服务器 URL。
- 显示 **此部署尚未启用 ChatGPT 授权**：通常是旧版服务，先更新 API 与前端，见文末。

![第一步：在 1flowbase 的 ChatGPT 连接配置中复制服务器 URL](assets/chatgpt-mcp/connect-chatgpt.png)

上图来自本次静态构建的浏览器验证，服务器 URL 已遮盖；其他历史截图的个人域名已打码。下文使用官网地址 `https://1flowbase.taichuy.com/` 举例；实际连接请替换为自己部署的公开 HTTPS 地址，示例不表示官网提供该 MCP 实例。

新版 ChatGPT 地址包含浏览器当前 origin（协议、域名与端口），后端不需要根据代理后的 Host 猜测公网地址。例如：

```text
https://1flowbase.taichuy.com/api/mcp/1flowbase?origin=https%3A%2F%2F1flowbase.taichuy.com
```

以弹窗实际复制的地址为准。准备一个有效的用户 API Key；该 Key 所属的工作区应包含要连接的 MCP 实例。

## 第二步：在 ChatGPT 创建自定义 MCP 服务器

打开 **创建自定义 MCP 服务器** 界面，按下面填写。入口名称可能随 ChatGPT 版本变化。

| 界面字段 | 怎么填 |
| --- | --- |
| 名称 | 例如 `1flowbase` |
| 图标、描述 | 可选，不影响认证 |
| 连接 | 选择服务器 URL，粘贴第一步复制的完整地址 |
| 身份验证 | 选择 **OAuth** |

![第二步：ChatGPT 自定义 MCP 服务器的 OAuth 设置](assets/chatgpt-mcp/chatgpt-create.png)

展开 **高级 OAuth 设置**，核对下面这些项：

| 高级设置字段 | 怎么选或填写 |
| --- | --- |
| 客户端配置方式／注册方式 | 选择 **动态客户端注册（DCR）**；当前 1flowbase 不支持 CIMD |
| OAuth 客户端 ID、客户端密钥 | 使用 DCR，无需手动填写；不要填用户 API Key |
| 令牌端点身份验证方式（如有） | `none`；表示客户端不使用密钥交换令牌，用户仍需授权 |
| 默认请求的权限范围 | 如需填写，填 `mcp:invoke`，不要填 `openid`、`email` 或 `profile` |
| 基础授权范围 | 留空 |
| OAuth 端点 | 由 ChatGPT 自动发现，无需逐项手填 |

如果端点为空或自动发现失败，先检查文末的服务版本和代理路由。手工填写端点不能修复部署路由。若端点已发现但创建按钮仍不可用，检查界面是否还有未完成的必填项或错误提示。

点击 **以插件形式创建**（或当前界面的创建按钮），再按 ChatGPT 提示连接、授权。创建配置和用户授权可能分成两个步骤；仅创建成功不代表已经授权。

## 第三步：在 1flowbase 授权页填写 API Key

连接过程中，ChatGPT 应打开 1flowbase 的授权页面。

1. 确认页面域名是你连接的 1flowbase 域名。
2. 在 **API Key** 输入框填写自己的 1flowbase 用户 API Key。
3. 点击 **验证 API Key / Verify API key**。

![第三步：填写 API Key，点击验证 API Key](assets/chatgpt-mcp/authorize-key.png)

无需输入管理后台账号密码，也无需在「MCP 客户端配置」弹窗保存 Key。

如果页面提示请求无效或已过期，返回 ChatGPT 重新发起连接。不要手工打开一个没有授权请求参数的 `/mcp/authorize` 地址。

## 第四步：核对工作区和实例，确认授权

验证成功后，页面会清空 Key 输入，并显示：

| 页面信息 | 核对内容 |
| --- | --- |
| 请求连接的客户端 / Requesting client | ChatGPT |
| MCP 实例 / MCP instance | 你要连接的实例 |
| 授权工作区 / Authorized workspace | 该 Key 对应的工作区 |

![第四步：核对工作区和 MCP 实例，点击同意授权并返回](assets/chatgpt-mcp/authorize-consent.png)

确认无误后，点击 **同意授权并返回 / Authorize and return**。

工作区不对时，点击 **使用其他 API Key / Use another API key** 重新验证；不想连接则点击拒绝授权。

## 第五步：返回 ChatGPT 验证连接

回到 ChatGPT 后，确认连接状态，并调用一个你有权限使用的工具。

完成标志是 **授权成功回跳，且工具调用成功**。仅验证 API Key，或者只看到插件创建成功，都不算连接完成。

## 连不上时，先看这里

| 现象 | 下一步 |
| --- | --- |
| 1flowbase 显示「此部署尚未启用 ChatGPT 授权」 | 管理员检查服务版本和部署状态，见下面部署检查 |
| ChatGPT 报 `MCP server ... does not implement OAuth` | 检查未认证的 POST 探测是否返回带 `WWW-Authenticate` 的 401；若返回 415／400，先更新服务 |
| ChatGPT 发现不到 OAuth 端点 | 检查下文两个 `/api/public/mcp-oauth/` 元数据地址是否返回 JSON；根目录 `/.well-known/` 返回 404 不一定影响新版客户端 |
| 元数据或 MCP 请求返回 Cloudflare 挑战 / 403 | 调整必要机器请求的 WAF 策略，保留 MCP 鉴权 |
| 出现手填客户端 ID／密钥的要求 | 核对是否选择 DCR，而不是手动配置客户端 |
| 授权页没有打开 | 确认创建后是否还需要点击连接或授权；检查 ChatGPT 错误提示 |
| API Key 验证失败 | 检查 Key 是否有效、用户是否启用、工作区与实例是否匹配 |
| 授权页提示请求无效或过期 | 返回 ChatGPT 重新连接 |
| 原 Key 被撤销、到期或权限变化后调用失败 | 使用当前权限下的有效 Key 重新授权 |
| 授权约 30 天后失效 | 重新连接并授权；自动刷新不会延长授权总期限 |

## 部署管理员：部署检查

### 1. 更新服务

部署包含本次默认启用调整的 API、前端和正式数据库迁移。**无需配置 OAuth 环境变量，也无需手动设置公开地址。**

生产使用 HTTPS，Web、API 和授权页使用同一个公开网站地址。前端将 `window.location.origin` 作为完整 URL 的 `origin` 参数提交，服务校验后按它生成认证地址；从哪个域名连接，就在该域名完成授权与令牌调用。没有 `origin` 参数的旧连接仍按 Host 与协议处理。公开地址改变后，应在 ChatGPT 重新建立连接。新版显式 origin 连接使用 `/api/public/mcp-oauth/origins/…` 作为授权服务标识；重新复制完整 MCP URL 并在 ChatGPT 重新连接，以更新发现配置。

### 2. 核对反向代理路由

Web、API 和授权页使用同一个公开网站地址：

| 路径 | 转发目标 |
| --- | --- |
| `/api/` | API 服务 |
| `/mcp/authorize` | Web 前端 |

仓库 Nginx 和 Vite 配置已包含对应路由。新版自动发现入口放在 `/api/` 下，不要求修改面板默认的根目录 `/.well-known/` 证书验证规则，也不需要新增 OAuth 环境配置。新版 ChatGPT URL 的 `origin` 参数会贯穿发现、注册、授权与令牌调用，即使代理把 Host 改为内部地址，也不据此覆盖该公开地址。旧版不带参数的连接仍需代理保留公网 Host 与正确的 `X-Forwarded-Proto`。Cloudflare/WAF 不能要求 ChatGPT 的元数据、注册、令牌及 MCP 机器请求完成浏览器挑战。

### 3. 在浏览器检查是否真的启用

将下面地址中的域名和实例 ID 换成实际值：

```text
https://你的域名/api/public/mcp-oauth/config?instance_id=你的实例ID&origin=https%3A%2F%2F你的域名
```

应返回 `enabled: true`，并包含正确的 `server_url`。旧版服务返回 `enabled: false` 时，先更新服务；新版若返回错误，检查 `origin` 参数是否包含正确协议、域名及端口；不带参数的旧连接仍需检查代理请求头。

新版应沿 MCP 401 响应的 `WWW-Authenticate` 中 `resource_metadata` 地址读取资源元数据，再沿 `authorization_servers` 中的 issuer 读取 `issuer + /.well-known/openid-configuration`。这些地址包含自动生成的 origin 路径上下文，无需手填。下面两个固定地址用于检查旧版无 origin 参数的发现入口：

```text
https://你的域名/api/public/mcp-oauth/.well-known/openid-configuration
https://你的域名/api/public/mcp-oauth/protected-resource/你的实例ID
```

两者都应返回 JSON。

新版显式 origin 自动发现过程是（路径中的 `…` 是服务生成的 origin 上下文）：

```text
ChatGPT 请求 /api/mcp/实例ID?origin=编码后的完整origin
  → 401 响应的 WWW-Authenticate 指定 /api/public/mcp-oauth/origins/…/protected-resource/实例ID
  → 资源元数据指定授权服务 https://你的域名/api/public/mcp-oauth/origins/…
  → 客户端按协议尝试发现地址
  → 根目录发现失败时，继续读取 issuer + /.well-known/openid-configuration
  → 读取授权、令牌和动态注册端点
```

`/.well-known` 是协议规定的发现路径名称；这里使用 MCP 规范支持的 OpenID Connect 路径追加方式，把发现入口放在授权服务路径下。它只提供 OAuth 端点信息，不代表用户要申请 `openid` 权限，也不增加 OpenID 登录或 ID Token。

面板若接管根目录 `/.well-known/`，前两个根目录发现请求可能返回 404；支持 MCP 2025-11-25 发现顺序的客户端会继续尝试上述 `/api/` 地址。若旧客户端遇到第一次 404 就停止，应升级客户端或为标准根目录发现配置转发。不能保证所有旧客户端都支持这条发现流程。

授权服务器元数据应包含正确的 HTTPS 地址：

| 字段 | 当前实现的路径 |
| --- | --- |
| `issuer` | `https://你的域名/api/public/mcp-oauth` |
| `authorization_endpoint` | `/api/public/mcp-oauth/authorize` |
| `token_endpoint` | `/api/public/mcp-oauth/token` |
| `registration_endpoint` | `/api/public/mcp-oauth/register` |

这些是排障时核对的字段，不是要求普通用户在 ChatGPT 手工填写的配置。

新版端点位于上述 issuer 路径下；表格列出的固定路径仍保留给无 origin 参数的旧连接。`origin` 只接受合法完整 origin，不接受路径、账号密码、额外查询参数、片段或重复值。授权请求、授权码和令牌均绑定该地址；不会把它保存成影响其他域名的全站配置。

### 4. 核对未认证的 MCP 探测

ChatGPT 的发现探测可能没有 JSON 请求头或请求体。下面请求应返回 **401**，并带有指向该显式 origin 上下文资源元数据的 `WWW-Authenticate` 响应头：

```bash
curl -i -X POST 'https://你的域名/api/mcp/你的实例ID?origin=https%3A%2F%2F你的域名'
```

旧版服务可能在鉴权前返回 415，导致 ChatGPT 没有取得元数据地址，最终提示服务不支持 OAuth。新版让未认证请求优先得到 OAuth challenge；已经认证的正式调用仍须使用正确的 JSON 请求格式。

本地 Vite 开发代理在 `tmp/logs/web.log` 记录 `[mcp-oauth-proxy]` 诊断信息，包括请求方法、无查询参数的路径、User-Agent 和状态码，不记录 API Key、令牌或请求体。若请求被外层 Nginx 拦截而未到达开发代理，本机日志无法看见，需要核对外层已有访问日志。

仅在浏览器打开网页成功，不能证明 ChatGPT 后台的发现流程成功；仅看到 MCP 的 GET 返回 405，也不能据此判断服务未实现 OAuth。

### 5. 已连接，但调用 `mcp_list` 报格式错误

若 ChatGPT 能看到工具，但调用时提示 `structuredContent: Input should be a valid dictionary`，说明授权已经进入工具调用阶段，服务的工具结果格式需要更新。

MCP 要求 `structuredContent` 是 JSON 对象。新版将目录数组包装为 `{"result": [...]}`，对象结果保留原字段；文本内容也包含相同对象。更新并重启 API 后，重试 `mcp_list`（例如 `{"depth": 2, "limit": 100}`），无需修改 API Key 或代理配置。

## 授权有效期和权限

- OAuth 令牌只用于指定 MCP 实例和 Key 的工作区，不能用于登录管理后台或访问其他实例。
- 后端每次使用令牌都重新校验原 Key、用户与角色权限；工具调用仍遵循业务权限。
- 后台网页登录会话过期，不影响已建立的 MCP OAuth 授权。
- 访问令牌最长有效 1 小时，ChatGPT 可使用刷新令牌续期；每次授权最长持续 30 天。
- 原 Key 更早到期、被撤销或用户被禁用时，连接也会提前失效。刷新令牌轮换，重复使用旧刷新令牌会使关联授权失效。

## 实现依据与验证范围

当前协议采用授权码 + PKCE S256 与动态客户端注册（DCR）。注册只接受支持的 ChatGPT HTTPS 回调，不支持 CIMD 外部元数据获取。授权码和刷新令牌由数据库原子消费，持久化的是令牌 hash 与关联数据，不是 API Key 原文。

本教程参考当前 1flowbase 的授权流程、OpenAI 官方认证文档，以及本地 AgentDock 的自动发现、动态客户端注册和授权页面实现。

教程中的 1flowbase 截图来自本次通过公网域名访问真实开发服务的浏览器操作；使用临时 API Key 授权，完成后撤销。ChatGPT 创建界面的图片为用户提供的实际截图；截图中的个人域名已打码。

已通过公网测试客户端验证：根目录发现返回 404 后，通过 `/api/` 下的发现入口完成 API Key 授权、回跳捕获、工具列举和只读调用、刷新及撤销；`mcp_list` 使用 `{"depth": 2, "limit": 100}` 返回 93 条目录项，`structuredContent` 为 JSON 对象。

**真实 ChatGPT 验收已确认：2026-10-05，用户在原 ChatGPT 会话重试后明确反馈“可以了，成功了”。** 此前的 OAuth 发现和工具结果格式问题已在该公网环境解决，修复与提交关联见 [问题记录 #2280](https://github.com/taichuy/1flowbase/issues/2280)。

本次确认的环境为个人公网部署（域名已隐藏）经 frpc 透传到本地 dev 服务，没有修改外层 Nginx/frpc，也没有新增必填 OAuth 环境配置。其他部署仍需更新 API 和前端，并按教程验证；未进行独立生产部署。只要现有代理能正确透传 `/api/` 与授权页，就无需为本次发现路径调整修改公网 Nginx。

源码与更新：[仓库接入说明](https://github.com/taichuy/1flowbase/blob/dev/docs/integrations/chatgpt-mcp.md)。

参考：[MCP 授权与发现顺序](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization)；[OpenAI MCP 认证要求](https://developers.openai.com/apps-sdk/build/auth/)；[1flowbase 实现与验收记录](https://github.com/taichuy/1flowbase/issues/2249)。

### 显式 origin 更新验证（2026-10-06）

使用空 `VITE_API_BASE_URL` 打包后的前端、真实开发 API 与本机 Chrome，模拟代理将 Host 改为 `127.0.0.1`、根目录发现返回 404。测试客户端完成发现、API Key 授权、工具调用、刷新和撤销验证；HTTPS 公网 origin 场景另由后端集成测试覆盖。本次新连接格式尚待更新公网部署后在真实 ChatGPT 重连确认，不能用上述历史验收替代。

**前端和它实际连接的 API 都必须更新。** 只替换前端压缩包，旧 API 仍可能忽略 origin 并返回内部地址。更新后重新复制服务器 URL，在 ChatGPT 新建连接。
