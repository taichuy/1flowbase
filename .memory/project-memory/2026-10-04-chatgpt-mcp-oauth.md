---
title: "ChatGPT MCP API Key 授权接入"
memory_type: project
created_at: "2026-10-04 08"
updated_at: "2026-10-04 09"
decision_policy: verify_before_decision
status: active
tags: [mcp, oauth, chatgpt]
---

用户于 2026-10-04 明确批准实现：ChatGPT 跳转到 1flowbase 独立授权页，用户填写已有用户 API Key 完成验证与显式授权，无需账号密码登录后台。对 ChatGPT 仍实现标准 OAuth 授权码 + PKCE、独立令牌与刷新。用户动机是复用已创建 Key、减少登录步骤；不接受把输入 Key 误解为直接填 OAuth client secret。

Root 负责实现/装配，范围为后端 OAuth 生命周期、前端授权页/ChatGPT Tab、必要部署代理配置。原 Key 过期/撤销或用户禁用时关联授权失效；权限继续服务端检查，Key 原文不交给 ChatGPT、不放 URL。当前无指定截止日期，完成标准以任务 issue 的 AC 与实际验证为准。

唯一执行账本：https://github.com/taichuy/1flowbase/issues/2249；纵向 Delivery #2250。候选位于 /home/taichuy/git/git_worktree/mcp-oauth-chatgpt-20261004，目标主工作树 dev 保持原分支。未据此授权自动生产部署；公网/Cloudflare与真实ChatGPT回跳须明确区分本地证据。

当前阶段：实现已合入并推送 dev（ef72ab63b），源码集成验收通过；每次授权最长30天，access最长1小时，原Key提前失效时授权亦失效。部署需设置API_MCP_OAUTH_ISSUER；未部署、未验证真人ChatGPT回跳。证据保留主目录tmp/test-governance/mcp-oauth/qa3/report.md，使用文档docs/integrations/chatgpt-mcp.md。候选worktree在集成后回收，后续从主目录现状核对。

用户要求同步项目Wiki并附操作截图；已发布图文入口：https://github.com/taichuy/1flowbase/wiki/ChatGPT-MCP-API-Key-Authorization-CN 。包含部署、连接参数、两张本地模拟API界面截图（明确标注示例）、权限和过期处理，并更新Wiki首页与侧栏。
