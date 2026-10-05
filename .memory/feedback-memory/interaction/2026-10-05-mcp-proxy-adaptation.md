---
title: "MCP 公网透传采用通用适配"
memory_type: feedback
feedback_category: interaction
created_at: "2026-10-05 17"
updated_at: "2026-10-05 17"
decision_policy: direct_reference
status: active
tags: [mcp, oauth, proxy]
---

- 规则：用户明确要求 MCP 公网代理采用通用适配，客户端配置什么公开 URL 就对应什么资源；不修改用户公网 Nginx，不增加域名环境配置。
- 原因：用户将 frpc 视为通用透传入口，不希望认证功能引入部署专用配置。
- 适用场景：MCP 地址投影与代理链路诊断；标准转发头可用于恢复外部地址，仍须区分地址投影与没有到达应用的路由问题，不能虚报完整授权已通过。
