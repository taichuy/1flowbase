---
title: "ChatGPT MCP API Key 授权接入"
memory_type: project
created_at: "2026-10-04 08"
updated_at: "2026-10-05 23"
decision_policy: verify_before_decision
status: active
tags: [mcp, oauth, chatgpt]
---

用户于 2026-10-04 明确批准实现：ChatGPT 跳转到 1flowbase 独立授权页，用户填写已有用户 API Key 完成验证与显式授权，无需账号密码登录后台。对 ChatGPT 仍实现标准 OAuth 授权码 + PKCE、独立令牌与刷新。用户动机是复用已创建 Key、减少登录步骤；不接受把输入 Key 误解为直接填 OAuth client secret。

Root 负责实现/装配，范围为后端 OAuth 生命周期、前端授权页/ChatGPT Tab、必要部署代理配置。原 Key 过期/撤销或用户禁用时关联授权失效；权限继续服务端检查，Key 原文不交给 ChatGPT、不放 URL。当前无指定截止日期，完成标准以任务 issue 的 AC 与实际验证为准。

首轮实现记录：https://github.com/taichuy/1flowbase/issues/2249；纵向 Delivery #2250。候选位于 /home/taichuy/git/git_worktree/mcp-oauth-chatgpt-20261004，目标主工作树 dev 保持原分支。未据此授权自动生产部署；公网/Cloudflare与真实ChatGPT回跳须明确区分本地证据。

当前阶段（2026-10-05 17）：用户明确批准去掉 OAuth 环境变量，默认启用自动发现；Root 在主目录 dev 实现并提交、推送 f7fd95b82。公开地址从请求 Host 与协议生成，Nginx/Vite 保留外部地址；授权请求、授权码交换、刷新及调用仍绑定本次 origin、工作区和实例。该决定替代首轮必填 API_MCP_OAUTH_ISSUER 的部署条件，目的是让普通用户只填服务器 URL、选 OAuth/DCR，再跳转填写 API Key。原 Key 和最长30天生命周期保持原设计，无指定截止日期。

验证：API OAuth 7 个定向测试、前端 9 个测试通过；Playwright 使用真实本地 API 和临时 Key 完成 DCR、无后台会话的 Key 验证/授权、回跳捕获、工具列举/只读调用、刷新与撤销验证；桌面/手机截图和真实 Nginx origin/端口投影通过。QA 入口 tmp/test-governance/chatgpt-oauth/report.md。Wiki 已更新到 fd8bc4c，入口仍为 https://github.com/taichuy/1flowbase/wiki/ChatGPT-MCP-API-Key-Authorization-CN ，教程和四张图同步。未执行生产部署；真人 ChatGPT 账号的接入未验证，本地回跳使用测试客户端。后续先核对当前代码及实际部署，不以 Wiki 发布代表服务升级。

2026-10-05 17 用户纠正客户端弹窗布局：保留 API Key 与提示区域在 Tabs 上方，切换 ChatGPT 不隐藏上方区域；通用第一、ChatGPT 第二，默认通用。Root 按局部组件修复，定向前端测试及桌面/手机 Playwright 布局检查通过，教程与连接截图同步更新；真实 ChatGPT 接入及生产部署仍非本轮布局验收范围。

布局修复已提交并推送 dev `6cb341b05`，Wiki `9c9c00a`；证据 `tmp/test-governance/chatgpt-oauth/tabs-report.md`。

2026-10-05 17 FRP 公网域名 https://1flowbase.taichuy.cn 对接本地3100，用户要求通用透传适配，禁止修改公网 Nginx。已核对请求保留 X-Forwarded-Host 和 HTTPS 协议，修复 Vite 重复追加协议及公网 Host 投影；公网页面已显示正确服务器地址，12 个测试及双端公网浏览器通过。公网 /.well-known 仍返回 Nginx404，不视为完整ChatGPT连接已通过。证据 tmp/test-governance/chatgpt-oauth/frpc-report.md。


2026-10-05 20 用户批准避开面板根目录 `/.well-known/` 证书验证 location：Root 在主目录 dev 使用标准 MCP 发现顺序，把授权服务标识设为网站 origin + `/api/public/mcp-oauth`，401 明确指向 `/api/` 下的资源元数据，并提供授权路径下的 OIDC 发现文档。网站 Origin、资源 URL 与授权服务 issuer 分离；没有修改公网 Nginx/frpc，也没有增加环境配置。动机是已有通用代理只要透传 API 与授权页就能使用，不要求按部署添加 OAuth 特殊路由；无指定截止日期。

实现已提交并推送 `ce19ed60e`，Wiki `27b2b0a`；本地 API 标准重建/重启完成，3100/7800 开发服务通过公网 `https://1flowbase.taichuy.cn` 验证。24 项定向测试通过，Nginx 临时容器检查通过；公网根目录两个发现地址仍404，但测试客户端通过 API 路径发现、DCR、真实 Key 授权、捕获回跳、工具列举/只读调用、刷新、撤销均通过，临时 Key/session 已回收。验收入口 `tmp/test-governance/chatgpt-oauth/path-discovery-report.md`。该状态替代上一阶段“根目录404仍阻断完整服务侧验证”的结论；不能据此断言真人 ChatGPT 已通过。要求客户端支持 MCP 的 OIDC 路径追加发现顺序；旧客户端可能仍需升级或标准根目录转发。生产镜像未部署，已有 ChatGPT 连接应重新连接以更新 issuer。


2026-10-05 20 用户实际 ChatGPT 创建页面仍报告无法发现 OAuth。该反馈证明前一阶段测试客户端的通过不能作为 ChatGPT UI 接入成功；当前集成继续诊断。已启用 Vite 侧 bounded MCP/OAuth 请求/响应日志，位置 `tmp/logs/web.log`，标记 `[mcp-oauth-proxy]`，只含时间/方法/无查询路径/状态/UA与公开路由信息，不记录 credentials/body。当前公网 GET MCP 为405且没有 metadata challenge，POST为401且有challenge；GET 探测是否导致失败仍是假设，须抓真实重试请求。已要求用户点击重试并回复；外层根目录404若未进入Vite，须看外层已有access log。不得把无本机日志推断成OpenAI无请求或网络不可达。诊断证据 `tmp/test-governance/chatgpt-oauth/discovery-diagnostics-report.md`，未改变公网配置。


2026-10-05 21 用户提供真实报错 `MCP server ... does not implement OAuth` 和临时 `tmp/curl/gpt-mcp.sh` 请求。日志于北京时间20:52:25捕获 Python aiohttp POST MCP415，随后 MCP路径追加的三个well-known探测404；上一阶段GET假设不是本次链路。Root 修复 Axum Json extractor rejection 的响应优先级：缺少凭据的探测即使媒体类型/JSON不合要求也返回 OAuth401challenge；认证成功后保留原JSON415/400。9项OAuth回归和公网完整测试客户端流程通过，裸POST公网已401。修复提交并推送dev `c65f2fd55`、Wiki `af580ee`，本地API重建/重启完成，外层Nginx/frpc未改。用户复制的ChatGPT请求在本机curl和Chrome重放均非JSON403，不能把该403当成1flowbase响应或验证ChatGPT成功；已请求用户在原ChatGPT会话再次发现并继续抓日志。证据 `tmp/test-governance/chatgpt-oauth/probe-auth-report.md`，凭据未输出或提交。

2026-10-05 用户最新反馈显示 ChatGPT 已看到四个 meta tools，并进入 mcp_list 调用；当前阻塞是 CallToolResult.structuredContent 返回数组，违反协议 object 类型。Root 修复统一协议投影：对象保持原字段，非对象包装为 {"result": 原值}，文本与结构结果一致，缓存/分页原始路径保持不变。62 项 MCP 定向测试与公网测试客户端相同 depth=2/limit=100 调用通过（93条），本地 API 已重建重启；真人 ChatGPT 重试尚未验证。证据 tmp/test-governance/chatgpt-oauth/structured-content-report.md。此状态替代此前认证是否能进入工具调用尚未知的阶段判断。

2026-10-05 23 用户在真实 ChatGPT 会话重试后确认“可以了，成功了”，补齐此前实际客户端验收缺口。Root 按用户要求将成功状态更新到 Wiki 与仓库教程，并以 #2280 记录公网接入修复、关联提交和验收评论。成功范围为当前 1flowbase.taichuy.cn 透传到本地 dev 的环境，不表示独立生产部署或所有客户端均验证；后续优先复核当前服务状态，无指定截止日期。
