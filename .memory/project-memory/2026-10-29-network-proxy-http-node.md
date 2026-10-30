---
memory_type: project
topic: SOCKS5 与 HTTP 节点代理选择
summary: 用户已确认平衡方案并授权实现：内置 SOCKS5，HTTP 节点可跟随工作区、指定已有代理或直接连接。
keywords: [network-egress, socks5, http-node, proxy-pool]
created_at: 2026-10-29 20
updated_at: 2026-10-29 20
last_verified_at: 2026-10-29 20
decision_policy: verify_before_decision
status: implemented_awaiting_acceptance
---

用户于 2026-10-29 确认由 Codex 实现 Issue #2708 的平衡方案，目的是在网络中心配置一次代理，再由 HTTP 节点直接复用，解决 SOCKS5 缺失和节点无法独立选代理的问题。没有约定截止日期。

边界：旧节点缺省跟随工作区；节点只保存代理引用，后端持有凭证并决定可用性；指定代理失效不回退直连；显式直接连接绕过环境代理。SOCKS5 默认使用代理端 DNS，覆盖无认证和用户名密码认证。采用 Single Issue；不扩展域名规则、自动故障切换或其他节点配置。

验收必须证明配置保存/恢复、实际 HTTP/HTTPS 代理转发、认证、失效引用、权限与旧 HTTP/工作区路由回归。执行与最新证据以 https://github.com/taichuy/1flowbase/issues/2708 为准，不把此批准记录视为已交付。

2026-10-29 20，已合入并推送 dev（6bdb87495），后端 CI 99 个用例通过；没有重启共享后端。Issue #2708 保持等待用户验收，详细证据在 tmp/test-governance/network-proxy/report.md。
